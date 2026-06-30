//! Git-инструменты только для чтения.

use crate::git::GitContext;
use crate::registry::Tool;
use async_trait::async_trait;
use whatcode_core::{ParamType, ToolCall, ToolParameter, ToolResult, ToolSpec};

/// `git_status` — рабочее дерево в кратком формате.
#[derive(Default, Clone)]
pub struct GitStatusTool {
    ctx: GitContext,
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
#[derive(Default, Clone)]
pub struct GitLogTool {
    ctx: GitContext,
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
#[derive(Default, Clone)]
pub struct GitDiffTool {
    ctx: GitContext,
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
                if !GitContext::safe_arg(&path) {
                    return ToolResult::rejected("git_diff", "путь не должен содержать `..` или начинаться с `-`");
                }
                self.ctx.git("git_diff", &["diff", "--", &path]).await
            }
            None => self.ctx.git("git_diff", &["diff"]).await,
        }
    }
}

/// `git_diff_staged` — изменения в индексе.
#[derive(Default, Clone)]
pub struct GitDiffStagedTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitDiffStagedTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_diff_staged",
            "Показать изменения, уже добавленные в индекс Git (`git diff --staged`). Только чтение. \
             Используй перед коммитом, чтобы убедиться, что в staged попало именно то, что нужно.",
            vec![],
        )
    }
    async fn call(&self, _call: &ToolCall) -> ToolResult {
        self.ctx.git("git_diff_staged", &["diff", "--staged"]).await
    }
}

/// `git_branches` — текущая ветка и список локальных веток.
#[derive(Default, Clone)]
pub struct GitBranchTool {
    ctx: GitContext,
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

/// `git_branch_remote` — локальные и удалённые ветки с их состоянием.
#[derive(Default, Clone)]
pub struct GitBranchRemoteTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitBranchRemoteTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_branch_remote",
            "Показать локальные и удалённые ветки Git (`git branch -a -vv`). Только чтение. \
             Показывает, какая ветка от какой удалённой отслеживается и насколько она впереди/позади.",
            vec![],
        )
    }
    async fn call(&self, _call: &ToolCall) -> ToolResult {
        self.ctx.git("git_branch_remote", &["branch", "-a", "-vv"]).await
    }
}

/// `git_show` — показать конкретный коммит (по умолчанию HEAD).
#[derive(Default, Clone)]
pub struct GitShowTool {
    ctx: GitContext,
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
        if !GitContext::safe_arg(&r) {
            return ToolResult::rejected("git_show", "недопустимая ссылка");
        }
        self.ctx.git("git_show", &["show", "--stat", &r]).await
    }
}

/// `git_grep` — поиск по содержимому отслеживаемых файлов.
#[derive(Default, Clone)]
pub struct GitGrepTool {
    ctx: GitContext,
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
        if !GitContext::safe_arg(&pattern) {
            return ToolResult::rejected("git_grep", "недопустимый pattern");
        }
        self.ctx
            .git("git_grep", &["grep", "-n", "-e", &pattern])
            .await
    }
}

/// `git_remote` — информация об удалённых репозиториях.
#[derive(Default, Clone)]
pub struct GitRemoteTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitRemoteTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_remote",
            "Показать настроенные удалённые репозитории Git (`git remote -v`). Только чтение. \
             Используй, чтобы узнать, куда можно push/pull.",
            vec![],
        )
    }
    async fn call(&self, _call: &ToolCall) -> ToolResult {
        self.ctx.git("git_remote", &["remote", "-v"]).await
    }
}
