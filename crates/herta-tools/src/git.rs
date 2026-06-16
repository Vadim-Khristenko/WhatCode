//! Интеграция с Git (только чтение). Инструменты дают модели обзор репозитория
//! без мутаций: статус, история, дифф, текущая ветка, список веток.
//!
//! Все команды выполняются в каталоге `repo_root` с таймаутом. Деструктивные
//! операции (commit/push/reset) сознательно не предоставляются — это политика
//! безопасности уровня инструмента.

use crate::registry::Tool;
use crate::util::run_capture;
use async_trait::async_trait;
use herta_core::{ParamType, ToolCall, ToolParameter, ToolResult, ToolSpec};
use std::path::PathBuf;

const TIMEOUT_SECS: u64 = 15;

#[derive(Clone)]
struct GitContext {
    repo_root: PathBuf,
}

impl GitContext {
    fn new() -> Self {
        Self {
            repo_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    async fn git(&self, tool: &'static str, args: &[&str]) -> ToolResult {
        match run_capture("git", args, Some(&self.repo_root), TIMEOUT_SECS).await {
            Ok(out) if out.combined.is_empty() => ToolResult::ok(tool, "(пусто)"),
            Ok(out) => ToolResult::ok(tool, out.combined),
            Err(e) => ToolResult::rejected(tool, e),
        }
    }
}

/// `git_status` — рабочее дерево в кратком формате.
pub struct GitStatusTool {
    ctx: GitContext,
}
impl Default for GitStatusTool {
    fn default() -> Self {
        Self {
            ctx: GitContext::new(),
        }
    }
}

#[async_trait]
impl Tool for GitStatusTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_status",
            "Показать состояние рабочего дерева Git (изменённые, добавленные, неотслеживаемые файлы) \
             в кратком формате `git status --short`. Только чтение. Используй, чтобы понять, что \
             сейчас изменено в репозитории, прежде чем советовать действия.",
            vec![],
        )
    }
    async fn call(&self, _call: &ToolCall) -> ToolResult {
        self.ctx
            .git("git_status", &["status", "--short", "--branch"])
            .await
    }
}

/// `git_log` — последние коммиты.
pub struct GitLogTool {
    ctx: GitContext,
}
impl Default for GitLogTool {
    fn default() -> Self {
        Self {
            ctx: GitContext::new(),
        }
    }
}

#[async_trait]
impl Tool for GitLogTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_log",
            "Показать последние коммиты Git одной строкой каждый (хеш, автор, относительная дата, \
             заголовок). Только чтение. Параметр `count` ограничивает число коммитов (по умолчанию 10, \
             максимум 50).",
            vec![ToolParameter::new("count", ParamType::Integer, "Сколько коммитов показать (1..50)", false)],
        )
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let count = call
            .arguments
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(10)
            .clamp(1, 50);
        let fmt = "--pretty=format:%h %an %ar %s";
        let n = format!("-n{count}");
        self.ctx.git("git_log", &["log", &n, fmt]).await
    }
}

/// `git_diff` — несохранённые изменения (опционально по одному пути).
pub struct GitDiffTool {
    ctx: GitContext,
}
impl Default for GitDiffTool {
    fn default() -> Self {
        Self {
            ctx: GitContext::new(),
        }
    }
}

#[async_trait]
impl Tool for GitDiffTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_diff",
            "Показать несохранённые изменения рабочего дерева (`git diff`). Только чтение. \
             Необязательный `path` ограничивает дифф одним файлом или каталогом относительно корня \
             репозитория. Используй для ревью правок перед коммитом.",
            vec![ToolParameter::new(
                "path",
                ParamType::String,
                "Путь относительно корня репозитория",
                false,
            )],
        )
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        match call.arg_str("path") {
            Some(path) => {
                // Запрещаем выход за пределы репозитория простым правилом.
                if path.contains("..") {
                    return ToolResult::rejected("git_diff", "путь не должен содержать `..`");
                }
                self.ctx.git("git_diff", &["diff", "--", &path]).await
            }
            None => self.ctx.git("git_diff", &["diff"]).await,
        }
    }
}

