//! Git-инструменты, изменяющие состояние репозитория.

use crate::git::GitContext;
use crate::registry::Tool;
use async_trait::async_trait;
use whatcode_core::{ParamType, ToolCall, ToolParameter, ToolResult, ToolSpec};

/// `git_add` — добавить пути в индекс (staging).
#[derive(Default, Clone)]
pub struct GitAddTool {
    ctx: GitContext,
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
            Some(path) if GitContext::safe_arg(&path) => {
                self.ctx.git("git_add", &["add", "--", &path]).await
            }
            Some(_) => ToolResult::rejected("git_add", "путь не должен содержать `..` или начинаться с `-`"),
            None => self.ctx.git("git_add", &["add", "-A"]).await,
        }
    }
}

/// `git_reset_head` — убрать изменения из индекса, но оставить в рабочем дереве.
#[derive(Default, Clone)]
pub struct GitResetHeadTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitResetHeadTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_reset_head",
            "Убрать изменения из индекса Git, но оставить их в рабочем дереве (`git reset HEAD`). \
             Полезно, если в staged попало лишнее. Параметр `path` ограничивает сброс одним файлом.",
            vec![ToolParameter::new("path", ParamType::String, "Путь для сброса staged (по умолчанию все)", false)],
        )
        .write()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        match call.arg_str("path") {
            Some(path) if GitContext::safe_arg(&path) => {
                self.ctx.git("git_reset_head", &["reset", "HEAD", "--", &path]).await
            }
            Some(_) => ToolResult::rejected("git_reset_head", "недопустимый путь"),
            None => self.ctx.git("git_reset_head", &["reset", "HEAD"]).await,
        }
    }
}

/// `git_commit` — создать коммит из проиндексированных изменений.
#[derive(Default, Clone)]
pub struct GitCommitTool {
    ctx: GitContext,
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

/// `git_push` — отправить текущую/указанную ветку на удалённый репозиторий.
#[derive(Default, Clone)]
pub struct GitPushTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitPushTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_push",
            "Отправить ветку в удалённый репозиторий Git (`git push`). Опасная операция: меняет \
             удалённую историю. Параметр `remote` (по умолчанию origin) и `branch` (по умолчанию текущая). \
             Используй с осторожностью.",
            vec![
                ToolParameter::new("remote", ParamType::String, "Имя remote (по умолчанию origin)", false),
                ToolParameter::new("branch", ParamType::String, "Имя ветки (по умолчанию текущая)", false),
            ],
        )
        .destructive()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let remote = call.arg_str("remote").unwrap_or_else(|| "origin".to_string());
        let branch = call.arg_str("branch").unwrap_or_default();
        if !GitContext::safe_arg(&remote) {
            return ToolResult::rejected("git_push", "недопустимое имя remote");
        }
        if branch.is_empty() {
            self.ctx.git_long("git_push", &["push", &remote]).await
        } else if GitContext::safe_arg(&branch) {
            self.ctx.git_long("git_push", &["push", &remote, &branch]).await
        } else {
            ToolResult::rejected("git_push", "недопустимое имя ветки")
        }
    }
}

/// `git_pull` — получить изменения из удалённого репозитория.
#[derive(Default, Clone)]
pub struct GitPullTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitPullTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_pull",
            "Получить изменения из удалённого репозитория Git (`git pull`). Может вызвать merge. \
             Параметр `remote` (по умолчанию origin) и `branch` (по умолчанию текущая).",
            vec![
                ToolParameter::new("remote", ParamType::String, "Имя remote (по умолчанию origin)", false),
                ToolParameter::new("branch", ParamType::String, "Имя ветки (по умолчанию текущая)", false),
            ],
        )
        .destructive()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let remote = call.arg_str("remote").unwrap_or_else(|| "origin".to_string());
        let branch = call.arg_str("branch").unwrap_or_default();
        if !GitContext::safe_arg(&remote) {
            return ToolResult::rejected("git_pull", "недопустимое имя remote");
        }
        if branch.is_empty() {
            self.ctx.git_long("git_pull", &["pull", &remote]).await
        } else if GitContext::safe_arg(&branch) {
            self.ctx.git_long("git_pull", &["pull", &remote, &branch]).await
        } else {
            ToolResult::rejected("git_pull", "недопустимое имя ветки")
        }
    }
}

