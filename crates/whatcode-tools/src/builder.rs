//! Сборка реестра инструментов из конфигурации и режима работы.
//!
//! Регистрируются все инструменты; видимость и исполнение фильтрует политика
//! режима ([`whatcode_core::Policy`]) внутри реестра. Базовый набор (git, файлы,
//! время, fetch_url, навыки, cargo, uv, toolchain) доступен всегда; память,
//! веб-поиск, анализ кода и системные действия — по флагам конфигурации.

use crate::code_tools::{LintTool, TypeCheckTool};
use crate::fs_tools::{AppendFileTool, ListDirTool, ReadFileTool, WriteFileTool};
use crate::git::{
    GitAddTool, GitBranchTool, GitCommitTool, GitDiffTool, GitGrepTool, GitLogTool, GitShowTool,
    GitStatusTool,
};
use crate::http_tool::FetchUrlTool;
use crate::memory_tools::{ForgetTool, RecallTool, RememberTool};
use crate::proc_tool::ProcessTool;
use crate::registry::ToolRegistry;
use crate::skills::{ListSkillsTool, SkillLibrary, UseSkillTool};
use crate::system_actions::{CreateNoteTool, OpenUrlTool};
use crate::time_tool::CurrentTimeTool;
use crate::toolchain::{CheckToolchainTool, InstallToolchainTool};
use crate::web_search::WebSearchTool;
use whatcode_core::config::AppConfig;
use whatcode_core::{AgentMode, LongMemoryStore, ToolRisk};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Каталог навыков из окружения `whatcode_SKILLS_DIR` (по умолчанию `skills`).
fn skills_dir() -> String {
    std::env::var("whatcode_SKILLS_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "skills".to_string())
}

/// Зарегистрировать инструменты автономной разработки на Rust (cargo).
fn register_cargo(reg: &mut ToolRegistry) {
    let w = ToolRisk::Write;
    let tools: Vec<ProcessTool> = vec![
        ProcessTool::new("cargo_check", "Быстрая проверка компиляции Rust-проекта без сборки артефактов (`cargo check`). Используй часто во время правок.", "cargo", vec!["check"], w, 300),
        ProcessTool::new("cargo_build", "Собрать Rust-проект (`cargo build`). Доп. аргументы: например `--release`.", "cargo", vec!["build"], w, 600),
        ProcessTool::new("cargo_test", "Запустить тесты Rust-проекта (`cargo test`). Можно указать имя теста в args.", "cargo", vec!["test"], w, 600),
        ProcessTool::new("cargo_clippy", "Линтер Rust (`cargo clippy --all-targets`). Показывает предупреждения и ошибки качества.", "cargo", vec!["clippy", "--all-targets"], w, 600),
        ProcessTool::new("cargo_fmt", "Отформатировать код Rust (`cargo fmt`). Изменяет файлы. Для проверки используй args `--check`.", "cargo", vec!["fmt"], w, 120),
        ProcessTool::new("cargo_add", "Добавить зависимость в Cargo.toml (`cargo add <crate>`). Имя крейта передай в args.", "cargo", vec!["add"], w, 120),
        ProcessTool::new("cargo_run", "Запустить бинарь проекта (`cargo run`). Аргументы программы — после `--` в args.", "cargo", vec!["run"], w, 600),
    ];
    for t in tools {
        reg.register(Arc::new(t));
    }
}

/// Зарегистрировать инструменты Python через UV-менеджер.
fn register_uv(reg: &mut ToolRegistry) {
    let w = ToolRisk::Write;
    let tools: Vec<ProcessTool> = vec![
        ProcessTool::new("uv_run", "Запустить Python-команду/скрипт в окружении проекта через UV (`uv run`). Команда — в args.", "uv", vec!["run"], w, 600),
        ProcessTool::new("uv_add", "Добавить Python-зависимость через UV (`uv add <pkg>`). Имя пакета — в args.", "uv", vec!["add"], w, 300),
        ProcessTool::new("uv_sync", "Синхронизировать окружение Python по lock-файлу (`uv sync`).", "uv", vec!["sync"], w, 600),
        ProcessTool::new("uv_pip", "Управление пакетами в стиле pip через UV (`uv pip ...`). Подкоманду и аргументы передай в args, напр. `install requests`.", "uv", vec!["pip"], w, 600),
    ];
    for t in tools {
        reg.register(Arc::new(t));
    }
}

/// Построить полный реестр инструментов для агента в заданном режиме.
pub fn build_registry(
    config: &AppConfig,
    long_memory: Arc<Mutex<LongMemoryStore>>,
    mode: AgentMode,
) -> ToolRegistry {
    let mut reg = ToolRegistry::with_mode(mode);

    // --- разведка и чтение ---
    reg.register(Arc::new(GitStatusTool::default()))
        .register(Arc::new(GitLogTool::default()))
        .register(Arc::new(GitDiffTool::default()))
        .register(Arc::new(GitBranchTool::default()))
        .register(Arc::new(GitShowTool::default()))
        .register(Arc::new(GitGrepTool::default()))
        .register(Arc::new(ReadFileTool))
        .register(Arc::new(ListDirTool))
        .register(Arc::new(FetchUrlTool::default()))
        .register(Arc::new(CurrentTimeTool))
        .register(Arc::new(CheckToolchainTool));

    // --- запись и git-мутации ---
    reg.register(Arc::new(WriteFileTool))
        .register(Arc::new(AppendFileTool))
        .register(Arc::new(GitAddTool::default()))
        .register(Arc::new(GitCommitTool::default()));

    // --- автономная разработка ---
    register_cargo(&mut reg);
    register_uv(&mut reg);

    // --- установка инструментария (опасное) ---
    reg.register(Arc::new(InstallToolchainTool));

    // --- навыки (прогрессивное раскрытие) ---
    let library = SkillLibrary::load(skills_dir());
    if !library.is_empty() {
        reg.register(Arc::new(ListSkillsTool::new(library.clone())))
            .register(Arc::new(UseSkillTool::new(library)));
    }

    // --- долговременная память ---
    if config.long_memory.enabled {
        reg.register(Arc::new(RememberTool::new(Arc::clone(&long_memory))))
            .register(Arc::new(RecallTool::new(Arc::clone(&long_memory))))
            .register(Arc::new(ForgetTool::new(Arc::clone(&long_memory))));
    }

    // --- веб-поиск ---
    if config.web_search.enabled {
        reg.register(Arc::new(WebSearchTool::new(config.web_search.clone())));
    }

    // --- анализ кода (Python) ---
    if config.code_tools.enabled {
        reg.register(Arc::new(TypeCheckTool::new(config.code_tools.clone())))
            .register(Arc::new(LintTool::new(config.code_tools.clone())));
    }

    // --- системные действия ---
    if config.system_actions.enabled {
        reg.register(Arc::new(OpenUrlTool::new(config.system_actions.clone())))
            .register(Arc::new(CreateNoteTool::new(config.system_actions.clone())));
    }

    reg
}
