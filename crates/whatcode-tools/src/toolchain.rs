//! Установка инструментария и зависимостей для навыков (опасные операции).
//!
//! Герта может сама подготовить окружение: поставить Rust (rustup), UV-менеджер,
//! Python через UV. Эти действия помечены как `Dangerous` — в режимах ниже
//! `full-auto` требуют явного разрешения (`/allow install_toolchain`).
//!
//! Команды установки берутся из проверенных официальных источников и
//! выполняются через системную оболочку.

use crate::registry::Tool;
use crate::util::run_capture;
use async_trait::async_trait;
use whatcode_core::{ParamType, ToolCall, ToolParameter, ToolResult, ToolSpec};

const INSTALL_TIMEOUT_SECS: u64 = 600;

/// `check_toolchain` — проверить наличие ключевых инструментов в PATH.
#[derive(Default)]
pub struct CheckToolchainTool;

#[async_trait]
impl Tool for CheckToolchainTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "check_toolchain",
            "Проверить, какие инструменты разработки доступны в системе: rustc, cargo, uv, python, \
             git, node. Только чтение. Вызови перед установкой, чтобы не ставить уже имеющееся.",
            vec![],
        )
    }
    async fn call(&self, _call: &ToolCall) -> ToolResult {
        let tools = ["rustc", "cargo", "uv", "python3", "python", "git", "node"];
        let mut lines = Vec::new();
        for t in tools {
            let present = run_capture(t, &["--version"], None, 15)
                .await
                .map(|o| o.success)
                .unwrap_or(false);
            lines.push(format!("{} {}", if present { "✓" } else { "✗" }, t));
        }
        ToolResult::ok("check_toolchain", lines.join("\n"))
    }
}

/// Поддерживаемые цели установки.
fn install_plan(target: &str) -> Option<(&'static str, Vec<&'static str>)> {
    // Возвращает (программа, аргументы). Только не-Windows shell-инсталляторы;
    // на Windows подсказываем ручной шаг (winget) текстом.
    match target.trim().to_lowercase().as_str() {
        "rust" | "rustup" => Some((
            "sh",
            vec![
                "-c",
                "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
            ],
        )),
        "uv" => Some((
            "sh",
            vec!["-c", "curl -LsSf https://astral.sh/uv/install.sh | sh"],
        )),
        "python" => Some(("uv", vec!["python", "install"])),
        _ => None,
    }
}

fn windows_hint(target: &str) -> Option<&'static str> {
    match target.trim().to_lowercase().as_str() {
        "rust" | "rustup" => Some("winget install --id Rustlang.Rustup -e"),
        "uv" => Some("winget install --id astral-sh.uv -e  (или: powershell -c \"irm https://astral.sh/uv/install.ps1 | iex\")"),
        "vs-build-tools" | "build-tools" => {
            Some("winget install --id Microsoft.VisualStudio.2022.BuildTools -e (нужны C++ build tools для линковки Rust)")
        }
        "python" => Some("uv python install (после установки uv)"),
        _ => None,
    }
}

/// `install_toolchain` — установить Rust/UV/Python и т.п.
#[derive(Default)]
pub struct InstallToolchainTool;

#[async_trait]
impl Tool for InstallToolchainTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "install_toolchain",
            "Установить инструмент разработки или зависимость навыка. Поддерживаемые цели: \
             `rust` (rustup), `uv` (менеджер пакетов Python), `python` (через uv), на Windows также \
             подсказка для `vs-build-tools`. ОПАСНО: скачивает и запускает установщики, меняет систему. \
             Сначала вызови check_toolchain. На Windows возвращает команду winget для ручного запуска.",
            vec![ToolParameter::new("target", ParamType::String, "rust | uv | python | vs-build-tools", true)],
        )
        .destructive()
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let Some(target) = call.arg_str("target") else {
            return ToolResult::rejected("install_toolchain", "не передан `target`");
        };

        if cfg!(windows) {
            return match windows_hint(&target) {
                Some(cmd) => ToolResult::ok(
                    "install_toolchain",
                    format!("На Windows автоустановка отключена ради безопасности. Выполните вручную:\n{cmd}"),
                ),
                None => ToolResult::rejected("install_toolchain", format!("неизвестная цель `{target}` для Windows")),
            };
        }

        match install_plan(&target) {
            Some((program, args)) => {
                match run_capture(program, &args, None, INSTALL_TIMEOUT_SECS).await {
                    Ok(out) if out.success => ToolResult::ok(
                        "install_toolchain",
                        format!("Установка `{target}` завершена.\n{}", out.combined),
                    ),
                    Ok(out) => ToolResult::rejected(
                        "install_toolchain",
                        format!("установка `{target}` вернула ошибку:\n{}", out.combined),
                    ),
                    Err(e) => ToolResult::rejected("install_toolchain", e),
                }
            }
            None => ToolResult::rejected(
                "install_toolchain",
                format!("неизвестная цель `{target}`; доступно: rust | uv | python"),
            ),
        }
    }
}
