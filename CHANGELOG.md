# Changelog

Все заметные изменения Rust-версии WhatCode.

## [0.7.0] — 2026-07-18

### Персоны
- Добавлена персона **Хацунэ Мику** (VOCALOID · Crypton Future Media): тёплый,
  жизнерадостный тон с музыкальными метафорами, цвет TUI — бирюзовый (`#39C5BB`).
- Обогащён лор Герты (флот из 281 марионетки, истинная форма 5★, дихотомия
  «интересное/скукотища») и Anis (официальный слоган-шутник, обращение
  «Командир», идол-арка Twinkle Tri-Star).

### Воркфлоу и межагентная кооперация
- Добавлен мульти-агентный движок воркфлоу: producers (веер) → синтез (барьер) →
  опциональная состязательная проверка (скептики голосуют). Встроенные `review`,
  `plan`, `research`, `debug`; пользовательские грузятся из TOML
  (`.whatcode/workflows/*.toml`). Команды TUI `/workflows` и `/workflow <id>`.
- Добавлен мост **Agent Context Protocol**: инструмент `ask_external_agent` и
  команды `/agents` / `/delegate` для делегирования задач другим CLI-агентам
  (`claude -p`, `codex exec`, `gemini -p`, `opencode run`, и др.). Настраивается
  через `WHATCODE_EXTERNAL_AGENTS_*`.

### Навыки
- Реализован расширенный формат навыков: директива `@tags`, поиск `Skill::matches`
  и фильтр `query` у инструмента `list_skills`.
- Добавлены навыки `security-review` (аудит диффа по классам угроз) и
  `pr-description` (структурное описание PR из реального диффа); все навыки
  снабжены тегами.

### Android
- Добавлена кросс-сборка под `aarch64-linux-android` через NDK
  (`scripts/build-android.sh`) и CI-джоб; поддержка нативной сборки в Termux.

### CI/CD
- Исправлена рассинхронизация тулчейна: `rust-toolchain.toml` (1.96.0)
  перекрывал тулчейн, куда добавлялись таргеты, из-за чего кросс-сборки падали с
  E0463. Теперь пиннутый тулчейн ставится явно, таргеты добавляются в него.
- Обновлены версии экшенов (`checkout@v7`), добавлены сводки сборки
  (`$GITHUB_STEP_SUMMARY`) и итоговый статус релиза; надёжная настройка Android NDK.

### Исправления
- `wakeword`: Unicode-корректное приведение к нижнему регистру — кириллические
  wake-фразы (`герта`) снова матчятся с текстом в любом регистре.

### Досье и прочее
- Добавлено досье `docs/MIKU.md`; обновлены `docs/HERTA.md` и `docs/ANIS.md`
  свежим каноном 2025-2026.
- `doctor` показывает доступные персоны и установленных внешних агентов.
- Единое форматирование кода по `rustfmt` во всём воркспейсе.

## [0.6.0] — 2026-06-30

### Ребрендинг
- Проект переименован из `The Herta` / `herta-*` в **WhatCode** / `whatcode-*`.
- Переименованы все crate-ы, бинарь (`whatcode`), env-переменные (`WHATCODE_*`),
  документация, packaging (`flake.nix`, `.deb`, AUR PKGBUILD) и CI/CD.
- Формат навыков переименован из `.herta` в `.skill` (универсальный) и `.whatcode`
  (расширенный).
- Имя персонажа «Великая Герта» сохранено как одна из персон.

### Персоны
- Добавлена абстракция персон: `whatcode-core::persona::{common, herta, anis}`.
- Добавлена персона **Anis** (Goddess of Victory: Nikke).
- Цвета TUI привязаны к персоне: нейтральный — белый, Герта — фиолетовый,
  Anis — жёлтый.

### TUI
- Улучшен адаптивный терминальный интерфейс с переключением акцентных цветов
  под активную персону.
- Улучшен пользовательский опыт и дизайн некоторых частей интерфейса.

### Инструменты
- Рефакторинг `whatcode-tools`: инструменты Git разбиты на `git/read`, `git/write`,
  `git/advanced` с единым `GitContext`.
- Добавлен полный набор Git-инструментов: status, log, diff, diff staged, branches,
  remote, add, reset HEAD, commit, push, pull, checkout, stash, reset, revert,
  rebase, cherry-pick, clean, merge, rollback commit, sync branch, savepoint.
- Добавлены инструменты сборки: `cargo_*`, `uv_*`, `bun_*`, `verify_build`, `project_info`.

### LLM-провайдеры
- Добавлен **Fireworks AI** (OpenAI-совместимый).
- Добавлен **OpenCode Go** (OpenAI-совместимый endpoint `https://opencode.ai/zen/go/v1`).

### Voice
- Добавлен Edge TTS (`TtsProvider::Edge`) через `edge-tts` CLI.
- Добавлен базовый текстовый wake-word detector в `whatcode-core::wakeword`.
- Удалены устаревшие Python-модули: `stt/`, `utils/logger.py`, `tts/edge_tts_engine.py`,
  `wakeword/matcher.py`, `wakeword/coordinator.py`.

### Навыки
- Переписаны `goal-planning`, `context-compaction`, `code-review` с реальными примерами.
- Добавлены `detailed-debugging`, `test-design`, `refactoring`, `git-workflow`.

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
