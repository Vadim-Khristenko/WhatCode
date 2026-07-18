//! Мульти-агентные воркфлоу: многоэтапные пайплайны, а не плоский веер.
//!
//! Вдохновлено «dynamic workflows» Claude Code, но в декларативной форме, чтобы
//! не тащить встроенный скрипт-движок. Воркфлоу — это направленный конвейер:
//!
//! 1. **Producers** — набор промптов над одним вводом, запускаются параллельно
//!    (веер). Каждый — отдельный саб-агент.
//! 2. **Synthesis** (барьер) — один агент получает выводы всех producer-ов и
//!    сводит их в единый результат.
//! 3. **Verify** (опционально) — несколько независимых «скептиков» в свежих
//!    контекстах голосуют, реально ли найденное; при переборе отказов результат
//!    помечается как неподтверждённый.
//!
//! Встроенные воркфлоу задаются в коде; пользовательские грузятся из TOML
//! (`.whatcode/workflows/*.toml` в проекте и `~/.whatcode/workflows/*.toml`).
//!
//! Чистая логика (разбор, сборка промптов, подсчёт голосов) юнит-тестируема без
//! модели; асинхронный исполнитель [`execute`] тонкий и опирается на
//! [`Supervisor`](crate::Supervisor).

use crate::{AgentEvent, AgentTask, Supervisor};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Producer-этап: помеченный промпт-шаблон с плейсхолдером `{input}`.
#[derive(Debug, Clone)]
pub struct ProducerStage {
    pub label: String,
    pub prompt_template: String,
}

/// Конфигурация состязательной проверки.
#[derive(Debug, Clone, Copy)]
pub struct VerifyConfig {
    /// Сколько независимых скептиков голосует.
    pub skeptics: u8,
    /// При скольких «опровергнуто» результат считается неподтверждённым.
    pub reject_threshold: u8,
}

/// Описание воркфлоу.
#[derive(Debug, Clone)]
pub struct WorkflowSpec {
    pub id: String,
    pub name: String,
    pub description: String,
    pub producers: Vec<ProducerStage>,
    /// Шаблон синтеза; плейсхолдеры `{input}` и `{results}`. Если `None` —
    /// выводы producer-ов возвращаются как есть.
    pub synthesis: Option<String>,
    pub verify: Option<VerifyConfig>,
}

impl WorkflowSpec {
    /// Число этапов для отображения (producers + synthesis + verify).
    pub fn stage_count(&self) -> usize {
        self.producers.len()
            + usize::from(self.synthesis.is_some())
            + usize::from(self.verify.is_some())
    }

    /// Развернуть producer-этапы в задачи саб-агентов (без исполнения).
    pub fn producer_tasks(&self, input: &str) -> Vec<AgentTask> {
        let input = normalize_input(input);
        self.producers
            .iter()
            .map(|p| {
                AgentTask::new(
                    format!("{}: {}", self.id, p.label),
                    p.prompt_template.replace("{input}", &input),
                )
            })
            .collect()
    }

