# Android Build Notes

## Проблема

Сборка `whatcode-cli` под Android (`aarch64-linux-android`) не работает в текущем виде.

## Проверено

- `cargo build --target aarch64-linux-android -p whatcode-core` — **собирается**.
  `whatcode-core` не зависит от desktop-only библиотек и совместим с Android.
- `cargo build --target aarch64-linux-android -p whatcode-cli` — **падает**.

## Причина ошибки

```text
warning: ring@0.17.14: Compiler family detection failed due to error:
  ToolNotFound: failed to find tool "aarch64-linux-android-clang": program not found

error occurred in cc-rs: failed to find tool "clang.exe": program not found
```

Это означает, что в системе **нет Android NDK** и не настроен кросс-компилятор C/C++ для Android.

`ring` (криптография, используется `rustls` → `reqwest`) содержит нативный код и требует `clang` из Android NDK для компиляции под `aarch64-linux-android`.

## Что нужно для базовой сборки

1. Установить Android NDK (например, через Android Studio или sdkmanager).
2. Добавить `rustup target add aarch64-linux-android` (уже сделано).
3. Создать `.cargo/config.toml` с указанием линкера и компилятора:

```toml
[target.aarch64-linux-android]
linker = "<path-to-ndk>/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android30-clang.cmd"

[env]
CC_aarch64_linux_android = "<path-to-ndk>/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android30-clang.cmd"
AR_aarch64_linux_android = "<path-to-ndk>/toolchains/llvm/prebuilt/windows-x86_64/bin/llvm-ar.exe"
```

4. Установить в PATH NDK `bin` или экспортировать `CC_aarch64_linux_android` / `AR_aarch64_linux_android`.

## Более глубокая проблема

Даже после установки NDK `whatcode-cli` **не сможет работать на Android**, потому что:

- `whatcode-tui` использует `crossterm` и `ratatui` — terminal UI библиотеки, неприменимые на Android.
- `whatcode-voice` использует системные TTS/STT процессы (`say`, `espeak-ng`, `powershell`, `whisper`), которых нет на Android.
- `whatcode-cli` — это desktop CLI/TUI приложение, а не Android-приложение.

## Путь к Android

Для реальной Android-поддержки нужно:

1. Создать отдельный crate `whatcode-android` или `whatcode-mobile`.
2. Использовать только Android-совместимые crate-ы:
   - `whatcode-core` (уже совместим),
   - `whatcode-llm`,
   - `whatcode-tools`,
   - `whatcode-agent` (без desktop-специфичных инструментов).
3. Подключить Android UI через `winit` + `android-activity`/`ndk-glue` или отдельный Kotlin/Swift UI, который вызывает Rust через JNI/UniFFI.
4. Для TTS/STT на Android использовать Android SDK (`TextToSpeech`, `SpeechRecognizer`) через JNI или облачные API.
5. Исключить `whatcode-tui` и `whatcode-cli` из Android-сборки.

## Рекомендация

- Если цель — просто проверить, что core-библиотеки собираются под Android: настроить NDK и `.cargo/config.toml`.
- Если цель — полноценное Android-приложение: это отдельный большой проект с мобильным UI и JNI/UniFFI. Требует отдельного планирования.

## Полезные команды

```bash
# Проверить, что core собирается
rustup target add aarch64-linux-android
cargo build --target aarch64-linux-android -p whatcode-core

# Проверить CLI (будет падать без NDK/UI)
cargo build --target aarch64-linux-android -p whatcode-cli
```

## Ссылки

- [Rust Android NDK setup](https://developer.android.com/ndk/guides/other_build_systems)
- [cargo-ndk](https://github.com/bbqsrc/cargo-ndk)
- [Rust on Android with winit](https://github.com/rust-mobile/rust-android-examples)