/// `git_branches` — текущая ветка и список локальных веток.
pub struct GitBranchTool {
    ctx: GitContext,
}
impl Default for GitBranchTool {
    fn default() -> Self {
        Self {
            ctx: GitContext::new(),
        }
    }
}

#[async_trait]
impl Tool for GitBranchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_branches",
            "Показать локальные ветки Git; текущая помечена `*`. Только чтение. Используй, чтобы \
             узнать, на какой ветке идёт работа и какие ещё ветки есть.",
            vec![],
        )
    }
    async fn call(&self, _call: &ToolCall) -> ToolResult {
        self.ctx.git("git_branches", &["branch", "--list"]).await
    }
}

/// `git_show` — показать конкретный коммит (по умолчанию HEAD).
pub struct GitShowTool {
    ctx: GitContext,
}
impl Default for GitShowTool {
    fn default() -> Self {
        Self {
            ctx: GitContext::new(),
        }
    }
}

#[async_trait]
impl Tool for GitShowTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_show",
            "Показать содержимое коммита Git (метаданные и дифф). Только чтение. Параметр `ref` — \
             хеш/ссылка коммита (по умолчанию HEAD). Используй, чтобы изучить конкретное изменение.",
            vec![ToolParameter::new("ref", ParamType::String, "Хеш или ссылка коммита (по умолчанию HEAD)", false)],
        )
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let r = call.arg_str("ref").unwrap_or_else(|| "HEAD".to_string());
        if r.contains("..") || r.starts_with('-') {
            return ToolResult::rejected("git_show", "недопустимая ссылка");
        }
        self.ctx.git("git_show", &["show", "--stat", &r]).await
    }
}

/// `git_grep` — поиск по содержимому отслеживаемых файлов.
pub struct GitGrepTool {
    ctx: GitContext,
}
impl Default for GitGrepTool {
    fn default() -> Self {
        Self {
            ctx: GitContext::new(),
        }
    }
}

#[async_trait]
impl Tool for GitGrepTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_grep",
            "Искать строку/паттерн по отслеживаемым в Git файлам (`git grep -n`). Только чтение. \
             Быстрее обхода файлов вручную. Параметр `pattern` обязателен.",
            vec![ToolParameter::new(
                "pattern",
                ParamType::String,
                "Искомая строка или regex",
                true,
            )],
        )
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(pattern) = call.arg_str("pattern") else {
            return ToolResult::rejected("git_grep", "не передан `pattern`");
        };
        self.ctx
            .git("git_grep", &["grep", "-n", "-e", &pattern])
            .await
    }
}

/// `git_add` — добавить пути в индекс (staging).
pub struct GitAddTool {
    ctx: GitContext,
}
impl Default for GitAddTool {
    fn default() -> Self {
        Self {
            ctx: GitContext::new(),
        }
    }
}

#[async_trait]
impl Tool for GitAddTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_add",
            "Добавить изменения в индекс Git (`git add`). Изменяет состояние репозитория. Параметр \
             `path` — путь/паттерн относительно корня (по умолчанию все изменения `-A`).",
            vec![ToolParameter::new("path", ParamType::String, "Путь для staging (по умолчанию все)", false)],
        )
        .write()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        match call.arg_str("path") {
            Some(path) if !path.contains("..") => {
                self.ctx.git("git_add", &["add", "--", &path]).await
            }
            Some(_) => ToolResult::rejected("git_add", "путь не должен содержать `..`"),
            None => self.ctx.git("git_add", &["add", "-A"]).await,
        }
    }
}

/// `git_commit` — создать коммит из проиндексированных изменений.
pub struct GitCommitTool {
    ctx: GitContext,
}
impl Default for GitCommitTool {
    fn default() -> Self {
        Self {
            ctx: GitContext::new(),
        }
    }
}

#[async_trait]
impl Tool for GitCommitTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_commit",
            "Создать коммит Git из проиндексированных изменений (`git commit -m`). Изменяет историю \
             репозитория. Параметр `message` обязателен. Не делает push.",
            vec![ToolParameter::new("message", ParamType::String, "Сообщение коммита", true)],
        )
        .write()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(message) = call.arg_str("message") else {
            return ToolResult::rejected("git_commit", "не передан `message`");
        };
        self.ctx
            .git("git_commit", &["commit", "-m", &message])
            .await
    }
}
