//! Персистентные настройки, задаваемые командами (без переменных окружения).
//!
//! На Android нельзя выставить переменные окружения, поэтому конфигурацию
//! (провайдер, модель, API-ключ, базовый URL, персона, режим) можно задавать
//! прямо в строке ввода командами `/set`, `/unset`, `/config`. Значения
//! сохраняются в TOML-файл и накладываются поверх окружения при загрузке
//! [`AppConfig::from_env`](crate::config::AppConfig::from_env).

use crate::config::{AppConfig, LlmProvider};
use crate::mode::AgentMode;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Ключи, которые можно задавать через `/set`.
pub const KNOWN_KEYS: &[&str] = &[
    "provider",
    "model",
    "api_key",
    "base_url",
    "persona",
    "mode",
    "log_level",
];

/// Ключи, чьё изменение требует пересборки LLM-клиента.
pub const CLIENT_KEYS: &[&str] = &["provider", "model", "api_key", "base_url"];

/// Персистентные пользовательские настройки.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    map: BTreeMap<String, String>,
    path: PathBuf,
}

/// Путь к файлу настроек: `$WHATCODE_CONFIG` → `$HOME/.whatcode/settings.toml`
/// → конфиг-каталог ОС → `./.whatcode/settings.toml`.
pub fn settings_path() -> PathBuf {
    if let Ok(p) = std::env::var("WHATCODE_CONFIG") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home).join(".whatcode").join("settings.toml");
        }
    }
    if let Some(dirs) = directories::BaseDirs::new() {
        return dirs.config_dir().join("whatcode").join("settings.toml");
    }
    PathBuf::from(".whatcode").join("settings.toml")
}

