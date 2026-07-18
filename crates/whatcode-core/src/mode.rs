//! Режимы работы агента и модель разрешений инструментов.
//!
//! Режимы (как в Claude Code) задают, какие инструменты видит модель и какие
//! вызовы исполняются автоматически, требуют разрешения или блокируются:
//! - `Chat`     — чистый разговор, инструментов нет;
//! - `Plan`     — только чтение (исследование и планирование), без мутаций;
//! - `Code`     — всё для кода; запись и опасное требуют разрешения;
//! - `Auto`     — чтение и запись автоматически, опасное запрещено;
//! - `FullAuto` — полный доступ ко всем инструментам без подтверждений.
//!
//! Разрешения «одобрить/отклонить все похожие» хранятся в [`PermissionLedger`]
//! по имени инструмента и переопределяют режим.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Уровень риска инструмента.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRisk {
    /// Только чтение, без побочных эффектов.
    ReadOnly,
    /// Изменяет файлы/состояние проекта (запись, коммит, сборка).
    Write,
    /// Потенциально опасно: установка ПО, запуск произвольных команд, сеть с эффектами.
    Dangerous,
}

/// Режим работы агента.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentMode {
    Chat,
    Plan,
    Code,
    Auto,
    FullAuto,
}

impl AgentMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "chat" => Some(Self::Chat),
            "plan" => Some(Self::Plan),
            "code" => Some(Self::Code),
            "auto" => Some(Self::Auto),
            "full-auto" | "full_auto" | "fullauto" | "full" => Some(Self::FullAuto),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Plan => "plan",
            Self::Code => "code",
            Self::Auto => "auto",
            Self::FullAuto => "full-auto",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Chat => "чистый разговор без инструментов",
            Self::Plan => "только чтение: исследование и планирование",
            Self::Code => "разработка кода; запись и опасное — по разрешению",
            Self::Auto => "чтение и запись авто; опасное запрещено",
            Self::FullAuto => "полный доступ ко всем инструментам",
        }
    }

    /// Видна ли модели хоть какая-то часть инструментов.
    pub fn allows_tools(self) -> bool {
        self != Self::Chat
    }

    /// Максимальный уровень риска, который вообще показывается модели в этом режиме.
    /// Инструменты выше этого уровня скрываются из схем.
    pub fn visible_ceiling(self) -> Option<ToolRisk> {
        match self {
            Self::Chat => None,
            Self::Plan => Some(ToolRisk::ReadOnly),
            Self::Code => Some(ToolRisk::Dangerous),
            Self::Auto => Some(ToolRisk::Write),
            Self::FullAuto => Some(ToolRisk::Dangerous),
        }
    }

    fn baseline(self, risk: ToolRisk) -> Permission {
        match (self, risk) {
            (Self::Chat, _) => Permission::Deny,
            (Self::Plan, ToolRisk::ReadOnly) => Permission::Allow,
            (Self::Plan, _) => Permission::Deny,
            (Self::Code, ToolRisk::ReadOnly) => Permission::Allow,
            (Self::Code, _) => Permission::Confirm,
            (Self::Auto, ToolRisk::ReadOnly | ToolRisk::Write) => Permission::Allow,
            (Self::Auto, ToolRisk::Dangerous) => Permission::Deny,
            (Self::FullAuto, _) => Permission::Allow,
        }
    }
}

/// Итоговое решение по конкретному вызову.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Исполнять автоматически.
    Allow,
    /// Требуется явное разрешение пользователя (через ledger / команду).
    Confirm,
    /// Запрещено в этом режиме.
    Deny,
}

/// Запомненные решения «одобрить/отклонить все похожие» по имени инструмента.
#[derive(Debug, Clone, Default)]
pub struct PermissionLedger {
    per_tool: HashMap<String, bool>, // true = allow-all, false = deny-all
    allow_everything: bool,
}

impl PermissionLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Одобрить все будущие вызовы инструмента.
    pub fn allow_tool(&mut self, name: &str) {
        self.per_tool.insert(name.to_string(), true);
    }

    /// Отклонять все будущие вызовы инструмента.
    pub fn deny_tool(&mut self, name: &str) {
        self.per_tool.insert(name.to_string(), false);
    }

    /// Снять явное решение по инструменту (вернуть к режиму).
    pub fn reset_tool(&mut self, name: &str) {
        self.per_tool.remove(name);
    }

    /// Разрешить вообще всё на эту сессию.
    pub fn allow_everything(&mut self) {
        self.allow_everything = true;
    }

    pub fn clear(&mut self) {
        self.per_tool.clear();
        self.allow_everything = false;
    }

    fn lookup(&self, name: &str) -> Option<bool> {
        if self.allow_everything {
            return Some(true);
        }
        self.per_tool.get(name).copied()
    }
}

