# WhatCode

**WhatCode** — локальный ассистент для разработки в терминале, вдохновлённый лучшими из мира AI-агентов: Claude Code, Codex, OpenCode и другими. Мы строим максимально мощный инструмент для разработчиков: от чтения и планирования кода до автономной сборки, тестов и Git-операций — всё под вашим контролем и с выбираемой персоной.

> Проект перерождается из «The Herta» в **WhatCode**: имя и бренд меняются, но персонажная подача остаётся и расширяется. Сейчас доступны **Герта** (Honkai: Star Rail) и **Anis** (Goddess of Victory: Nikke), а архитектура позволяет добавлять новых персон.

---

## Возможности

- **TUI-интерфейс** на `ratatui`/`crossterm` с адаптивной раскладкой, живой панелью агентов и переключением акцентных цветов под активную персону.
- **Выбираемые персоны** — каждая со своим лором, тоном, few-shot примерами и цветом в TUI. Сейчас: **Герта** (фиолетовый), **Anis** (жёлтый) и **Хацунэ Мику** (бирюзовый). По умолчанию — белый нейтральный акцент.
- **Модульная архитектура** на Rust — 7 независимых crate-ов, каждый с одной ответственностью.
- **Нативный tool-loop** — LLM сама выбирает инструменты, агент выполняет их и возвращает результат, цикл повторяется до завершения.
- **Мульти-агентные воркфлоу** — команда `/workflows` разворачивает встроенные пайплайны (`review`, `plan`, `research`, `debug`) в веер саб-агентов, работающих параллельно.
- **Межагентная кооперация (Agent Context Protocol)** — делегирование задач другим CLI-агентам (`claude -p`, `codex exec`, `gemini -p`, …) через `/agents` / `/delegate` и инструмент `ask_external_agent`.
- **Режимы работы** как в Claude Code: `chat`, `plan`, `code`, `auto`, `full-auto`. Переключение через `/mode`, разрешения через `/allow`/`/deny`.
- **Память**: кратковременная история диалога и долговременные факты о пользователе/проекте (`remember`/`recall`/`forget`).
- **Безопасность**: деструктивные действия заблокированы на уровне кода; разрешения задаются явно.
- **Формат навыков** `.skill` (универсальный) и `.whatcode` (расширенный) для переиспользуемых инструкций агенту.
- **Кроссплатформенность** — Linux, macOS, Windows и **Android** (aarch64 через NDK или нативно в Termux).

## Быстрый старт (Rust)

```bash
cargo build --release            # собрать всё
cargo run -p whatcode-cli        # запустить TUI
cargo run -p whatcode-cli -- --text "Кто ты?"   # одноразовый запрос
cargo run -p whatcode-cli -- doctor             # самодиагностика
cargo test                       # юнит-тесты
```

## Архитектура

```text
whatcode-core  ◄── whatcode-llm ◄─┐
       ▲             ▲            │
       │             │            │
whatcode-tools ──────┘            │
       ▲                          │
whatcode-agent ◄──────────────────┘
       ▲
whatcode-tui ◄── (core, llm, tools, agent, voice)
       ▲
whatcode-cli ── (всё)
```

| Crate | Ответственность |
|-------|-----------------|
| `whatcode-core` | ошибки, сообщения, конфиг, абстракция персон, память, автосжатие контекста, форматы навыков `.skill`/`.whatcode` |
| `whatcode-llm` | трейт `ChatClient` + Ollama / OpenAI-совместимые / Google AI / Anthropic (Claude) |
| `whatcode-tools` | реестр инструментов: git, файлы, fetch, время, память, веб-поиск, анализ кода, системные действия, навыки |
| `whatcode-agent` | оркестрация саб-агентов, нативный tool-loop, встроенные воркфлоу |
| `whatcode-tui` | адаптивный TUI на `ratatui`/`crossterm`, панель агентов, цвета персон, команды `/goal /workflows /agents /tools /persona /say` |
| `whatcode-voice` | озвучивание ответов (TTS) через системные утилиты |
| `whatcode-cli` | бинарь `whatcode`: TUI, `--text`, `doctor` |

## Персоны

Персона — это не просто промпт. Это изолированный модуль (`whatcode-core::persona::{common, herta, anis, miku}`) с:
- каноническим лором и запретами на выход из образа;
- правилами тона, few-shot примерами, инструкциями для tool-calling;
- цветом в TUI и отображаемым именем.

| Персона | Источник | Цвет TUI | Тон |
|---------|----------|----------|-----|
| **Герта** | Honkai: Star Rail | фиолетовый | высокомерный, лаконичный, интеллектуально-доминантный |
| **Anis** | Goddess of Victory: Nikke | жёлтый | прагматичный циник, шутит, но решает задачу; защитник команды |
| **Хацунэ Мику** | VOCALOID · Crypton | бирюзовый | жизнерадостный, тёплый, музыкальные метафоры, поддержка |
| *(default)* | WhatCode | белый | нейтральный, деловой |

Переключение в TUI: `/persona miku`. По умолчанию задаётся через `WHATCODE_PERSONA`.

