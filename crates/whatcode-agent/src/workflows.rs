//! Встроенные мульти-агентные воркфлоу.
//!
//! Воркфлоу — это именованный шаблон, который разворачивается в набор задач для
//! саб-агентов («марионеток»). Задачи запускаются пачкой через [`Supervisor`],
//! исполняются параллельно (в пределах семафора) и стримят события в TUI. Это
//! даёт быстрый веер специализированных проходов: ревью по измерениям, набор
//! независимых планов, разные углы исследования и т. п.
//!
//! Дизайн намеренно простой и без состояния: каждый этап — самостоятельный
//! промпт над одним и тем же вводом. Синтез результатов делает основная модель,
//! читая вывод марионеток в ленте.

use crate::AgentTask;

/// Один этап воркфлоу: короткая метка и промпт-шаблон с плейсхолдером `{input}`.
#[derive(Debug, Clone)]
pub struct WorkflowStage {
    pub label: &'static str,
    pub prompt_template: &'static str,
}

/// Описание встроенного воркфлоу.
#[derive(Debug, Clone)]
pub struct WorkflowSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub stages: &'static [WorkflowStage],
}

impl WorkflowSpec {
    /// Развернуть воркфлоу в задачи саб-агентов, подставив ввод пользователя.
    pub fn expand(&self, input: &str) -> Vec<AgentTask> {
        let input = if input.trim().is_empty() {
            "(контекст текущего проекта и последнего обсуждения)"
        } else {
            input.trim()
        };
        self.stages
            .iter()
            .map(|s| {
                let prompt = s.prompt_template.replace("{input}", input);
                AgentTask::new(format!("{}: {}", self.id, s.label), prompt)
            })
            .collect()
    }
}

/// Все встроенные воркфлоу.
pub const WORKFLOWS: &[WorkflowSpec] = &[
    WorkflowSpec {
        id: "review",
        name: "Код-ревью по измерениям",
        description: "Веер ревьюеров: корректность, безопасность, производительность, стиль.",
        stages: &[
            WorkflowStage {
                label: "корректность",
                prompt_template: "Проведи ревью на корректность и логические баги. Объект ревью: {input}. \
                    Дай список конкретных проблем с местами и предложениями исправления. Только реальные дефекты.",
            },
            WorkflowStage {
                label: "безопасность",
                prompt_template: "Проведи ревью безопасности: инъекции, небезопасные вызовы, утечки, \
                    валидация ввода, обработка секретов. Объект: {input}. Дай конкретные находки и риски.",
            },
            WorkflowStage {
                label: "производительность",
                prompt_template: "Проведи ревью производительности: сложность алгоритмов, лишние аллокации, \
                    IO в циклах, блокировки. Объект: {input}. Дай конкретные узкие места и оптимизации.",
            },
            WorkflowStage {
                label: "стиль",
                prompt_template: "Проведи ревью читаемости и стиля: именование, модульность, типизация, дублирование. \
                    Объект: {input}. Дай короткий список улучшений без придирок ради придирок.",
            },
        ],
    },
    WorkflowSpec {
        id: "plan",
        name: "Панель планов",
        description: "Три независимых подхода к задаче под разными углами.",
        stages: &[
            WorkflowStage {
                label: "MVP-first",
                prompt_template: "Составь план реализации задачи максимально просто и быстро (MVP-first). \
                    Задача: {input}. Дай пошаговый план и укажи риски срезанных углов.",
            },
            WorkflowStage {
                label: "risk-first",
                prompt_template: "Составь план реализации, начиная с самых рискованных/неопределённых частей (risk-first). \
                    Задача: {input}. Дай пошаговый план и точки проверки гипотез.",
            },
            WorkflowStage {
                label: "quality-first",
                prompt_template: "Составь план реализации с упором на архитектуру, тестируемость и долгосрочное качество. \
                    Задача: {input}. Дай пошаговый план и ключевые инварианты.",
            },
        ],
    },
    WorkflowSpec {
        id: "research",
        name: "Многоугловое исследование",
        description: "Параллельные проходы: обзор, аналоги, подводные камни.",
        stages: &[
            WorkflowStage {
                label: "обзор",
                prompt_template: "Дай сжатый технический обзор темы: ключевые понятия и текущее состояние. Тема: {input}.",
            },
            WorkflowStage {
                label: "аналоги",
                prompt_template: "Найди и сравни существующие подходы/библиотеки/паттерны по теме: {input}. \
                    Плюсы, минусы, когда что уместно.",
            },
            WorkflowStage {
                label: "риски",
                prompt_template: "Перечисли подводные камни, типичные ошибки и ограничения по теме: {input}. \
                    Как их избежать.",
            },
        ],
    },
    WorkflowSpec {
        id: "debug",
        name: "Отладочный конвейер",
        description: "Гипотезы о причине, способы воспроизведения и стратегия фикса.",
        stages: &[
            WorkflowStage {
                label: "гипотезы",
                prompt_template: "Сформулируй 3-5 наиболее вероятных причин проблемы, ранжируй по вероятности. Проблема: {input}.",
            },
            WorkflowStage {
                label: "воспроизведение",
                prompt_template: "Предложи минимальные шаги и проверки, чтобы воспроизвести и локализовать проблему: {input}.",
            },
            WorkflowStage {
                label: "фикс",
                prompt_template: "Предложи стратегию исправления и как убедиться, что фикс не сломал остальное. Проблема: {input}.",
            },
        ],
    },
];

/// Найти воркфлоу по id.
pub fn find(id: &str) -> Option<&'static WorkflowSpec> {
    let id = id.trim().to_lowercase();
    WORKFLOWS.iter().find(|w| w.id == id)
}

/// Компактный список воркфлоу для TUI `/workflows`.
pub fn listing() -> String {
    WORKFLOWS
        .iter()
        .map(|w| format!("• {} — {} ({} этапов)", w.id, w.description, w.stages.len()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_known_workflow() {
        assert!(find("review").is_some());
        assert!(find("REVIEW").is_some());
        assert!(find("nope").is_none());
    }

    #[test]
    fn expand_substitutes_input_into_every_stage() {
        let wf = find("review").unwrap();
        let tasks = wf.expand("файл src/main.rs");
        assert_eq!(tasks.len(), wf.stages.len());
        assert!(tasks.iter().all(|t| t.prompt.contains("файл src/main.rs")));
        assert!(tasks.iter().all(|t| !t.prompt.contains("{input}")));
    }

    #[test]
    fn expand_uses_fallback_for_empty_input() {
        let wf = find("plan").unwrap();
        let tasks = wf.expand("   ");
        assert!(tasks.iter().all(|t| !t.prompt.contains("{input}")));
    }

    #[test]
    fn listing_mentions_all_workflows() {
        let l = listing();
        for w in WORKFLOWS {
            assert!(l.contains(w.id));
        }
    }
}