    /// Собрать промпт синтеза из выводов producer-ов.
    pub fn synthesis_prompt(&self, input: &str, results: &[(String, String)]) -> Option<String> {
        let tmpl = self.synthesis.as_ref()?;
        let joined = results
            .iter()
            .map(|(label, out)| format!("### {label}\n{out}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        Some(
            tmpl.replace("{input}", &normalize_input(input))
                .replace("{results}", &joined),
        )
    }
}

fn normalize_input(input: &str) -> String {
    if input.trim().is_empty() {
        "(контекст текущего проекта и последнего обсуждения)".to_string()
    } else {
        input.trim().to_string()
    }
}

/// Итог исполнения воркфлоу.
#[derive(Debug, Clone)]
pub struct WorkflowOutcome {
    pub producer_results: Vec<(String, String)>,
    pub synthesis: Option<String>,
    pub verify: Option<VerifyOutcome>,
}

/// Итог голосования скептиков.
#[derive(Debug, Clone)]
pub struct VerifyOutcome {
    pub skeptics: u8,
    pub rejections: u8,
    pub confirmed: bool,
}

impl WorkflowOutcome {
    /// Финальный текст для показа пользователю.
    pub fn final_text(&self) -> String {
        let mut out = if let Some(s) = &self.synthesis {
            s.clone()
        } else {
            self.producer_results
                .iter()
                .map(|(label, o)| format!("### {label}\n{o}"))
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        if let Some(v) = &self.verify {
            let mark = if v.confirmed {
                "подтверждено"
            } else {
                "не подтверждено"
            };
            out.push_str(&format!(
                "\n\n— Проверка: {} ({}/{} скептиков против).",
                mark, v.rejections, v.skeptics
            ));
        }
        out
    }
}

/// Подсчитать голоса скептиков: считаем «опровергнуто», если ответ так начинается
/// или явно это утверждает.
pub fn tally_votes(verdicts: &[String], cfg: VerifyConfig) -> VerifyOutcome {
    let rejections = verdicts.iter().filter(|v| is_refutation(v)).count() as u8;
    VerifyOutcome {
        skeptics: cfg.skeptics,
        rejections,
        confirmed: rejections < cfg.reject_threshold,
    }
}

fn is_refutation(verdict: &str) -> bool {
    let n = verdict.trim().to_lowercase();
    n.starts_with("опроверг")
        || n.starts_with("verdict: refuted")
        || n.starts_with("refuted")
        || n.contains("вердикт: опроверг")
        || n.contains("нереально")
        || n.contains("не подтвержд")
}

/// Промпт для одного скептика (свежий контекст, задача — опровергнуть).
fn skeptic_prompt(finding: &str) -> String {
    format!(
        "Ты независимый скептик. Твоя задача — попытаться ОПРОВЕРГНУТЬ следующий результат, \
         найдя в нём фактические ошибки, необоснованные утверждения или пропущенные детали. \
         Если результат выдерживает критику — подтверди его. Начни ответ строго с одного слова: \
         «ПОДТВЕРЖДЕНО» или «ОПРОВЕРГНУТО», затем одна короткая причина.\n\nРезультат для проверки:\n{finding}"
    )
}

/// Исполнить воркфлоу через супервизор. `progress` (если задан) получает копии
/// событий саб-агентов для отображения в TUI.
pub async fn execute(
    supervisor: &Supervisor,
    spec: &WorkflowSpec,
    input: &str,
    progress: Option<mpsc::UnboundedSender<AgentEvent>>,
) -> WorkflowOutcome {
    // --- этап 1: producers (веер) ---
    let tasks = spec.producer_tasks(input);
    let mut id_to_label: HashMap<String, String> = HashMap::new();
    for (task, p) in tasks.iter().zip(spec.producers.iter()) {
        id_to_label.insert(task.id.clone(), p.label.clone());
    }
    let mut producer_results = run_batch(supervisor, tasks, &id_to_label, &progress).await;
    // Стабильный порядок — как в описании воркфлоу.
    producer_results.sort_by_key(|(label, _)| {
        spec.producers
            .iter()
            .position(|p| &p.label == label)
            .unwrap_or(usize::MAX)
    });

    // --- этап 2: синтез (барьер) ---
    let synthesis = match spec.synthesis_prompt(input, &producer_results) {
        Some(prompt) => {
            let title = format!("{}: синтез", spec.id);
            run_single(supervisor, title, prompt, &progress).await.ok()
        }
        None => None,
    };

    // --- этап 3: проверка (голосование скептиков) ---
    let verify = match spec.verify {
        Some(cfg) if cfg.skeptics > 0 => {
            let finding = synthesis.clone().unwrap_or_else(|| {
                producer_results
                    .iter()
                    .map(|(_, o)| o.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            });
            let tasks: Vec<AgentTask> = (0..cfg.skeptics)
                .map(|i| {
                    AgentTask::new(
                        format!("{}: скептик {}", spec.id, i + 1),
                        skeptic_prompt(&finding),
                    )
                })
                .collect();
            let labels: HashMap<String, String> = tasks
                .iter()
                .map(|t| (t.id.clone(), t.title.clone()))
                .collect();
            let verdicts = run_batch(supervisor, tasks, &labels, &progress).await;
            let verdict_texts: Vec<String> = verdicts.into_iter().map(|(_, o)| o).collect();
            Some(tally_votes(&verdict_texts, cfg))
        }
        _ => None,
    };

    WorkflowOutcome {
        producer_results,
        synthesis,
        verify,
    }
}

/// Запустить пачку задач параллельно, вернуть `(label, output)` по завершении.
async fn run_batch(
    supervisor: &Supervisor,
    tasks: Vec<AgentTask>,
    id_to_label: &HashMap<String, String>,
    progress: &Option<mpsc::UnboundedSender<AgentEvent>>,
) -> Vec<(String, String)> {
    let expected = tasks.len();
    if expected == 0 {
        return Vec::new();
    }
    let (tx, mut rx) = mpsc::unbounded_channel();
    for task in tasks {
        supervisor.spawn(task, tx.clone());
    }
    drop(tx);

    let mut results = Vec::with_capacity(expected);
    while let Some(ev) = rx.recv().await {
        if let Some(fwd) = progress {
            let _ = fwd.send(ev.clone());
        }
        match ev {
            AgentEvent::Completed { id, output } => {
                let label = id_to_label.get(&id).cloned().unwrap_or(id);
                results.push((label, output));
            }
            AgentEvent::Failed { id, error } => {
                let label = id_to_label.get(&id).cloned().unwrap_or(id);
                results.push((label, format!("(сбой) {error}")));
            }
            _ => {}
        }
        if results.len() == expected {
            break;
        }
    }
    results
}

/// Запустить один агент и вернуть его вывод.
async fn run_single(
    supervisor: &Supervisor,
    title: String,
    prompt: String,
    progress: &Option<mpsc::UnboundedSender<AgentEvent>>,
) -> Result<String, String> {
    let task = AgentTask::new(title, prompt);
    let mut labels = HashMap::new();
    labels.insert(task.id.clone(), task.title.clone());
    let mut out = run_batch(supervisor, vec![task], &labels, progress).await;
    out.pop().map(|(_, o)| o).ok_or_else(|| "нет вывода".into())
}

// ---------- реестр: встроенные + пользовательские (TOML) ----------

/// Реестр воркфлоу.
#[derive(Debug, Clone)]
pub struct WorkflowRegistry {
    workflows: Vec<WorkflowSpec>,
}

impl Default for WorkflowRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

impl WorkflowRegistry {
    /// Только встроенные воркфлоу.
    pub fn with_builtins() -> Self {
        Self {
            workflows: builtin_workflows(),
        }
    }

    /// Встроенные + загруженные из стандартных каталогов TOML.
    pub fn load() -> Self {
        let mut reg = Self::with_builtins();
        for dir in workflow_dirs() {
            reg.load_dir(&dir);
        }
        reg
    }

    /// Загрузить пользовательские воркфлоу из каталога (перекрывают по id).
    pub fn load_dir(&mut self, dir: &std::path::Path) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(spec) = parse_toml(&text) {
                    self.upsert(spec);
                }
            }
        }
    }