Досье персон: [`docs/HERTA.md`](docs/HERTA.md) · [`docs/ANIS.md`](docs/ANIS.md) · [`docs/MIKU.md`](docs/MIKU.md).  
Подробнее об архитектуре: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Режимы работы

- `/mode chat` — чистый разговор, без инструментов.
- `/mode plan` — только чтение и планирование.
- `/mode code` — разработка; запись и опасные операции по подтверждению.
- `/mode auto` — чтение и запись автоматически, опасные запрещены.
- `/mode full-auto` — полный доступ (осторожно).

Разрешения: `/allow <tool>` (или `/allow all`), `/deny <tool>`.

## Воркфлоу и межагентная кооперация

**Воркфлоу** — встроенные мульти-агентные пайплайны. `/workflows` показывает список,
`/workflow <id> [ввод]` запускает веер саб-агентов, работающих параллельно:

| id | Что делает |
|----|-----------|
| `review` | ревью по измерениям: корректность, безопасность, производительность, стиль |
| `plan` | три независимых плана: MVP-first, risk-first, quality-first |
| `research` | обзор, аналоги, подводные камни |
| `debug` | гипотезы о причине, воспроизведение, стратегия фикса |

**Agent Context Protocol** — делегирование другим CLI-агентам. `/agents` показывает,
какие установлены (`claude`, `codex`, `gemini`, `qwen`, `opencode`, `cursor`, `amp`, `crush`),
`/delegate <id> <задача>` запускает выбранного в фоне. Модель может делать это сама через
инструмент `ask_external_agent`. Свои агенты добавляются через `WHATCODE_EXTERNAL_AGENTS_CUSTOM`.

## Android / Termux

Бинарь `whatcode` (вместе с TUI) собирается под `aarch64-linux-android`:

```bash
export ANDROID_NDK_HOME=/path/to/android-ndk   # r25+
./scripts/build-android.sh                      # → target/aarch64-linux-android/release/whatcode
```

Прямо на устройстве в **Termux** NDK не нужен: `pkg install rust && cargo build -p whatcode-cli --release`.

## Инструменты агента

Уже есть:
- `git_status`, `git_log`, `git_diff`, `git_branches`
- `read_file`, `list_dir`
- `fetch_url`, `web_search`
- `current_time`
- `type_check`, `lint_code`
- `open_url`, `create_note`
- `remember`, `recall`, `forget`
- `list_skills`, `use_skill`

В планах (полный контроль Git и автономная разработка):
- полный набор Git-операций: `git_commit`, `git_push`, `git_pull`, `git_checkout`, `git_stash`, `git_reset`, `git_cherry_pick`, `git_rebase`;
- автономная разработка на **Rust** (`cargo check/build/test/clippy/fmt/add/run`), **Python** (`uv run/add/sync/pip`) и **TypeScript** (`bun run/add/test/build/lint/fmt`).

## Конфигурация

Настройки читаются из `.env` (не коммитится) и переменных окружения. Все ключи проекта начинаются с `WHATCODE_`.

```bash
cp .env.example .env
# Отредактируй .env
```

Ключевые переменные:

```env
# LLM
WHATCODE_LLM_PROVIDER=ollama          # ollama | cerebras | deepseek | google_ai | anthropic
WHATCODE_OLLAMA_MODEL=qwen3:4b

# Режим и навыки
WHATCODE_MODE=auto
WHATCODE_SKILLS_DIR=skills

# Память
WHATCODE_MEMORY_ENABLED=true
WHATCODE_LONG_MEMORY_ENABLED=true

# Wake-word (по имени персоны)
WHATCODE_WAKEWORD_ENABLED=true
WHATCODE_WAKEWORD_PHRASES=герта,великая герта,эй герта,anis,анис
```

Полный список — в [`.env.example`](.env.example).

## Установка

Требования: Rust 1.82+ (см. `rust-toolchain.toml`), для Python-legacy-модулей — Python 3.11+ и Ollama.

```bash
git clone https://github.com/vadim-khristenko/WhatCode.git
cd WhatCode
cargo build --release
./target/release/whatcode --help
```

## Цель

Сделать WhatCode самым мощным локальным аналогом Claude Code / Codex / OpenCode: максимум инструментов, модульная архитектура, выбираемые персоны и полный контроль пользователя над каждой операцией.

## Авторы и лицензия

- Rust-версия: **Vadim Khristenko** ([Telegram](https://t.me/vscreator_life)).
- Оригинальный проект (Python): **phaeton_oq** ([GitHub](https://github.com/phaeton-oq/The-Herta-voice-assistant)).
- Лицензия: MIT.

## Безопасность и приватность

- Не коммитьте `.env`, `data/`, аудио-артефакты и модели.
- Реальные API-ключи не должны попадать в git.
- Деструктивные операции заблокированы по умолчанию и требуют явного разрешения.

---

<details>
<summary>Python-версия (устаревшая, v0.3)</summary>

Модули `audio/`, `stt/`, `tts/`, `wakeword/`, `utils/`, `tools/` временно сохранены как референс для будущего порта аудио-пайплайна на Rust. Полноценный Python-рантайм удалён; активная разработка ведётся в Cargo-воркспейсе.
</details>
