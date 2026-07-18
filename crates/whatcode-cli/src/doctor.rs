//! Самодиагностика: проверка конфигурации, провайдера и доступности инструментов.
//! Возвращает код выхода: 0 — всё в норме (нет FAIL), 1 — есть критичные проблемы.

use whatcode_core::{AppConfig, LlmProvider};
use whatcode_llm::build_client;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Ok,
    Warn,
    Fail,
}

fn line(level: Level, label: &str, detail: &str) {
    let tag = match level {
        Level::Ok => "[ OK ]",
        Level::Warn => "[WARN]",
        Level::Fail => "[FAIL]",
    };
    println!("{tag} {label}: {detail}");
}

async fn binary_available(program: &str) -> bool {
    tokio::process::Command::new(program)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

pub async fn run(config: &AppConfig) -> i32 {
    let mut fails = 0;
    let mut warns = 0;

    println!("=== Диагностика WhatCode ===");
    println!("персона: {}\n", config.persona);

    // Провайдер LLM.
    line(Level::Ok, "Провайдер", config.llm_provider.as_str());
    line(Level::Ok, "Модель", config.active_model());

    match config.llm_provider {
        LlmProvider::Ollama => {
            line(Level::Ok, "Ollama host", &config.ollama.host);
        }
        LlmProvider::Cerebras if config.cerebras.api_key.is_none() => {
            line(Level::Fail, "Cerebras", "не задан CEREBRAS_API_KEY");
            fails += 1;
        }
        LlmProvider::DeepSeek if config.deepseek.api_key.is_none() => {
            line(Level::Fail, "DeepSeek", "не задан DEEPSEEK_API_KEY");
            fails += 1;
        }
        LlmProvider::GoogleAi if config.google_ai.api_key.is_none() => {
            line(Level::Fail, "Google AI", "не задан GOOGLE_AI_API_KEY");
            fails += 1;
        }
        LlmProvider::Anthropic if config.anthropic.api_key.is_none() => {
            line(Level::Fail, "Anthropic", "не задан ANTHROPIC_API_KEY");
            fails += 1;
        }
        LlmProvider::Fireworks if config.fireworks.api_key.is_none() => {
            line(Level::Fail, "Fireworks", "не задан FIREWORKS_API_KEY");
            fails += 1;
        }
        LlmProvider::OpenCodeGo if config.opencode_go.api_key.is_none() => {
            line(Level::Fail, "OpenCode Go", "не задан OPENCODE_GO_API_KEY");
            fails += 1;
        }
        _ => {}
    }

    // Попытка построить и прогреть клиента.
    match build_client(config) {
        Ok(client) => match client.warm_up().await {
            Ok(_) => line(Level::Ok, "Прогрев", "клиент готов"),
            Err(e) => {
                line(Level::Warn, "Прогрев", &e.to_string());
                warns += 1;
            }
        },
        Err(e) => {
            line(Level::Fail, "Клиент", &e.to_string());
            fails += 1;
        }
    }

    // Память.
    line(
        Level::Ok,
        "Кратковременная память",
        if config.memory.enabled {
            &config.memory.path
        } else {
            "выключена"
        },
    );
    line(
        Level::Ok,
        "Долговременная память",
        if config.long_memory.enabled {
            &config.long_memory.path
        } else {
            "выключена"
        },
    );

    // Контекстное окно.
    line(
        Level::Ok,
        "Контекст",
        &format!(
            "{} токенов, автосжатие при {:.0}%",
            config.context.max_tokens,
            config.context.compaction_threshold * 100.0
        ),
    );

    // Саб-агенты.
    line(
        Level::Ok,
        "Саб-агенты",
        &format!(
            "до {} параллельно, таймаут {} c",
            config.agent.max_concurrent, config.agent.timeout_seconds
        ),
    );

    // Инструменты кода.
    if config.code_tools.enabled {
        for tool in ["mypy", "ruff"] {
            if binary_available(tool).await {
                line(Level::Ok, "Инструмент кода", tool);
            } else {
                line(
                    Level::Warn,
                    "Инструмент кода",
                    &format!("{tool} не найден в PATH"),
                );
                warns += 1;
            }
        }
    }

    // Веб-поиск.
    if config.web_search.enabled && config.web_search.api_key.is_none() {
        line(
            Level::Warn,
            "Веб-поиск",
            "включён, но не задан TAVILY_API_KEY",
        );
        warns += 1;
    }

    // Персоны.
    let personas = whatcode_core::persona::common::list()
        .into_iter()
        .map(|(id, name)| format!("{id} ({name})"))
        .collect::<Vec<_>>()
        .join(", ");
    line(Level::Ok, "Персоны", &personas);

    // Внешние агенты (Agent Context Protocol).
    if config.external_agents.enabled {
        let statuses = whatcode_tools::detect_external_agents(&config.external_agents);
        let available: Vec<String> = statuses
            .iter()
            .filter(|s| s.available)
            .map(|s| s.id.clone())
            .collect();
        if available.is_empty() {
            line(
                Level::Warn,
                "Внешние агенты",
                "включены, но ни один CLI не найден в PATH (claude, codex, gemini, …)",
            );
            warns += 1;
        } else {
            line(
                Level::Ok,
                "Внешние агенты",
                &format!("доступны: {}", available.join(", ")),
            );
        }
    } else {
        line(Level::Ok, "Внешние агенты", "выключены");
    }

    println!("\nИтог: {fails} критичных, {warns} предупреждений.");
    if fails > 0 {
        1
    } else {
        0
    }
}
