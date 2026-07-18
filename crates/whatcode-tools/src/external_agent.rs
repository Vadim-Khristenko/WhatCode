//! Мост к внешним CLI-агентам — «Agent Context Protocol» в прагматичном виде.
//!
//! WhatCode умеет делегировать задачу другому кодовому агенту, запуская его в
//! неинтерактивном режиме (`claude -p`, `codex exec`, `gemini -p`, …) и забирая
//! ответ из stdout. Это даёт межагентную кооперацию без общего рантайма: каждый
//! агент — отдельный процесс со своей моделью и инструментами.
//!
//! Набор известных агентов статичен ([`KNOWN_AGENTS`]); дополнительные команды
//! добавляются через конфиг (`WHATCODE_EXTERNAL_AGENTS_CUSTOM`). Инструмент
//! помечен как `Dangerous`, поэтому в безопасных режимах требует подтверждения.

use crate::registry::Tool;
use crate::util::{run_capture, MAX_OUTPUT_CHARS};
use async_trait::async_trait;
use whatcode_core::config::ExternalAgentsConfig;
use whatcode_core::{ParamType, ToolCall, ToolParameter, ToolResult, ToolSpec};

/// Описание одного внешнего агента и способа его неинтерактивного вызова.
#[derive(Debug, Clone)]
pub struct AgentSpec {
    /// Машинный идентификатор (`claude`, `codex`, …).
    pub id: String,
    /// Человекочитаемое имя.
    pub display: String,
    /// Исполняемый файл.
    pub program: String,
    /// Базовые аргументы; промпт добавляется последним элементом.
    pub base_args: Vec<String>,
}

