//! Распознавание речи (STT) по аудиофайлу — локально и в облаке.
//!
//! Провайдеры:
//! - **WhisperLocal** — локальный `whisper` (openai-whisper) офлайн;
//! - **OpenAiCompatible** — `/audio/transcriptions` (OpenAI, Groq, Qwen-omni);
//! - **Deepgram**, **Azure**, **GoogleCloud** — облачные REST.
//!
//! Вход — путь к аудиофайлу (wav/mp3/flac/…); выход — текст. Захват с микрофона
//! (cpal) — отдельная итерация; здесь работает транскрипция готовых файлов как
//! локально, так и через облако.

use base64::Engine;
use whatcode_core::config::{SttConfig, SttProvider};
use serde_json::json;
use std::time::Duration;

/// Распознаватель речи.
#[derive(Debug, Clone)]
pub struct Stt {
    cfg: SttConfig,
    http: reqwest::Client,
}

impl Stt {
    pub fn from_config(cfg: &SttConfig) -> Self {
        Self {
            cfg: cfg.clone(),
            http: reqwest::Client::new(),
        }
    }

    pub fn provider(&self) -> SttProvider {
        self.cfg.provider
    }

    /// Транскрибировать аудиофайл в текст.
    pub async fn transcribe_file(&self, path: &str) -> Result<String, String> {
        if !std::path::Path::new(path).is_file() {
            return Err(format!("файл не найден: {path}"));
        }
        match self.cfg.provider {
            SttProvider::WhisperLocal => self.whisper_local(path).await,
            SttProvider::OpenAiCompatible => self.openai_compatible(path).await,
            SttProvider::Deepgram => self.deepgram(path).await,
            SttProvider::Azure => self.azure(path).await,
            SttProvider::GoogleCloud => self.google(path).await,
        }
    }

    /// Локальный Whisper (openai-whisper CLI). Полностью офлайн.
    async fn whisper_local(&self, path: &str) -> Result<String, String> {
        let cmd = self
            .cfg
            .whisper_command
            .clone()
            .unwrap_or_else(|| "whisper".to_string());
        let model = self
            .cfg
            .whisper_model
            .clone()
            .unwrap_or_else(|| "base".to_string());
        let out_dir = std::env::temp_dir().join(format!("whatcode-stt-{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&out_dir).await;

        let mut args = vec![
            path.to_string(),
            "--model".into(),
            model,
            "--output_format".into(),
            "txt".into(),
            "--output_dir".into(),
            out_dir.to_string_lossy().to_string(),
        ];
        if let Some(lang) = &self.cfg.language {
            args.push("--language".into());
            args.push(lang.clone());
        }

        let status = tokio::process::Command::new(&cmd)
            .args(&args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .map_err(|e| format!("не удалось запустить `{cmd}`: {e}. Установите: uv pip install -U openai-whisper"))?;
        if !status.success() {
            return Err(format!("`{cmd}` завершился с ошибкой"));
        }

        // openai-whisper кладёт <имя>.txt в output_dir.
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let txt = out_dir.join(format!("{stem}.txt"));
        match tokio::fs::read_to_string(&txt).await {
            Ok(text) => Ok(text.trim().to_string()),
            Err(e) => Err(format!(
                "не найден результат whisper ({}): {e}",
                txt.display()
            )),
        }
    }

    /// OpenAI-совместимый `/audio/transcriptions` (multipart).
    async fn openai_compatible(&self, path: &str) -> Result<String, String> {
        let key = self.cfg.api_key.as_deref().ok_or("нет STT_API_KEY")?;
        let base = self
            .cfg
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1");
        let model = self.cfg.model.as_deref().unwrap_or("whisper-1");
        let url = format!("{}/audio/transcriptions", base.trim_end_matches('/'));

        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| format!("чтение файла: {e}"))?;
        let filename = std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("audio.wav")
            .to_string();
        let part = reqwest::multipart::Part::bytes(bytes).file_name(filename);
        let mut form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", model.to_string());
        if let Some(lang) = &self.cfg.language {
            form = form.text("language", lang.clone());
        }

        let resp = self
            .http
            .post(&url)
            .bearer_auth(key)
            .multipart(form)
            .timeout(Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("сеть STT: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("STT HTTP {}", resp.status().as_u16()));
        }
        let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        Ok(value
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string())
    }

    /// Deepgram prerecorded.
    async fn deepgram(&self, path: &str) -> Result<String, String> {
        let key = self.cfg.api_key.as_deref().ok_or("нет DEEPGRAM_API_KEY")?;
        let model = self.cfg.model.as_deref().unwrap_or("nova-2");
        let mut url = format!("https://api.deepgram.com/v1/listen?model={model}&smart_format=true");
        if let Some(lang) = &self.cfg.language {
            url.push_str(&format!("&language={lang}"));
        }
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| format!("чтение файла: {e}"))?;
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Token {key}"))
            .header("Content-Type", "audio/*")
            .body(bytes)
            .timeout(Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("сеть Deepgram: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("Deepgram HTTP {}", resp.status().as_u16()));
        }
        let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        Ok(value
            .pointer("/results/channels/0/alternatives/0/transcript")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string())
    }

    /// Microsoft Azure Speech-to-Text (короткое аудио, WAV).
    async fn azure(&self, path: &str) -> Result<String, String> {
        let key = self.cfg.api_key.as_deref().ok_or("нет AZURE_SPEECH_KEY")?;
        let region = self
            .cfg
            .azure_region
            .as_deref()
            .ok_or("нет AZURE_SPEECH_REGION")?;
        let lang = self.cfg.language.as_deref().unwrap_or("ru-RU");
        let url = format!(
            "https://{region}.stt.speech.microsoft.com/speech/recognition/conversation/cognitiveservices/v1?language={lang}"
        );
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| format!("чтение файла: {e}"))?;
        let resp = self
            .http
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", key)
            .header(
                "Content-Type",
                "audio/wav; codecs=audio/pcm; samplerate=16000",
            )
            .body(bytes)
            .timeout(Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("сеть Azure STT: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("Azure STT HTTP {}", resp.status().as_u16()));
        }
        let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        Ok(value
            .get("DisplayText")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string())
    }

    /// Google Cloud Speech-to-Text (заголовок файла определяет кодек).
    async fn google(&self, path: &str) -> Result<String, String> {
        let key = self.cfg.api_key.as_deref().ok_or("нет GOOGLE_AI_API_KEY")?;
        let lang = self.cfg.language.as_deref().unwrap_or("ru-RU");
        let url = format!("https://speech.googleapis.com/v1/speech:recognize?key={key}");
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| format!("чтение файла: {e}"))?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let resp = self
            .http
            .post(&url)
            .json(&json!({
                "config": { "languageCode": lang, "enableAutomaticPunctuation": true },
                "audio": { "content": b64 }
            }))
            .timeout(Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("сеть Google STT: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("Google STT HTTP {}", resp.status().as_u16()));
        }
        let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        Ok(value
            .pointer("/results/0/alternatives/0/transcript")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string())
    }
}