/// `git_checkout` — переключить ветку или создать новую.
#[derive(Default, Clone)]
pub struct GitCheckoutTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitCheckoutTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_checkout",
            "Переключить ветку Git (`git checkout`) или создать новую (`-b`). Может привести к \
             изменениям в рабочем дереве. Параметр `branch` обязателен; `create=true` создаёт ветку.",
            vec![
                ToolParameter::new("branch", ParamType::String, "Имя ветки", true),
                ToolParameter::new("create", ParamType::Boolean, "Создать ветку (-b)", false),
                ToolParameter::new("start_point", ParamType::String, "Ответвиться от (хеш/ветка, по умолчанию HEAD)", false),
            ],
        )
        .write()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(branch) = call.arg_str("branch") else {
            return ToolResult::rejected("git_checkout", "не передан `branch`");
        };
        if !GitContext::safe_arg(&branch) {
            return ToolResult::rejected("git_checkout", "недопустимое имя ветки");
        }
        let create = call
            .arguments
            .get("create")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let start_point = call.arg_str("start_point").unwrap_or_default();

        let args = if create {
            if start_point.is_empty() {
                vec!["checkout", "-b", &branch]
            } else if GitContext::safe_arg(&start_point) {
                vec!["checkout", "-b", &branch, &start_point]
            } else {
                return ToolResult::rejected("git_checkout", "недопустимая start_point");
            }
        } else {
            vec!["checkout", &branch]
        };
        self.ctx.git("git_checkout", &args).await
    }
}

/// `git_stash` — спрятать/вернуть изменения.
#[derive(Default, Clone)]
pub struct GitStashTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitStashTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_stash",
            "Управлять stash Git. `action` — push (спрятать), pop (вернуть), list, drop, clear. \
             Параметр `message` только для push.",
            vec![
                ToolParameter::new("action", ParamType::String, "push | pop | list | drop | clear", true),
                ToolParameter::new("message", ParamType::String, "Сообщение stash (только для push)", false),
                ToolParameter::new("index", ParamType::Integer, "Индекс stash (для pop/drop)", false),
            ],
        )
        .write()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(action) = call.arg_str("action") else {
            return ToolResult::rejected("git_stash", "не передан `action`");
        };
        match action.as_str() {
            "push" => {
                let msg = call.arg_str("message").unwrap_or_default();
                if msg.is_empty() {
                    self.ctx.git("git_stash", &["stash", "push"]).await
                } else {
                    self.ctx.git("git_stash", &["stash", "push", "-m", &msg]).await
                }
            }
            "pop" => {
                let idx = call
                    .arguments
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|i| format!("stash@{{{i}}}"))
                    .unwrap_or_else(|| "stash@{0}".to_string());
                self.ctx.git("git_stash", &["stash", "pop", &idx]).await
            }
            "list" => self.ctx.git("git_stash", &["stash", "list"]).await,
            "drop" => {
                let idx = call
                    .arguments
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|i| format!("stash@{{{i}}}"))
                    .unwrap_or_else(|| "stash@{0}".to_string());
                self.ctx.git("git_stash", &["stash", "drop", &idx]).await
            }
            "clear" => self.ctx.git("git_stash", &["stash", "clear"]).await,
            _ => ToolResult::rejected("git_stash", "неизвестный action: push | pop | list | drop | clear"),
        }
    }
}

/// `git_reset` — сбросить текущий HEAD до указанного состояния.
#[derive(Default, Clone)]
pub struct GitResetTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitResetTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_reset",
            "Сбросить текущий HEAD до указанного коммита (`git reset`). ОПАСНО: может изменить \
             историю. `mode` — soft (сохранить индекс+дерево), mixed (по умолчанию), hard (всё сбросить). \
             `ref` — хеш/ссылка (по умолчанию HEAD~1). Используй крайне осторожно.",
            vec![
                ToolParameter::new("ref", ParamType::String, "Хеш/ссылка (по умолчанию HEAD~1)", false),
                ToolParameter::new("mode", ParamType::String, "soft | mixed | hard (по умолчанию mixed)", false),
            ],
        )
        .destructive()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let r = call.arg_str("ref").unwrap_or_else(|| "HEAD~1".to_string());
        let mode = call.arg_str("mode").unwrap_or_else(|| "mixed".to_string());
        if !GitContext::safe_arg(&r) {
            return ToolResult::rejected("git_reset", "недопустимая ссылка");
        }
        let mode_flag = match mode.as_str() {
            "soft" => "--soft",
            "mixed" => "--mixed",
            "hard" => "--hard",
            _ => return ToolResult::rejected("git_reset", "mode: soft | mixed | hard"),
        };
        self.ctx.git("git_reset", &["reset", mode_flag, &r]).await
    }
}