impl Settings {
    /// Загрузить настройки (отсутствие файла — пустые настройки).
    pub fn load() -> Self {
        let path = settings_path();
        let map = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| text.parse::<toml::Table>().ok())
            .map(|table| {
                table
                    .into_iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        Self { map, path }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(|s| s.as_str())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.map.insert(key.to_string(), value.to_string());
    }

    pub fn unset(&mut self, key: &str) -> bool {
        self.map.remove(key).is_some()
    }

    pub fn pairs(&self) -> impl Iterator<Item = (&String, &String)> {
        self.map.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Сохранить настройки в файл (создаёт родительские каталоги).
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut table = toml::Table::new();
        for (k, v) in &self.map {
            table.insert(k.clone(), toml::Value::String(v.clone()));
        }
        let text = toml::to_string_pretty(&table)
            .unwrap_or_else(|_| String::from("# ошибка сериализации\n"));
        std::fs::write(&self.path, text)
    }

    /// Наложить настройки на конфигурацию (значения настроек имеют приоритет).
    pub fn apply_to(&self, cfg: &mut AppConfig) {
        if let Some(v) = self.get("log_level") {
            cfg.log_level = v.to_uppercase();
        }
        if let Some(v) = self.get("provider") {
            cfg.llm_provider = LlmProvider::parse(v);
        }
        if let Some(v) = self.get("persona") {
            cfg.persona = v.trim().to_lowercase();
        }
        if let Some(v) = self.get("mode") {
            if let Some(m) = AgentMode::parse(v) {
                cfg.mode = m;
            }
        }
        // Провайдер-специфичные — после установки провайдера.
        if let Some(v) = self.get("model") {
            cfg.set_active_model(v.to_string());
        }
        if let Some(v) = self.get("api_key") {
            cfg.set_active_api_key(v.to_string());
        }
        if let Some(v) = self.get("base_url") {
            cfg.set_active_base_url(v.to_string());
        }
    }
}

/// Не показывать секрет целиком.
fn mask(key: &str, value: &str) -> String {
    if key == "api_key" {
        let tail: String = value
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        format!("…{tail} (задан)")
    } else {
        value.to_string()
    }
}

/// Результат обработки команды настроек.
#[derive(Debug, Clone)]
pub struct SettingsOutcome {
    pub message: String,
    /// Требуется ли пересборка LLM-клиента (изменился provider/model/api_key/base_url).
    pub needs_client_rebuild: bool,
}

/// Обработать команду настроек (`/set`, `/unset`, `/config`). Возвращает `None`,
/// если это не команда настроек — тогда вызывающий обрабатывает ввод обычно.
pub fn handle_command(input: &str) -> Option<SettingsOutcome> {
    let t = input.trim();
    let (cmd, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
    let rest = rest.trim();
    match cmd {
        "/set" => Some(cmd_set(rest)),
        "/unset" => Some(cmd_unset(rest)),
        "/config" | "/settings" | "/get" => Some(cmd_show(rest)),
        _ => None,
    }
}

fn cmd_set(rest: &str) -> SettingsOutcome {
    let (key, value) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let key = key.trim().to_lowercase();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return SettingsOutcome {
            message: format!(
                "Использование: /set <ключ> <значение>. Ключи: {}",
                KNOWN_KEYS.join(", ")
            ),
            needs_client_rebuild: false,
        };
    }
    if !KNOWN_KEYS.contains(&key.as_str()) {
        return SettingsOutcome {
            message: format!(
                "Неизвестный ключ «{key}». Доступные: {}",
                KNOWN_KEYS.join(", ")
            ),
            needs_client_rebuild: false,
        };
    }
    let mut s = Settings::load();
    s.set(&key, value);
    let saved = s.save();
    let where_ = settings_path();
    let msg = match saved {
        Ok(()) => format!(
            "{key} = {} (сохранено в {})",
            mask(&key, value),
            where_.display()
        ),
        Err(e) => format!("{key} задан, но не удалось сохранить файл: {e}"),
    };
    SettingsOutcome {
        message: msg,
        needs_client_rebuild: CLIENT_KEYS.contains(&key.as_str()),
    }
}

fn cmd_unset(rest: &str) -> SettingsOutcome {
    let key = rest.trim().to_lowercase();
    if key.is_empty() {
        return SettingsOutcome {
            message: "Использование: /unset <ключ>".into(),
            needs_client_rebuild: false,
        };
    }
    let mut s = Settings::load();
    let removed = s.unset(&key);
    let _ = s.save();
    SettingsOutcome {
        message: if removed {
            format!("Ключ «{key}» сброшен.")
        } else {
            format!("Ключ «{key}» не был задан.")
        },
        needs_client_rebuild: CLIENT_KEYS.contains(&key.as_str()),
    }
}

fn cmd_show(_rest: &str) -> SettingsOutcome {
    let s = Settings::load();
    let body = if s.is_empty() {
        "Пользовательских настроек нет. Задать: /set <ключ> <значение>".to_string()
    } else {
        s.pairs()
            .map(|(k, v)| format!("  {k} = {}", mask(k, v)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    SettingsOutcome {
        message: format!(
            "Настройки ({}):\n{body}\nКлючи: {}",
            settings_path().display(),
            KNOWN_KEYS.join(", ")
        ),
        needs_client_rebuild: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_non_settings_returns_none() {
        assert!(handle_command("/workflows").is_none());
        assert!(handle_command("обычный текст").is_none());
    }

    #[test]
    fn set_unknown_key_is_reported() {
        let out = handle_command("/set nope 1").unwrap();
        assert!(out.message.contains("Неизвестный ключ"));
        assert!(!out.needs_client_rebuild);
    }

    #[test]
    fn set_provider_needs_rebuild() {
        // Изолируем файл настроек во временном пути.
        std::env::set_var(
            "WHATCODE_CONFIG",
            std::env::temp_dir().join("wc_test_settings.toml"),
        );
        let out = handle_command("/set provider anthropic").unwrap();
        assert!(out.needs_client_rebuild);
        let mut cfg = AppConfig::default();
        Settings::load().apply_to(&mut cfg);
        assert_eq!(cfg.llm_provider, LlmProvider::Anthropic);
        // cleanup
        let _ = std::fs::remove_file(settings_path());
        std::env::remove_var("WHATCODE_CONFIG");
    }

    #[test]
    fn api_key_is_masked_in_output() {
        assert_eq!(mask("api_key", "sk-secret1234"), "…1234 (задан)");
        assert_eq!(mask("model", "gpt"), "gpt");
    }
}