impl AgentSpec {
    fn new(id: &str, display: &str, program: &str, base_args: &[&str]) -> Self {
        Self {
            id: id.to_string(),
            display: display.to_string(),
            program: program.to_string(),
            base_args: base_args.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Известные CLI-агенты и их неинтерактивный («headless») вызов.
///
/// Во всех случаях промпт передаётся последним позиционным аргументом, поэтому
/// достаточно `base_args + [prompt]`.
pub fn known_agents() -> Vec<AgentSpec> {
    vec![
        AgentSpec::new("claude", "Claude Code", "claude", &["-p"]),
        AgentSpec::new("codex", "OpenAI Codex CLI", "codex", &["exec"]),
        AgentSpec::new("gemini", "Gemini CLI", "gemini", &["-p"]),
        AgentSpec::new("qwen", "Qwen Code", "qwen", &["-p"]),
        AgentSpec::new("opencode", "opencode", "opencode", &["run"]),
        AgentSpec::new("cursor", "Cursor Agent", "cursor-agent", &["-p"]),
        AgentSpec::new("amp", "Sourcegraph Amp", "amp", &["-x"]),
        AgentSpec::new("crush", "Charm Crush", "crush", &["run"]),
        AgentSpec::new("grok", "Grok Build CLI (xAI)", "grok", &["-p"]),
    ]
}

/// Разобрать пользовательские определения агентов из конфига.
///
/// Формат одной записи: `id=программа арг1 арг2 …`. Если среди аргументов есть
/// литерал `{prompt}`, он заменяется на текст; иначе промпт добавляется в конец.
fn parse_custom(defs: &[String]) -> Vec<AgentSpec> {
    let mut out = Vec::new();
    for def in defs {
        let Some((id, rest)) = def.split_once('=') else {
            continue;
        };
        let id = id.trim();
        let mut parts = rest.split_whitespace();
        let Some(program) = parts.next() else {
            continue;
        };
        let base_args: Vec<String> = parts.map(|s| s.to_string()).collect();
        if id.is_empty() {
            continue;
        }
        out.push(AgentSpec {
            id: id.to_lowercase(),
            display: format!("{id} (custom)"),
            program: program.to_string(),
            base_args,
        });
    }
    out
}

/// Все агенты: пользовательские (приоритет) + встроенные.
pub fn all_agents(cfg: &ExternalAgentsConfig) -> Vec<AgentSpec> {
    let mut agents = parse_custom(&cfg.custom);
    let custom_ids: std::collections::HashSet<String> =
        agents.iter().map(|a| a.id.clone()).collect();
    for a in known_agents() {
        if !custom_ids.contains(&a.id) {
            agents.push(a);
        }
    }
    agents
}

/// Есть ли исполняемый файл `program` в `PATH`. Кроссплатформенно, без запуска
/// самой программы (чтобы не подвиснуть на интерактивных бинарях).
pub fn is_on_path(program: &str) -> bool {
    // Абсолютный/относительный путь — проверяем напрямую.
    if program.contains(std::path::MAIN_SEPARATOR) {
        return std::path::Path::new(program).is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into())
            .split(';')
            .map(|s| s.to_string())
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path) {
        for ext in &exts {
            let candidate = dir.join(format!("{program}{ext}"));
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

/// Снимок доступности одного агента для отчётов TUI/doctor.
#[derive(Debug, Clone)]
pub struct AgentStatus {
    pub id: String,
    pub display: String,
    pub program: String,
    pub available: bool,
}

/// Проверить, какие из настроенных агентов реально установлены.
pub fn detect(cfg: &ExternalAgentsConfig) -> Vec<AgentStatus> {
    all_agents(cfg)
        .into_iter()
        .map(|a| AgentStatus {
            available: is_on_path(&a.program),
            id: a.id,
            display: a.display,
            program: a.program,
        })
        .collect()
}

/// Инструмент делегирования задачи внешнему агенту.
pub struct ExternalAgentTool {
    cfg: ExternalAgentsConfig,
}

impl ExternalAgentTool {
    pub fn new(cfg: ExternalAgentsConfig) -> Self {
        Self { cfg }
    }

    fn resolve(&self, id: &str) -> Option<AgentSpec> {
        let id = id.trim().to_lowercase();
        all_agents(&self.cfg).into_iter().find(|a| a.id == id)
    }
}

#[async_trait]
impl Tool for ExternalAgentTool {
    fn spec(&self) -> ToolSpec {
        let ids: Vec<String> = all_agents(&self.cfg).into_iter().map(|a| a.id).collect();
        let desc = format!(
            "Делегировать задачу другому кодовому агенту через его CLI \
             (Agent Context Protocol / headless-режим). Полезно для второго мнения, \
             ревью или узкоспециализированной модели. Доступные id: {}. \
             Внешний агент может изменять файлы в каталоге проекта.",
            ids.join(", ")
        );
        let mut spec = ToolSpec::new(
            "ask_external_agent",
            desc,
            vec![
                ToolParameter::new(
                    "agent",
                    ParamType::String,
                    "Идентификатор агента (claude, codex, gemini, qwen, opencode, …)",
                    self.cfg.default_agent.is_none(),
                ),
                ToolParameter::new(
                    "prompt",
                    ParamType::String,
                    "Текст задачи/вопроса для внешнего агента",
                    true,
                ),
                ToolParameter::new(
                    "cwd",
                    ParamType::String,
                    "Рабочий каталог (необязательно, по умолчанию текущий)",
                    false,
                ),
            ],
        );
        spec = spec.destructive();
        spec
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let name = "ask_external_agent";
        let prompt = match call.arg_str("prompt") {
            Some(p) if !p.trim().is_empty() => p,
            _ => return ToolResult::rejected(name, "не задан параметр `prompt`"),
        };
        let agent_id = call
            .arg_str("agent")
            .filter(|s| !s.trim().is_empty())
            .or_else(|| self.cfg.default_agent.clone())
            .unwrap_or_default();
        if agent_id.trim().is_empty() {
            return ToolResult::rejected(
                name,
                "не указан агент и нет default_agent в конфигурации",
            );
        }

        let Some(spec) = self.resolve(&agent_id) else {
            let known: Vec<String> = all_agents(&self.cfg).into_iter().map(|a| a.id).collect();
            return ToolResult::rejected(
                name,
                format!(
                    "неизвестный агент `{agent_id}`. Доступные: {}",
                    known.join(", ")
                ),
            );
        };

        if !is_on_path(&spec.program) {
            return ToolResult::rejected(
                name,
                format!(
                    "агент `{}` не установлен (нет `{}` в PATH)",
                    spec.id, spec.program
                ),
            );
        }

        // Сборка аргументов: base_args, подставляя {prompt} или добавляя в конец.
        let mut args: Vec<String> = Vec::with_capacity(spec.base_args.len() + 1);
        let mut substituted = false;
        for a in &spec.base_args {
            if a.contains("{prompt}") {
                args.push(a.replace("{prompt}", &prompt));
                substituted = true;
            } else {
                args.push(a.clone());
            }
        }
        if !substituted {
            args.push(prompt.clone());
        }
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let cwd = call
            .arg_str("cwd")
            .filter(|s| !s.trim().is_empty())
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::current_dir().ok());

        match run_capture(
            &spec.program,
            &arg_refs,
            cwd.as_deref(),
            self.cfg.timeout_seconds,
        )
        .await
        {
            Ok(out) => {
                let status = if out.success {
                    "успех"
                } else {
                    "ошибка"
                };
                let body = if out.combined.is_empty() {
                    "(нет вывода)".to_string()
                } else {
                    out.combined
                };
                ToolResult::ok(
                    name,
                    format!("[{} · {}] {}\n{}", spec.display, spec.program, status, body),
                )
            }
            Err(e) => ToolResult::rejected(name, e),
        }
    }
}

/// Компактный отчёт о доступных агентах (для TUI `/agents` и doctor).
pub fn availability_report(cfg: &ExternalAgentsConfig) -> String {
    let statuses = detect(cfg);
    let mut lines = Vec::with_capacity(statuses.len());
    for s in statuses {
        let mark = if s.available { "✓" } else { "·" };
        lines.push(format!("{mark} {} ({}) — {}", s.id, s.display, s.program));
    }
    let mut report = lines.join("\n");
    if report.chars().count() > MAX_OUTPUT_CHARS {
        report = report.chars().take(MAX_OUTPUT_CHARS).collect();
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_agents_include_claude_and_codex() {
        let ids: Vec<String> = known_agents().into_iter().map(|a| a.id).collect();
        assert!(ids.contains(&"claude".to_string()));
        assert!(ids.contains(&"codex".to_string()));
        assert!(ids.contains(&"gemini".to_string()));
    }

    #[test]
    fn parse_custom_entries() {
        let defs = vec![
            "mycli=mytool run --json {prompt}".to_string(),
            "bad-no-equals".to_string(),
            "empty=".to_string(),
        ];
        let parsed = parse_custom(&defs);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "mycli");
        assert_eq!(parsed[0].program, "mytool");
        assert!(parsed[0].base_args.contains(&"{prompt}".to_string()));
    }

    #[test]
    fn custom_overrides_builtin() {
        let cfg = ExternalAgentsConfig {
            custom: vec!["claude=my-claude --print".to_string()],
            ..Default::default()
        };
        let agents = all_agents(&cfg);
        let claude: Vec<&AgentSpec> = agents.iter().filter(|a| a.id == "claude").collect();
        assert_eq!(claude.len(), 1);
        assert_eq!(claude[0].program, "my-claude");
    }

    #[test]
    fn nonexistent_program_not_on_path() {
        assert!(!is_on_path("definitely-not-a-real-binary-xyz-123"));
    }
}