/// `git_revert` — создать коммит, отменяющий указанный.
#[derive(Default, Clone)]
pub struct GitRevertTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitRevertTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_revert",
            "Создать новый коммит, отменяющий изменения указанного коммита (`git revert`). Безопасная \
             альтернатива reset для публичной истории. Параметр `ref` обязателен.",
            vec![ToolParameter::new("ref", ParamType::String, "Хеш/ссылка коммита для отмены", true)],
        )
        .write()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(r) = call.arg_str("ref") else {
            return ToolResult::rejected("git_revert", "не передан `ref`");
        };
        if !GitContext::safe_arg(&r) {
            return ToolResult::rejected("git_revert", "недопустимая ссылка");
        }
        self.ctx.git("git_revert", &["revert", "--no-edit", &r]).await
    }
}

/// `git_rebase` — перебазировать текущую ветку.
#[derive(Default, Clone)]
pub struct GitRebaseTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitRebaseTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_rebase",
            "Перебазировать текущую ветку на другую (`git rebase`). ОПАСНО: меняет историю. \
             Параметр `onto` — целевая ветка/коммит (обязателен). `interactive=true` включает `-i`.",
            vec![
                ToolParameter::new("onto", ParamType::String, "Целевая ветка или коммит", true),
                ToolParameter::new("interactive", ParamType::Boolean, "Интерактивный rebase", false),
            ],
        )
        .destructive()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(onto) = call.arg_str("onto") else {
            return ToolResult::rejected("git_rebase", "не передан `onto`");
        };
        if !GitContext::safe_arg(&onto) {
            return ToolResult::rejected("git_rebase", "недопустимая ссылка");
        }
        let interactive = call
            .arguments
            .get("interactive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if interactive {
            // Интерактивный rebase в TUI-ассистенте не поддерживается автоматически;
            // возвращаем команду для ручного запуска.
            ToolResult::ok(
                "git_rebase",
                format!("Интерактивный rebase требует ручного редактирования. Запустите: git rebase -i {onto}")
            )
        } else {
            self.ctx.git_long("git_rebase", &["rebase", &onto]).await
        }
    }
}

/// `git_cherry_pick` — применить коммит(ы) на текущую ветку.
#[derive(Default, Clone)]
pub struct GitCherryPickTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitCherryPickTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_cherry_pick",
            "Применить изменения указанного коммита на текущую ветку (`git cherry-pick`). \
             Параметр `ref` обязателен. Может вызвать конфликт.",
            vec![ToolParameter::new("ref", ParamType::String, "Хеш/ссылка коммита", true)],
        )
        .write()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(r) = call.arg_str("ref") else {
            return ToolResult::rejected("git_cherry_pick", "не передан `ref`");
        };
        if !GitContext::safe_arg(&r) {
            return ToolResult::rejected("git_cherry_pick", "недопустимая ссылка");
        }
        self.ctx.git("git_cherry_pick", &["cherry-pick", &r]).await
    }
}

/// `git_clean` — удалить неотслеживаемые файлы.
#[derive(Default, Clone)]
pub struct GitCleanTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitCleanTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_clean",
            "Удалить неотслеживаемые файлы Git (`git clean`). ОПАСНО: безвозвратно удаляет файлы. \
             Параметр `dry_run=true` показывает, что будет удалено, без реального удаления. \
             По умолчанию dry_run=true.",
            vec![ToolParameter::new("dry_run", ParamType::Boolean, "Показать, что будет удалено (true по умолчанию)", false)],
        )
        .destructive()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let dry_run = call
            .arguments
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if dry_run {
            self.ctx.git("git_clean", &["clean", "-fdn"]).await
        } else {
            self.ctx.git("git_clean", &["clean", "-fd"]).await
        }
    }
}

/// `git_merge` — слить ветку в текущую.
#[derive(Default, Clone)]
pub struct GitMergeTool {
    ctx: GitContext,
}

#[async_trait]
impl Tool for GitMergeTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "git_merge",
            "Слить указанную ветку в текущую (`git merge`). Может вызвать конфликт. \
             Параметр `branch` обязателен. `no_ff=true` форсирует merge-коммит.",
            vec![
                ToolParameter::new("branch", ParamType::String, "Имя ветки для слияния", true),
                ToolParameter::new("no_ff", ParamType::Boolean, "Создать merge-коммит", false),
            ],
        )
        .write()
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(branch) = call.arg_str("branch") else {
            return ToolResult::rejected("git_merge", "не передан `branch`");
        };
        if !GitContext::safe_arg(&branch) {
            return ToolResult::rejected("git_merge", "недопустимое имя ветки");
        }
        let no_ff = call
            .arguments
            .get("no_ff")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if no_ff {
            self.ctx.git("git_merge", &["merge", "--no-ff", &branch]).await
        } else {
            self.ctx.git("git_merge", &["merge", &branch]).await
        }
    }
}
