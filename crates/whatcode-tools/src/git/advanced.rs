//! Полуавтоматические Git-workflows.
//!
//! Каждый инструмент здесь выполняет последовательность git-команд, но строго
//! контролирует аргументы и возвращает результат в виде, понятном LLM.

use crate::git::GitContext;
use crate::registry::Tool;
use async_trait::async_trait;
use whatcode_core::{ParamType, ToolCall, ToolParameter, ToolResult, ToolSpec};

/// `git_rollback_commit` — безопасно откатить последний коммит.
///
/// Если `mode` = `soft` (по умолчанию), изменения возвращаются в индекс.
/// Если `mode` = `mixed`, изменения остаются в рабочем дереве, но не в индексе.
/// Если `mode` = `hard`, изменения удаляются.
#[derive(Default, Clone)]
pub struct GitRollbackCommitTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitRollbackCommitTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_rollback_commit",
            "Безопасно откатить последний локальный коммит. По умолчанию `soft` — изменения \
             сохраняются в индексе, чтобы можно было быстро перекоммитить. Для публичной истории \
             используй `git_revert` вместо этого. `mode` = soft | mixed | hard.",
            vec![ToolParameter::new(
                "mode",
                ParamType::String,
                "soft | mixed | hard (по умолчанию soft)",
                false,
            )],
        )
        .destructive()
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let mode = call.arg_str("mode").unwrap_or_else(|| "soft".to_string());
        let flag = match mode.as_str() {
            "soft" => "--soft",
            "mixed" => "--mixed",
            "hard" => "--hard",
            _ => return ToolResult::rejected("git_rollback_commit", "mode: soft | mixed | hard"),
        };

        let step1 = self
            .ctx
            .git("git_rollback_commit", &["reset", flag, "HEAD~1"])
            .await;
        if !step1.executed {
            return step1;
        }
        let step2 = self
            .ctx
            .git("git_rollback_commit", &["status", "--short"])
            .await;
        ToolResult::ok(
            "git_rollback_commit",
            format!(
                "Последний коммит откачен ({}). Текущее состояние:\n{}",
                mode, step2.message
            ),
        )
    }
}

/// `git_sync_branch` — получить изменения из remote и перебазировать текущую ветку.
#[derive(Default, Clone)]
pub struct GitSyncBranchTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitSyncBranchTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_sync_branch",
            "Получить изменения из удалённого репозитория и перебазировать текущую ветку на \
             upstream (`git pull --rebase`). Параметр `remote` (по умолчанию origin). \
             ОПАСНО: может изменить историю локальной ветки.",
            vec![ToolParameter::new(
                "remote",
                ParamType::String,
                "Имя remote (по умолчанию origin)",
                false,
            )],
        )
        .destructive()
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let remote = call
            .arg_str("remote")
            .unwrap_or_else(|| "origin".to_string());
        if !GitContext::safe_arg(&remote) {
            return ToolResult::rejected("git_sync_branch", "недопустимое имя remote");
        }
        self.ctx
            .git_long("git_sync_branch", &["pull", "--rebase", &remote])
            .await
    }
}

/// `git_unstage_all` — убрать все изменения из индекса, сохранив в рабочем дереве.
#[derive(Default, Clone)]
pub struct GitUnstageAllTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitUnstageAllTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_unstage_all",
            "Убрать все изменения из индекса Git, оставив их в рабочем дереве. \
             Эквивалент `git reset HEAD`.",
            vec![],
        )
        .write()
    }

    async fn call(&self, _call: &ToolCall) -> ToolResult {
        self.ctx.git("git_unstage_all", &["reset", "HEAD"]).await
    }
}

/// `git_discard_changes` — откатить изменения в рабочем дереве для указанного пути.
///
/// Если `path` не передан, откатывает всё рабочее дерево (`git checkout -- .`).
#[derive(Default, Clone)]
pub struct GitDiscardChangesTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitDiscardChangesTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_discard_changes",
            "Откатить изменения в рабочем дереве Git. ОПАСНО: безвозвратно теряет правки. \
             `path` ограничивает откат одним файлом; без пути откатывает всё рабочее дерево.",
            vec![ToolParameter::new(
                "path",
                ParamType::String,
                "Путь для отката (по умолчанию всё рабочее дерево)",
                false,
            )],
        )
        .destructive()
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        match call.arg_str("path") {
            Some(path) if GitContext::safe_arg(&path) => {
                self.ctx
                    .git("git_discard_changes", &["checkout", "--", &path])
                    .await
            }
            Some(_) => ToolResult::rejected("git_discard_changes", "недопустимый путь"),
            None => {
                self.ctx
                    .git("git_discard_changes", &["checkout", "--", "."])
                    .await
            }
        }
    }
}

/// `git_commit_all` — добавить все изменения и создать коммит одной командой.
#[derive(Default, Clone)]
pub struct GitCommitAllTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitCommitAllTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_commit_all",
            "Добавить все изменения в индекс и создать коммит (`git add -A && git commit -m`). \
             Параметр `message` обязателен. Не делает push.",
            vec![ToolParameter::new(
                "message",
                ParamType::String,
                "Сообщение коммита",
                true,
            )],
        )
        .write()
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(message) = call.arg_str("message") else {
            return ToolResult::rejected("git_commit_all", "не передан `message`");
        };
        let step1 = self.ctx.git("git_commit_all", &["add", "-A"]).await;
        if !step1.executed {
            return step1;
        }
        self.ctx
            .git("git_commit_all", &["commit", "-m", &message])
            .await
    }
}

/// `git_savepoint` — создать stash и коммит WIP для безопасного эксперимента.
#[derive(Default, Clone)]
pub struct GitSavepointTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitSavepointTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_savepoint",
            "Создать точку сохранения: спрятать текущие изменения в stash и показать статус. \
             Полезно перед рискованными операциями (rebase, reset, эксперименты).",
            vec![ToolParameter::new(
                "message",
                ParamType::String,
                "Сообщение stash (по умолчанию WIP-savepoint)",
                false,
            )],
        )
        .write()
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let msg = call
            .arg_str("message")
            .unwrap_or_else(|| "WIP-savepoint".to_string());
        let step1 = self
            .ctx
            .git("git_savepoint", &["stash", "push", "-m", &msg])
            .await;
        if !step1.executed {
            return step1;
        }
        let step2 = self.ctx.git("git_savepoint", &["status", "--short"]).await;
        ToolResult::ok(
            "git_savepoint",
            format!("Сохранено в stash. Текущий статус:\n{}", step2.message),
        )
    }
}