    fn upsert(&mut self, spec: WorkflowSpec) {
        if let Some(existing) = self.workflows.iter_mut().find(|w| w.id == spec.id) {
            *existing = spec;
        } else {
            self.workflows.push(spec);
        }
    }

    pub fn find(&self, id: &str) -> Option<&WorkflowSpec> {
        let id = id.trim().to_lowercase();
        self.workflows.iter().find(|w| w.id == id)
    }

    pub fn all(&self) -> &[WorkflowSpec] {
        &self.workflows
    }

    /// Компактный список для TUI `/workflows`.
    pub fn listing(&self) -> String {
        self.workflows
            .iter()
            .map(|w| {
                let verify = if w.verify.is_some() { " +verify" } else { "" };
                format!(
                    "• {} — {} ({} producers → {}{})",
                    w.id,
                    w.description,
                    w.producers.len(),
                    if w.synthesis.is_some() {
                        "синтез"
                    } else {
                        "без синтеза"
                    },
                    verify
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Каталоги пользовательских воркфлоу: проект и домашний.
fn workflow_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = vec![std::path::PathBuf::from(".whatcode/workflows")];
    if let Some(home) = directories::BaseDirs::new() {
        dirs.push(home.home_dir().join(".whatcode/workflows"));
    }
    dirs
}

// ---------- TOML-разбор ----------

#[derive(Debug, Deserialize)]
struct TomlWorkflow {
    id: String,
    name: Option<String>,
    description: Option<String>,
    synthesis: Option<String>,
    verify: Option<TomlVerify>,
    #[serde(default, rename = "producer")]
    producers: Vec<TomlProducer>,
}

#[derive(Debug, Deserialize)]
struct TomlProducer {
    label: String,
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct TomlVerify {
    skeptics: u8,
    reject_threshold: u8,
}

/// Разобрать один воркфлоу из TOML.
pub fn parse_toml(text: &str) -> Result<WorkflowSpec, String> {
    let raw: TomlWorkflow = toml::from_str(text).map_err(|e| e.to_string())?;
    if raw.id.trim().is_empty() {
        return Err("пустой id воркфлоу".into());
    }
    if raw.producers.is_empty() {
        return Err("воркфлоу без producer-этапов".into());
    }
    Ok(WorkflowSpec {
        id: raw.id.trim().to_lowercase(),
        name: raw.name.unwrap_or_else(|| raw.id.clone()),
        description: raw.description.unwrap_or_default(),
        producers: raw
            .producers
            .into_iter()
            .map(|p| ProducerStage {
                label: p.label,
                prompt_template: p.prompt,
            })
            .collect(),
        synthesis: raw.synthesis,
        verify: raw.verify.map(|v| VerifyConfig {
            skeptics: v.skeptics,
            reject_threshold: v.reject_threshold,
        }),
    })
}

// ---------- встроенные воркфлоу ----------

fn producer(label: &str, prompt: &str) -> ProducerStage {
    ProducerStage {
        label: label.to_string(),
        prompt_template: prompt.to_string(),
    }
}

/// Встроенные воркфлоу.
pub fn builtin_workflows() -> Vec<WorkflowSpec> {
    vec![
        WorkflowSpec {
            id: "review".into(),
            name: "Код-ревью по измерениям".into(),
            description: "Ревьюеры по измерениям + сведение находок".into(),
            producers: vec![
                producer("корректность", "Проведи ревью на корректность и логические баги. Объект: {input}. Дай конкретные проблемы с местами и фиксами. Только реальные дефекты."),
                producer("безопасность", "Проведи ревью безопасности: инъекции, небезопасные вызовы, утечки, валидация ввода, секреты. Объект: {input}. Конкретные находки и риски."),
                producer("производительность", "Проведи ревью производительности: сложность, лишние аллокации, IO в циклах, блокировки. Объект: {input}. Конкретные узкие места."),
                producer("стиль", "Проведи ревью читаемости и стиля: именование, модульность, типизация, дублирование. Объект: {input}. Короткий список улучшений."),
            ],
            synthesis: Some("Сведи находки ревью в единый отчёт, сгруппировав по важности (критично/важно/мелочи) и убрав дубли. Задача: {input}.\n\nНаходки ревьюеров:\n{results}".into()),
            verify: None,
        },
        WorkflowSpec {
            id: "plan".into(),
            name: "Панель планов".into(),
            description: "Три подхода + выбор лучшего синтезом".into(),
            producers: vec![
                producer("MVP-first", "Составь план реализации максимально просто и быстро (MVP-first). Задача: {input}. Пошагово + риски срезанных углов."),
                producer("risk-first", "Составь план, начиная с самых рискованных частей (risk-first). Задача: {input}. Пошагово + точки проверки гипотез."),
                producer("quality-first", "Составь план с упором на архитектуру и тестируемость. Задача: {input}. Пошагово + ключевые инварианты."),
            ],
            synthesis: Some("Сравни три плана и собери из них один лучший, взяв сильное из каждого. Явно назови компромиссы. Задача: {input}.\n\nПланы:\n{results}".into()),
            verify: None,
        },
        WorkflowSpec {
            id: "research".into(),
            name: "Многоугловое исследование".into(),
            description: "Обзор/аналоги/риски + сведение с проверкой".into(),
            producers: vec![
                producer("обзор", "Дай сжатый технический обзор темы: ключевые понятия и текущее состояние. Тема: {input}."),
                producer("аналоги", "Сравни существующие подходы/библиотеки/паттерны по теме: {input}. Плюсы, минусы, когда что уместно."),
                producer("риски", "Перечисли подводные камни и типичные ошибки по теме: {input}. Как их избежать."),
            ],
            synthesis: Some("Собери связный отчёт по теме из трёх проходов, без повторов, с выводом и рекомендацией. Тема: {input}.\n\nМатериалы:\n{results}".into()),
            verify: Some(VerifyConfig { skeptics: 3, reject_threshold: 2 }),
        },
        WorkflowSpec {
            id: "debug".into(),
            name: "Отладочный конвейер".into(),
            description: "Гипотезы/воспроизведение/фикс + сведение".into(),
            producers: vec![
                producer("гипотезы", "Сформулируй 3-5 наиболее вероятных причин, ранжируй по вероятности. Проблема: {input}."),
                producer("воспроизведение", "Предложи минимальные шаги и проверки для воспроизведения и локализации. Проблема: {input}."),
                producer("фикс", "Предложи стратегию исправления и как убедиться, что фикс ничего не сломал. Проблема: {input}."),
            ],
            synthesis: Some("Собери из проходов единый план отладки: самая вероятная причина → как воспроизвести → как чинить → как проверить. Проблема: {input}.\n\nМатериалы:\n{results}".into()),
            verify: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_finds_builtins() {
        let reg = WorkflowRegistry::with_builtins();
        assert!(reg.find("review").is_some());
        assert!(reg.find("REVIEW").is_some());
        assert!(reg.find("nope").is_none());
        assert!(reg.listing().contains("review"));
    }

    #[test]
    fn producer_tasks_substitute_input() {
        let reg = WorkflowRegistry::with_builtins();
        let wf = reg.find("review").unwrap();
        let tasks = wf.producer_tasks("src/main.rs");
        assert_eq!(tasks.len(), wf.producers.len());
        assert!(tasks.iter().all(|t| t.prompt.contains("src/main.rs")));
        assert!(tasks.iter().all(|t| !t.prompt.contains("{input}")));
    }

    #[test]
    fn synthesis_prompt_embeds_results() {
        let reg = WorkflowRegistry::with_builtins();
        let wf = reg.find("plan").unwrap();
        let results = vec![
            ("MVP-first".to_string(), "быстрый план".to_string()),
            ("risk-first".to_string(), "рисковый план".to_string()),
        ];
        let p = wf.synthesis_prompt("задача X", &results).unwrap();
        assert!(p.contains("задача X"));
        assert!(p.contains("быстрый план"));
        assert!(p.contains("### risk-first"));
        assert!(!p.contains("{results}"));
    }

    #[test]
    fn empty_input_gets_fallback() {
        let reg = WorkflowRegistry::with_builtins();
        let wf = reg.find("debug").unwrap();
        let tasks = wf.producer_tasks("   ");
        assert!(tasks.iter().all(|t| !t.prompt.contains("{input}")));
    }

    #[test]
    fn vote_tally_confirms_and_rejects() {
        let cfg = VerifyConfig {
            skeptics: 3,
            reject_threshold: 2,
        };
        let confirmed = tally_votes(
            &[
                "ПОДТВЕРЖДЕНО, всё верно".into(),
                "ОПРОВЕРГНУТО: ошибка".into(),
                "Подтверждено".into(),
            ],
            cfg,
        );
        assert!(confirmed.confirmed);
        assert_eq!(confirmed.rejections, 1);

        let rejected = tally_votes(
            &[
                "ОПРОВЕРГНУТО: раз".into(),
                "ОПРОВЕРГНУТО: два".into(),
                "ПОДТВЕРЖДЕНО".into(),
            ],
            cfg,
        );
        assert!(!rejected.confirmed);
        assert_eq!(rejected.rejections, 2);
    }

    #[test]
    fn parse_toml_workflow() {
        let text = r#"
id = "audit"
name = "Security audit"
description = "аудит доступа"
synthesis = "Собери находки: {results} для {input}"
[verify]
skeptics = 3
reject_threshold = 2
[[producer]]
label = "инъекции"
prompt = "Проверь инъекции в {input}"
[[producer]]
label = "секреты"
prompt = "Проверь секреты в {input}"
"#;
        let wf = parse_toml(text).unwrap();
        assert_eq!(wf.id, "audit");
        assert_eq!(wf.producers.len(), 2);
        assert!(wf.synthesis.is_some());
        let v = wf.verify.unwrap();
        assert_eq!(v.skeptics, 3);
        assert_eq!(v.reject_threshold, 2);
        assert_eq!(wf.stage_count(), 2 + 1 + 1);
    }

    #[test]
    fn parse_toml_rejects_no_producers() {
        let text = "id = \"x\"\n";
        assert!(parse_toml(text).is_err());
    }

    #[test]
    fn user_workflow_overrides_builtin() {
        let mut reg = WorkflowRegistry::with_builtins();
        let custom = parse_toml(
            "id = \"review\"\ndescription = \"мой ревью\"\n[[producer]]\nlabel = \"x\"\nprompt = \"{input}\"\n",
        )
        .unwrap();
        reg.upsert(custom);
        assert_eq!(reg.find("review").unwrap().description, "мой ревью");
        assert_eq!(reg.find("review").unwrap().producers.len(), 1);
    }
}
