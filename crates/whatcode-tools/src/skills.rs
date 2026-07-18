//! Навыки как инструменты агента (прогрессивное раскрытие).
//!
//! `list_skills` отдаёт каталог (имя/назначение/когда применять), `use_skill`
//! подгружает полное тело конкретного навыка. Так модель держит в контексте
//! только дешёвые карточки и разворачивает инструкции лишь при необходимости —
//! включая навык авто-сжатия контекста.

use crate::registry::Tool;
use async_trait::async_trait;
use std::sync::Arc;
use whatcode_core::{ParamType, Skill, ToolCall, ToolParameter, ToolResult, ToolSpec};

/// Каталог загруженных навыков (иммутабельный после загрузки).
#[derive(Debug, Clone)]
pub struct SkillLibrary {
    skills: Arc<Vec<Skill>>,
}

impl SkillLibrary {
    /// Загрузить навыки из каталога (`*.skill`, `*.whatcode`).
    pub fn load(dir: impl AsRef<std::path::Path>) -> Self {
        Self {
            skills: Arc::new(whatcode_core::load_skills(dir)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn catalog(&self) -> String {
        self.catalog_matching("")
    }

    /// Каталог, отфильтрованный по запросу (имя/назначение/теги). Пустой запрос —
    /// весь каталог.
    pub fn catalog_matching(&self, query: &str) -> String {
        if self.skills.is_empty() {
            return "Навыки не найдены.".to_string();
        }
        let matched: Vec<String> = self
            .skills
            .iter()
            .filter(|s| s.matches(query))
            .map(Skill::summary)
            .collect();
        if matched.is_empty() {
            return format!("Навыки по запросу «{}» не найдены.", query.trim());
        }
        matched.join("\n")
    }

    fn find(&self, name: &str) -> Option<&Skill> {
        self.skills
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name.trim()))
    }
}

/// `list_skills` — перечислить доступные навыки.
pub struct ListSkillsTool {
    lib: SkillLibrary,
}
impl ListSkillsTool {
    pub fn new(lib: SkillLibrary) -> Self {
        Self { lib }
    }
}

#[async_trait]
impl Tool for ListSkillsTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "list_skills",
            "Перечислить доступные навыки ассистента с их назначением, условием применения и тегами. \
             Необязательный параметр `query` фильтрует по имени/назначению/тегам (например, security). \
             Вызови первым делом, если задача похожа на специализированную, чтобы узнать, \
             какой навык загрузить через use_skill.",
            vec![ToolParameter::new(
                "query",
                ParamType::String,
                "Ключевое слово или тег для фильтра (необязательно)",
                false,
            )],
        )
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let query = call.arg_str("query").unwrap_or_default();
        ToolResult::ok("list_skills", self.lib.catalog_matching(&query))
    }
}

/// `use_skill` — загрузить полное тело навыка по имени.
pub struct UseSkillTool {
    lib: SkillLibrary,
}
impl UseSkillTool {
    pub fn new(lib: SkillLibrary) -> Self {
        Self { lib }
    }
}

#[async_trait]
impl Tool for UseSkillTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "use_skill",
            "Загрузить полные инструкции конкретного навыка по его имени (см. list_skills). Возвращает \
             пошаговое руководство, которому нужно следовать. Используй, когда условие применения навыка \
             выполнено — например, навык context-compaction при приближении к лимиту контекста.",
            vec![ToolParameter::new("name", ParamType::String, "Имя навыка из каталога list_skills", true)],
        )
    }
    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(name) = call.arg_str("name") else {
            return ToolResult::rejected("use_skill", "не передано `name`");
        };
        match self.lib.find(&name) {
            Some(skill) => ToolResult::ok(
                "use_skill",
                format!("Навык `{}`:\n{}", skill.name, skill.body),
            ),
            None => ToolResult::rejected(
                "use_skill",
                format!("навык `{name}` не найден; вызови list_skills"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_filters_by_tag() {
        let lib = SkillLibrary::load("../../skills");
        assert!(!lib.is_empty(), "репозиторные навыки должны загрузиться");
        let full = lib.catalog();
        assert!(full.contains("security-review"));
        assert!(full.contains("pr-description"));
        // Фильтр по тегу оставляет security-review и отсекает несвязанные навыки.
        let sec = lib.catalog_matching("security");
        assert!(sec.contains("security-review"));
        assert!(!sec.contains("git-workflow"));
        // Пустой результат по несуществующему запросу.
        let none = lib.catalog_matching("нетакогонавыкавообще");
        assert!(none.contains("не найден"));
    }
}
