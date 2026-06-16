//! Обобщённый инструмент запуска внешней команды (`ProcessTool`).
//!
//! Используется для автономной разработки: cargo (сборка/тесты/проверки),
//! uv (Python), и пр. Базовые аргументы фиксированы; пользовательские
//! аргументы из параметра `args` добавляются (разбиение по пробелам).

use crate::registry::Tool;
use crate::util::run_capture;
use async_trait::async_trait;
use herta_core::{ParamType, ToolCall, ToolParameter, ToolResult, ToolRisk, ToolSpec};

/// Запуск программы `program base_args [args...]` в корне проекта.
pub struct ProcessTool {
    name: &'static str,
    description: &'static str,
    program: &'static str,
    base_args: Vec<&'static str>,
    risk: ToolRisk,
    timeout_secs: u64,
}

impl ProcessTool {
    pub fn new(
        name: &'static str,
        description: &'static str,
        program: &'static str,
        base_args: Vec<&'static str>,
        risk: ToolRisk,
        timeout_secs: u64,
    ) -> Self {
        Self {
            name,
            description,
            program,
            base_args,
            risk,
            timeout_secs,
        }
    }
}

#[async_trait]
impl Tool for ProcessTool {
    fn spec(&self) -> ToolSpec {
        let mut spec = ToolSpec::new(
            self.name,
            self.description,
            vec![ToolParameter::new(
                "args",
                ParamType::String,
                "Доп. аргументы командной строки (через пробел), необязательно",
                false,
            )],
        );
        spec.risk = self.risk;
        spec.destructive = self.risk == ToolRisk::Dangerous;
        spec
    }

    async fn call(&self, call: &ToolCall) -> ToolResult {
        let mut args: Vec<&str> = self.base_args.clone();
        let extra = call.arg_str("args").unwrap_or_default();
        // Простое разбиение по пробелам; для dev-команд этого достаточно.
        let extra_parts: Vec<String> = extra.split_whitespace().map(|s| s.to_string()).collect();
        for part in &extra_parts {
            args.push(part.as_str());
        }
        let cwd = std::env::current_dir().ok();
        match run_capture(self.program, &args, cwd.as_deref(), self.timeout_secs).await {
            Ok(out) => {
                let status = if out.success {
                    "успех"
                } else {
                    "ошибки"
                };
                let body = if out.combined.is_empty() {
                    "(нет вывода)".to_string()
                } else {
                    out.combined
                };
                ToolResult::ok(
                    self.name,
                    format!("[{}] {}\n{}", self.program, status, body),
                )
            }
            Err(e) => ToolResult::rejected(self.name, e),
        }
    }
}
