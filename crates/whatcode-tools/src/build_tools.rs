//! Универсальные инструменты сборки и верификации проектов.
//!
//! `verify_build` определяет тип проекта по наличию конфигурационных файлов и
//! запускает соответствующую команду сборки/проверки. Возвращает чёткий
//! результат для LLM: успех или список ошибок.

use crate::registry::Tool;
use crate::util::run_capture;
use async_trait::async_trait;
use whatcode_core::{ParamType, ToolCall, ToolParameter, ToolResult, ToolSpec};
use std::path::Path;

const TIMEOUT_SECS: u64 = 600;

/// Определяет тип проекта по файлам в корне.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectType {
    Rust,
    PythonUv,
    TypeScriptBun,
    NodeNpm,
    Unknown,
}

fn detect_project_type(root: &Path) -> ProjectType {
    if root.join("Cargo.toml").exists() {
        return ProjectType::Rust;
    }
    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        return ProjectType::PythonUv;
    }
    if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
        return ProjectType::TypeScriptBun;
    }
    if root.join("package.json").exists() {
        return ProjectType::NodeNpm;
    }
    ProjectType::Unknown
}

/// `verify_build` — определить тип проекта и запустить проверку сборки.
#[derive(Default)]
pub struct VerifyBuildTool;

#[async_trait]
impl Tool for VerifyBuildTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "verify_build",
            "Определить тип проекта в текущем каталоге и выполнить проверку сборки. \
             Для Rust: `cargo check --all-targets`. Для Python+UV: `uv run -- python -m compileall`. \
             Для Bun: `bun run build` (или `bun test` если нет build). Для npm: `npm run build`. \
             Возвращает успех или список ошибок. Это read-only для рабочего дерева (не пушит/не коммитит).",
            vec![ToolParameter::new(
                "target",
                ParamType::String,
                "rust | python | bun | npm | auto (по умолчанию auto)",
                false,
            )],
        )
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let root = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        let target = call
            .arg_str("target")
            .unwrap_or_else(|| "auto".to_string())
            .to_lowercase();

        let project_type = match target.as_str() {
            "rust" => ProjectType::Rust,
            "python" => ProjectType::PythonUv,
            "bun" => ProjectType::TypeScriptBun,
            "npm" => ProjectType::NodeNpm,
            "auto" => detect_project_type(&root),
            _ => return ToolResult::rejected("verify_build", "target: rust | python | bun | npm | auto"),
        };

        let (program, args, label) = match project_type {
            ProjectType::Rust => ("cargo", vec!["check", "--all-targets"], "Rust cargo check"),
            ProjectType::PythonUv => ("uv", vec!["run", "--", "python", "-m", "compileall", "."], "Python compileall"),
            ProjectType::TypeScriptBun => {
                let package_json = root.join("package.json");
                let has_build = if package_json.exists() {
                    std::fs::read_to_string(&package_json)
                        .ok()
                        .map(|s| s.contains("\"build\""))
                        .unwrap_or(false)
                } else {
                    false
                };
                if has_build {
                    ("bun", vec!["run", "build"], "Bun build")
                } else {
                    ("bun", vec!["test"], "Bun test")
                }
            }
            ProjectType::NodeNpm => ("npm", vec!["run", "build"], "npm run build"),
            ProjectType::Unknown => {
                return ToolResult::rejected(
                    "verify_build",
                    "не удалось определить тип проекта (Cargo.toml, pyproject.toml, package.json, bun.lock)",
                );
            }
        };

        match run_capture(program, &args, Some(&root), TIMEOUT_SECS).await {
            Ok(out) => {
                let status = if out.success { "успех" } else { "ошибка" };
                let body = if out.combined.is_empty() {
                    "(нет вывода)".to_string()
                } else {
                    out.combined
                };
                ToolResult::ok(
                    "verify_build",
                    format!("[{label}] {status}\n{body}"),
                )
            }
            Err(e) => ToolResult::rejected("verify_build", e),
        }
    }
}

/// `project_info` — показать, какой тип проекта detected и какие инструменты доступны.
#[derive(Default)]
pub struct ProjectInfoTool;

#[async_trait]
impl Tool for ProjectInfoTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "project_info",
            "Определить тип проекта в текущем каталоге и показать доступные инструменты сборки. \
             Только чтение.",
            vec![],
        )
    }

    async fn call(&self, _call: &ToolCall) -> ToolResult {
        let root = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        let detected = detect_project_type(&root);
        let info = match detected {
            ProjectType::Rust => "Rust-проект (Cargo.toml). Инструменты: cargo_check, cargo_build, cargo_test, cargo_clippy, cargo_fmt, cargo_add, cargo_run, verify_build.",
            ProjectType::PythonUv => "Python-проект (pyproject.toml/requirements.txt). Инструменты: uv_run, uv_add, uv_sync, uv_pip, verify_build.",
            ProjectType::TypeScriptBun => "TypeScript/Bun-проект. Инструменты: bun_run, bun_test, bun_build, bun_add, bun_install, bun_lint, verify_build.",
            ProjectType::NodeNpm => "Node.js/npm-проект. Инструменты: npm-скрипты через verify_build, bun_run.",
            ProjectType::Unknown => "Тип проекта не определён. Не найдены Cargo.toml, pyproject.toml, package.json, bun.lock.",
        };
        ToolResult::ok("project_info", info.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_by_cargo_toml() {
        let tmp = std::env::temp_dir().join("whatcode-test-rust-detect");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("Cargo.toml"), "[package]\n").unwrap();
        assert_eq!(detect_project_type(&tmp), ProjectType::Rust);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn detects_python_by_pyproject() {
        let tmp = std::env::temp_dir().join("whatcode-test-py-detect");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("pyproject.toml"), "[project]\n").unwrap();
        assert_eq!(detect_project_type(&tmp), ProjectType::PythonUv);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
