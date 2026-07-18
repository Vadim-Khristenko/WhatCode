#!/usr/bin/env bash
# WhatCode — сборка под Android.
#
# Кроссплатформенная сборка Rust-бинаря `whatcode` для Android-таргетов через
# NDK (или, если запускается прямо в Termux — нативным clang).
#
# Требуется:
#   - Android NDK (r25+; проверено на r27c). Укажите путь через ANDROID_NDK_HOME
#     или ANDROID_NDK_ROOT. В Termux NDK не нужен — используется системный clang.
#   - rustup target add aarch64-linux-android (и/или другие ABI).
#
# Использование:
#   ANDROID_NDK_HOME=/path/to/ndk ./scripts/build-android.sh              # aarch64, release
#   ANDROID_API=24 ABIS="aarch64 armv7 x86_64" ./scripts/build-android.sh # несколько ABI
#   PROFILE=debug ./scripts/build-android.sh                              # debug-сборка
set -euo pipefail

API="${ANDROID_API:-24}"
PROFILE="${PROFILE:-release}"
ABIS="${ABIS:-aarch64}"
PACKAGE="${PACKAGE:-whatcode-cli}"

# Соответствие короткого имени ABI → Rust target triple + префикс clang.
triple_for() {
  case "$1" in
    aarch64) echo "aarch64-linux-android" ;;
    armv7)   echo "armv7-linux-androideabi" ;;
    x86_64)  echo "x86_64-linux-android" ;;
    x86)     echo "i686-linux-android" ;;
    *) echo "неизвестный ABI: $1" >&2; exit 1 ;;
  esac
}

# clang-обёртка NDK различается для armv7 (androideabi + API).
clang_for() {
  case "$1" in
    aarch64) echo "aarch64-linux-android${API}-clang" ;;
    armv7)   echo "armv7a-linux-androideabi${API}-clang" ;;
    x86_64)  echo "x86_64-linux-android${API}-clang" ;;
    x86)     echo "i686-linux-android${API}-clang" ;;
  esac
}

cargo_env_prefix() { # AARCH64_LINUX_ANDROID из aarch64-linux-android
  echo "$1" | tr '[:lower:]-' '[:upper:]_'
}

IN_TERMUX=0
if [ -n "${TERMUX_VERSION:-}" ] || [ -d "/data/data/com.termux" ]; then
  IN_TERMUX=1
fi

if [ "$IN_TERMUX" -eq 0 ]; then
  NDK="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
  if [ -z "$NDK" ] || [ ! -d "$NDK" ]; then
    echo "ОШИБКА: не задан путь к NDK. Установите ANDROID_NDK_HOME." >&2
    exit 1
  fi
  case "$(uname -s)" in
    Linux)  HOST_TAG="linux-x86_64" ;;
    Darwin) HOST_TAG="darwin-x86_64" ;;
    *) echo "неподдерживаемый хост: $(uname -s)" >&2; exit 1 ;;
  esac
  TOOLBIN="$NDK/toolchains/llvm/prebuilt/$HOST_TAG/bin"
  if [ ! -d "$TOOLBIN" ]; then
    echo "ОШИБКА: не найден toolchain NDK: $TOOLBIN" >&2
    exit 1
  fi
fi

PROFILE_FLAG=""
[ "$PROFILE" = "release" ] && PROFILE_FLAG="--release"

for abi in $ABIS; do
  triple="$(triple_for "$abi")"
  echo ">>> Сборка $PACKAGE для $triple (API $API, $PROFILE)"
  rustup target add "$triple" >/dev/null 2>&1 || true

  if [ "$IN_TERMUX" -eq 0 ]; then
    prefix="$(cargo_env_prefix "$triple")"
    clang="$TOOLBIN/$(clang_for "$abi")"
    export "CARGO_TARGET_${prefix}_LINKER=$clang"
    export "CC_${triple//-/_}=$clang"
    export "AR_${triple//-/_}=$TOOLBIN/llvm-ar"
  fi

  cargo build -p "$PACKAGE" --target "$triple" $PROFILE_FLAG
  echo ">>> Готово: target/$triple/$PROFILE/whatcode"
done
