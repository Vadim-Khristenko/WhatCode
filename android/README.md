# WhatCode — Android UI-обёртка

Простое и удобное Android-приложение поверх нативного бинаря WhatCode. Это не
терминал: чат-интерфейс (ввод → ответ), где каждый запрос выполняется через
одноразовый режим `whatcode --text`.

## Как это устроено

- Нативный бинарь `whatcode` (собранный под `aarch64-linux-android`) кладётся в
  APK как `lib/arm64-v8a/libwhatcode.so`. Это единственный поддерживаемый способ
  держать исполняемый файл в приложении: файлы из `nativeLibraryDir` разрешено
  запускать (`android:extractNativeLibs="true"`, `useLegacyPackaging = true`).
- `MainActivity` запускает бинарь через `ProcessBuilder` и стримит вывод в ленту.
- Пакет: `space.vairice.whatcode`. Подпись — debug-ключ (APK ставится сайдлоадом).

## Сборка

Бинарь и APK собираются в CI (`.github/workflows`). Локально:

```bash
# 1. Собрать нативный бинарь под Android (нужен NDK):
ANDROID_NDK_HOME=/path/to/ndk ./scripts/build-android.sh

# 2. Положить его в приложение и собрать APK (нужен Android SDK):
cp target/aarch64-linux-android/release/whatcode \
   android/app/src/main/jniLibs/arm64-v8a/libwhatcode.so
cd android
echo "sdk.dir=$ANDROID_SDK_ROOT" > local.properties
gradle :app:assembleRelease
# → android/app/build/outputs/apk/release/app-release.apk
```

## Ограничения

- Полный TUI живёт в терминале (Termux + бинарь). APK даёт простой GUI-чат.
- Для развёрнутых ответов задайте LLM-провайдера через окружение (см.
  `.env.example`); офлайн работают быстрые ответы об идентичности персоны.
