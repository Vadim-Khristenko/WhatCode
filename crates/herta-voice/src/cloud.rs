//! Облачный синтез речи: ElevenLabs и Google Cloud TTS.
//! Аудио получается по HTTP, пишется во временный файл и проигрывается системным
//! плеером — без нативных аудио-зависимостей.

use base64::Engine;
use herta_core::config::{TtsProvider, VoiceConfig};
use serde_json::json;
use std::process::Stdio;
use std::time::Duration;

/// Синтезировать речь и проиграть её. Возвращает текстовую ошибку при сбое.
pub async fn synthesize_and_play(
    http: &reqwest::Client,
    cfg: &VoiceConfig,
    provider: TtsProvider,
    text: &str,
) -> Result<(), String> {
    let (bytes, ext) = match provider {
        TtsProvider::ElevenLabs => (elevenlabs(http, cfg, text).await?, "mp3"),
        TtsProvider::GoogleCloud => (google(http, cfg, text).await?, "mp3"),
        TtsProvider::System => return Err("system-провайдер не использует облако".into()),
    };
    let path = write_temp(&bytes, ext)?;
    play(&path)
}

async fn elevenlabs(
    http: &reqwest::Client,
    cfg: &VoiceConfig,
    text: &str,
) -> Result<Vec<u8>, String> {
    let key = cfg
        .elevenlabs_api_key
        .as_deref()
        .ok_or("нет ELEVENLABS_API_KEY")?;
    let voice_id = cfg
        .elevenlabs_voice_id
        .as_deref()
        .unwrap_or("21m00Tcm4TlvDq8ikWAM");
    let model = cfg
        .elevenlabs_model
        .as_deref()
        .unwrap_or("eleven_multilingual_v2");
    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{voice_id}");
    let resp = http
        .post(&url)
        .header("xi-api-key", key)
        .header("accept", "audio/mpeg")
        .json(&json!({ "text": text, "model_id": model }))
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("сеть ElevenLabs: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("ElevenLabs HTTP {}", resp.status().as_u16()));
    }
    Ok(resp.bytes().await.map_err(|e| e.to_string())?.to_vec())
}

async fn google(http: &reqwest::Client, cfg: &VoiceConfig, text: &str) -> Result<Vec<u8>, String> {
    let key = cfg
        .google_api_key
        .as_deref()
        .ok_or("нет GOOGLE_TTS_API_KEY")?;
    let language = cfg.google_language.as_deref().unwrap_or("ru-RU");
    let voice = cfg.google_voice.as_deref().unwrap_or("ru-RU-Standard-A");
    let url = format!("https://texttospeech.googleapis.com/v1/text:synthesize?key={key}");
    let resp = http
        .post(&url)
        .json(&json!({
            "input": { "text": text },
            "voice": { "languageCode": language, "name": voice },
            "audioConfig": { "audioEncoding": "MP3" }
        }))
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("сеть Google TTS: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Google TTS HTTP {}", resp.status().as_u16()));
    }
    let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let b64 = value
        .get("audioContent")
        .and_then(|v| v.as_str())
        .ok_or("нет audioContent в ответе")?;
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("base64: {e}"))
}

fn write_temp(bytes: &[u8], ext: &str) -> Result<std::path::PathBuf, String> {
    let mut path = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path.push(format!("herta-tts-{nanos}.{ext}"));
    std::fs::write(&path, bytes).map_err(|e| format!("запись temp: {e}"))?;
    Ok(path)
}

/// Проиграть аудиофайл доступным системным плеером.
fn play(path: &std::path::Path) -> Result<(), String> {
    let p = path.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    let candidates: Vec<(&str, Vec<String>)> = vec![("afplay", vec![p.clone()])];

    #[cfg(target_os = "windows")]
    let candidates: Vec<(&str, Vec<String>)> = vec![(
        "powershell",
        vec![
            "-NoProfile".into(),
            "-Command".into(),
            format!("(New-Object Media.SoundPlayer '{p}').PlaySync()"),
        ],
    )];

    #[cfg(all(unix, not(target_os = "macos")))]
    let candidates: Vec<(&str, Vec<String>)> = vec![
        (
            "ffplay",
            vec![
                "-nodisp".into(),
                "-autoexit".into(),
                "-loglevel".into(),
                "quiet".into(),
                p.clone(),
            ],
        ),
        (
            "mpv",
            vec!["--no-video".into(), "--really-quiet".into(), p.clone()],
        ),
        ("mpg123", vec!["-q".into(), p.clone()]),
        (
            "cvlc",
            vec![
                "--play-and-exit".into(),
                "--intf".into(),
                "dummy".into(),
                p.clone(),
            ],
        ),
    ];

    for (player, args) in &candidates {
        if crate::which(player) {
            let spawned = std::process::Command::new(player)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            if spawned.is_ok() {
                return Ok(());
            }
        }
    }
    Err("не найден аудиоплеер (ffplay/mpv/afplay/...)".into())
}