/// Политика доступа: режим + ledger. Решает судьбу каждого вызова.
#[derive(Debug, Clone)]
pub struct Policy {
    pub mode: AgentMode,
    pub ledger: PermissionLedger,
}

impl Policy {
    pub fn new(mode: AgentMode) -> Self {
        Self {
            mode,
            ledger: PermissionLedger::new(),
        }
    }

    /// Виден ли инструмент данного риска модели в текущем режиме.
    pub fn is_visible(&self, risk: ToolRisk) -> bool {
        match self.mode.visible_ceiling() {
            None => false,
            Some(ceiling) => risk <= ceiling,
        }
    }

    /// Решение по вызову инструмента. Ledger переопределяет режим.
    pub fn decide(&self, tool_name: &str, risk: ToolRisk) -> Permission {
        match self.ledger.lookup(tool_name) {
            Some(true) => Permission::Allow,
            Some(false) => Permission::Deny,
            None => self.mode.baseline(risk),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_parsing() {
        assert_eq!(AgentMode::parse("full-auto"), Some(AgentMode::FullAuto));
        assert_eq!(AgentMode::parse("CODE"), Some(AgentMode::Code));
        assert_eq!(AgentMode::parse("nonsense"), None);
    }

    #[test]
    fn chat_blocks_all_tools() {
        let p = Policy::new(AgentMode::Chat);
        assert!(!p.is_visible(ToolRisk::ReadOnly));
        assert_eq!(p.decide("read_file", ToolRisk::ReadOnly), Permission::Deny);
    }

    #[test]
    fn plan_is_read_only() {
        let p = Policy::new(AgentMode::Plan);
        assert_eq!(p.decide("read_file", ToolRisk::ReadOnly), Permission::Allow);
        assert_eq!(p.decide("write_file", ToolRisk::Write), Permission::Deny);
        assert!(!p.is_visible(ToolRisk::Write));
    }

    #[test]
    fn code_confirms_write_and_dangerous() {
        let p = Policy::new(AgentMode::Code);
        assert_eq!(p.decide("read_file", ToolRisk::ReadOnly), Permission::Allow);
        assert_eq!(p.decide("write_file", ToolRisk::Write), Permission::Confirm);
        assert_eq!(
            p.decide("install_toolchain", ToolRisk::Dangerous),
            Permission::Confirm
        );
    }

    #[test]
    fn auto_blocks_dangerous_allows_write() {
        let p = Policy::new(AgentMode::Auto);
        assert_eq!(p.decide("write_file", ToolRisk::Write), Permission::Allow);
        assert_eq!(
            p.decide("install_toolchain", ToolRisk::Dangerous),
            Permission::Deny
        );
    }

    #[test]
    fn full_auto_allows_everything() {
        let p = Policy::new(AgentMode::FullAuto);
        assert_eq!(
            p.decide("install_toolchain", ToolRisk::Dangerous),
            Permission::Allow
        );
    }

    #[test]
    fn ledger_overrides_mode() {
        let mut p = Policy::new(AgentMode::Code);
        assert_eq!(
            p.decide("cargo_build", ToolRisk::Write),
            Permission::Confirm
        );
        p.ledger.allow_tool("cargo_build");
        assert_eq!(p.decide("cargo_build", ToolRisk::Write), Permission::Allow);
        p.ledger.deny_tool("cargo_build");
        assert_eq!(p.decide("cargo_build", ToolRisk::Write), Permission::Deny);
    }

    #[test]
    fn allow_everything_short_circuits() {
        let mut p = Policy::new(AgentMode::Auto);
        assert_eq!(
            p.decide("install_toolchain", ToolRisk::Dangerous),
            Permission::Deny
        );
        p.ledger.allow_everything();
        assert_eq!(
            p.decide("install_toolchain", ToolRisk::Dangerous),
            Permission::Allow
        );
    }
}
