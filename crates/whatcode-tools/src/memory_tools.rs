//! Инструменты долговременной памяти: `remember`, `recall`, `forget`.
//! Делят один `LongMemoryStore` под асинхронным мьютексом.

use crate::registry::Tool;
use async_trait::async_trait;
use whatcode_core::{
    FactCategory, FactSource, LongMemoryStore, ParamType, ToolCall, ToolParameter, ToolResult,
    ToolSpec,
};
use std::sync::Arc;
use tokio::sync::Mutex;

type Store = Arc<Mutex<LongMemoryStore>>;

/// `remember(content, category)` — сохранить факт.
pub struct RememberTool {
    store: Store,
}

impl RememberTool {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for RememberTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "remember",
            "Сохранить стабильный факт о пользователе, проекте или предпочтениях в долговременную память.",
            vec![
                ToolParameter::new("content", ParamType::String, "Текст факта", true),
                ToolParameter::new("category", ParamType::String, "user | project | preferences | notes", false),
            ],
        )
        .write()
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(content) = call.arg_str("content") else {
            return ToolResult::rejected("remember", "не передан `content`");
        };
        let category = call
            .arg_str("category")
            .map(|c| FactCategory::parse(&c))
            .unwrap_or(FactCategory::Notes);
        let mut store = self.store.lock().await;
        match store.add_fact(&content, category, FactSource::Explicit) {
            Ok(Some(fact)) => ToolResult::ok("remember", format!("Запомнила: {}", fact.content)),
            Ok(None) => ToolResult::ok("remember", "Этот факт мне уже известен."),
            Err(e) => ToolResult::rejected("remember", e.to_string()),
        }
    }
}

/// `recall(category?)` — перечислить факты.
pub struct RecallTool {
    store: Store,
}

impl RecallTool {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for RecallTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "recall",
            "Вспомнить сохранённые факты. Необязательно сузить по категории и/или по подстроке \
             поиска `query` (регистронезависимо). Используй, чтобы достать ранее сохранённый \
             контекст о пользователе или проекте.",
            vec![
                ToolParameter::new(
                    "category",
                    ParamType::String,
                    "user | project | preferences | notes",
                    false,
                ),
                ToolParameter::new(
                    "query",
                    ParamType::String,
                    "Подстрока для поиска по фактам",
                    false,
                ),
            ],
        )
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let store = self.store.lock().await;
        let query = call.arg_str("query").map(|q| q.to_lowercase());

        let mut facts: Vec<&whatcode_core::Fact> = match call.arg_str("category") {
            Some(cat) => store.by_category(FactCategory::parse(&cat)),
            None => store.all_facts().iter().collect(),
        };
        if let Some(q) = &query {
            facts.retain(|f| f.content.to_lowercase().contains(q));
        }

        let listing = facts
            .iter()
            .map(|f| format!("- {}", f.content))
            .collect::<Vec<_>>()
            .join("\n");
        if listing.is_empty() {
            let msg = if query.is_some() {
                "Совпадений не найдено."
            } else {
                "Пока ничего не сохранено."
            };
            ToolResult::ok("recall", msg)
        } else {
            ToolResult::ok("recall", listing)
        }
    }
}

/// `forget(content_match)` — удалить факты по подстроке.
pub struct ForgetTool {
    store: Store,
}

impl ForgetTool {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for ForgetTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "forget",
            "Удалить из памяти факты, содержащие указанную подстроку.",
            vec![ToolParameter::new(
                "content_match",
                ParamType::String,
                "Подстрока для поиска",
                true,
            )],
        )
        .write()
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(needle) = call.arg_str("content_match") else {
            return ToolResult::rejected("forget", "не передан `content_match`");
        };
        let mut store = self.store.lock().await;
        match store.remove_by_content(&needle) {
            Ok(0) => ToolResult::ok("forget", "Совпадений не найдено."),
            Ok(n) => ToolResult::ok("forget", format!("Удалено фактов: {n}")),
            Err(e) => ToolResult::rejected("forget", e.to_string()),
        }
    }
}
