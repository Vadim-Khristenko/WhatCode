# Changelog

Все заметные изменения Rust-версии «Великой Герты».

## [0.5.0] — 2026-06-16

Первый полноценный релиз Rust-порта.

### Провайдеры LLM
- Ollama (локально), OpenAI-совместимые (Cerebras, DeepSeek), Google AI (Gemini/Gemma),
  **Anthropic (Claude)** — единый трейт `ChatClient`, повторы с backoff, очистка `<think>`.

### Агент и инструменты
- Нативный провайдер-агностичный tool-loop.
- Режимы (как в Claude Code): `chat` / `plan` / `code` / `auto` / `full-auto`
  с системой разрешений (`/allow`, `/deny`, ledger «одобрить все похожие»).
- Инструменты: git (status/log/diff/branches/show/grep/add/commit), файлы
  (read/list/write/append), `fetch_url`, `current_time`, веб-поиск, анализ кода,
  системные действия, память (`remember`/`recall`+поиск/`forget`), навыки.
- Автономная разработка: Rust (`cargo_check/build/test/clippy/fmt/add/run`),
  Python через UV (`uv_run/add/sync/pip`), `install_toolchain` (rustup/uv/python;
  Windows — winget incl. VS Build Tools), `check_toolchain`.

### Голос
- TTS: System (say/espeak/PowerShell), **ElevenLabs** (голос по умолчанию
  `ZYcSL3av41fQqtckDugo`), **Google Cloud**, **Microsoft Azure**, **Qwen/DashScope**.
- STT по аудиофайлу (локально и в облаке): **Whisper (локально)**,
  **OpenAI-совместимый** (OpenAI/Groq/Qwen), **Deepgram**, **Azure**, **Google Cloud**.
  Команда `/transcribe <файл>`. Живой захват с микрофона (cpal) — следующая итерация.

### Память и контекст
- Кратко-/долговременная память; факты с важностью (салиентные — выше в промпте).
- Движок автосжатия контекста + **авто-recap** (тумблер `/recap on|off`, `/recap` сейчас).

### Персона и навыки
- Канонический лор Honkai: Star Rail в персоне; собственный формат навыков `.herta`
  (`list_skills`/`use_skill`): context-compaction, goal-planning, code-review.

### TUI
- ratatui/crossterm, единый `tokio::select!`-цикл, панель саб-агентов, индикатор
  контекста и режима. Команды: `/goal /ask /agent /tools /mode /allow /deny
  /compact /recap /transcribe /say /model /clear /quit`.

### Сборка и релизы
- GitHub Actions по тегу `v*`: Windows, macOS (x86_64+arm), Linux (x86_64+arm),
  `.deb`, AUR `PKGBUILD`, Nix flake, Android (best-effort). CI: fmt + clippy + тесты
  на 3 ОС, тулчейн закреплён на 1.96.0.

### Авторы
- Rust-версия — Vadim Khristenko (<https://t.me/vscreator_life>).
- Оригинал (Python) — phaeton_oq
  (<https://github.com/phaeton-oq/The-Herta-voice-assistant>, <https://t.me/cmd_phaeton_oq>).
