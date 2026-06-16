//! Реестр инструментов: хранение, схемы для модели, диспетчеризация с учётом
//! режима работы и разрешений ([`herta_core::Policy`]).
//!
//! Политика живёт за `Mutex` и разделяется: TUI меняет режим и разрешения через
//! `&ToolRegistry`, пока tool-loop исполняется. Видимость инструментов (`specs`)
//! и исполнение (`dispatch`) согласованы с текущим режимом.

use async_trait::async_trait;
use herta_core::{AgentMode, Permission, Policy, ToolCall, ToolResult, ToolSpec};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Один исполняемый инструмент.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Схема инструмента для модели (включая уровень риска).
    fn spec(&self) -> ToolSpec;

    /// Выполнить вызов. Реализация не должна паниковать — только `ToolResult`.
    async fn call(&self, call: &ToolCall) -> ToolResult;
}

/// Набор зарегистрированных инструментов с политикой доступа.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    policy: Arc<Mutex<Policy>>,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("mode", &self.mode().as_str())
            .finish()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Новый реестр в безопасном режиме по умолчанию (`Auto`).
    pub fn new() -> Self {
        Self::with_mode(AgentMode::Auto)
    }

    pub fn with_mode(mode: AgentMode) -> Self {
        Self {
            tools: HashMap::new(),
            policy: Arc::new(Mutex::new(Policy::new(mode))),
        }
    }

    /// Зарегистрировать инструмент. Дубликаты по имени перезаписываются.
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> &mut Self {
        self.tools.insert(tool.spec().name, tool);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    // --- управление политикой ---

    pub fn mode(&self) -> AgentMode {
        self.policy.lock().expect("policy mutex").mode
    }

    pub fn set_mode(&self, mode: AgentMode) {
        self.policy.lock().expect("policy mutex").mode = mode;
    }

    pub fn allow_tool(&self, name: &str) {
        self.policy
            .lock()
            .expect("policy mutex")
            .ledger
            .allow_tool(name);
    }

    pub fn deny_tool(&self, name: &str) {
        self.policy
            .lock()
            .expect("policy mutex")
            .ledger
            .deny_tool(name);
    }

    pub fn allow_everything(&self) {
        self.policy
            .lock()
            .expect("policy mutex")
            .ledger
            .allow_everything();
    }

    pub fn reset_permissions(&self) {
        self.policy.lock().expect("policy mutex").ledger.clear();
    }

    /// Схемы инструментов, видимых модели в текущем режиме.
    pub fn specs(&self) -> Vec<ToolSpec> {
        let policy = self.policy.lock().expect("policy mutex");
        self.tools
            .values()
            .map(|t| t.spec())
            .filter(|s| policy.is_visible(s.risk))
            .collect()
    }

    /// Все схемы без фильтра по режиму (для команды /tools).
    pub fn all_specs(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|t| t.spec()).collect()
    }

    /// Выполнить вызов по имени с учётом политики.
    pub async fn dispatch(&self, call: &ToolCall) -> ToolResult {
        let Some(tool) = self.tools.get(&call.name) else {
            return ToolResult::rejected(
                call.name.clone(),
                format!("инструмент `{}` не найден", call.name),
            );
        };
        let spec = tool.spec();

        // Решение принимается под кратким локом, без удержания на await.
        let decision = {
            let policy = self.policy.lock().expect("policy mutex");
            policy.decide(&spec.name, spec.risk)
        };

        match decision {
            Permission::Allow => tool.call(call).await,
            Permission::Confirm => ToolResult::rejected(
                spec.name.clone(),
                format!(
                    "требуется разрешение. Одобрите: /allow {} (или /mode full-auto)",
                    spec.name
                ),
            ),
            Permission::Deny => ToolResult::rejected(
                spec.name.clone(),
                format!("запрещено в режиме {}", self.mode().as_str()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use herta_core::{ParamType, ToolParameter};
    use serde_json::json;

    struct Echo;
    #[async_trait]
    impl Tool for Echo {
        fn spec(&self) -> ToolSpec {
            ToolSpec::new(
                "echo",
                "вернуть текст",
                vec![ToolParameter::new("text", ParamType::String, "что", true)],
            )
        }
        async fn call(&self, call: &ToolCall) -> ToolResult {
            ToolResult::ok("echo", call.arg_str("text").unwrap_or_default())
        }
    }

    struct Nuke;
    #[async_trait]
    impl Tool for Nuke {
        fn spec(&self) -> ToolSpec {
            ToolSpec::new("nuke", "опасное", vec![]).destructive()
        }
        async fn call(&self, _call: &ToolCall) -> ToolResult {
            ToolResult::ok("nuke", "выполнено")
        }
    }

    #[tokio::test]
    async fn auto_mode_allows_read_blocks_dangerous() {
        let mut reg = ToolRegistry::with_mode(AgentMode::Auto);
        reg.register(Arc::new(Echo)).register(Arc::new(Nuke));

        let echo = reg
            .dispatch(&ToolCall {
                id: "1".into(),
                name: "echo".into(),
                arguments: json!({"text":"привет"}),
            })
            .await;
        assert!(echo.executed);

        let nuke = reg
            .dispatch(&ToolCall {
                id: "2".into(),
                name: "nuke".into(),
                arguments: json!({}),
            })
            .await;
        assert!(!nuke.executed); // dangerous запрещён в auto

        // Опасный инструмент не виден модели в auto.
        assert!(reg.specs().iter().all(|s| s.name != "nuke"));
    }

    #[tokio::test]
    async fn ledger_allow_enables_dangerous() {
        let mut reg = ToolRegistry::with_mode(AgentMode::Code);
        reg.register(Arc::new(Nuke));
        // В code опасное требует подтверждения.
        let r = reg
            .dispatch(&ToolCall {
                id: "1".into(),
                name: "nuke".into(),
                arguments: json!({}),
            })
            .await;
        assert!(!r.executed);
        reg.allow_tool("nuke");
        let r = reg
            .dispatch(&ToolCall {
                id: "2".into(),
                name: "nuke".into(),
                arguments: json!({}),
            })
            .await;
        assert!(r.executed);
    }
}
