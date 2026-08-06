#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Copyright (c) 2026 Lê Hùng Quang Minh
#
# Build canonical cho release: tách target dir để .so không bị Slint nhiễm.
#
# Mặc định `cargo build --all-features` tạo cdylib libcantype.so bao gồm cả
# Slint (vì cùng crate-type). Build này tách hai target dir:
#   target/fcitx  → libcantype.so (addon, chỉ link Fcitx5, KHÔNG Slint)
#   target/app    → cantype binary (GUI + tray, link Slint, KHÔNG Fcitx5)
#
# Sau build, verify .so không chứa Slint. Dùng cho release và packaging Phase 3.
#
# Tùy chọn:
#   --check   chỉ verify artifact đã build (không rebuild)
#   -h, --help  in hướng dẫn

set -euo pipefail

trap 'echo "build-release.sh: bi ngat" >&2; exit 130' INT TERM

cd "$(dirname "$0")/.."

in_su_dung() {
    cat <<'EOF'
Cài đặt: scripts/build-release.sh [--check]
  --check    chỉ verify artifact đã build (không rebuild)
  -h, --help in hướng dẫn này

Build canonical tách target dir:
  target/fcitx/release/libcantype.so  (addon, không Slint)
  target/app/release/cantype          (GUI + tray, không Fcitx5)

Build --features fcitx5 cần Fcitx5 dev headers (hoặc PKG_CONFIG_PATH).
EOF
}

# --- Parse args ---
check_only=0
for arg in "$@"; do
    case "$arg" in
        --check) check_only=1 ;;
        -h|--help) in_su_dung; exit 0 ;;
        *) echo "build-release.sh: tham so khong hop le: $arg" >&2; in_su_dung; exit 2 ;;
    esac
done

# --- Build fcitx addon (.so) ---
if [[ $check_only -eq 0 ]]; then
    echo "[1/2] build libcantype.so (target/fcitx)..."
    PKG_CONFIG_PATH="${PKG_CONFIG_PATH:-}" \
    CARGO_TARGET_DIR=target/fcitx \
    cargo build --release --lib --no-default-features --features fcitx5
fi

SO="target/fcitx/release/libcantype.so"
if [[ ! -f "$SO" ]]; then
    echo "build-release.sh: khong tim thay $SO. Chay lai (khong --check)." >&2
    exit 1
fi

# --- Build app binary ---
if [[ $check_only -eq 0 ]]; then
    echo "[2/2] build cantype binary (target/app)..."
    CARGO_TARGET_DIR=target/app \
    cargo build --release --bin cantype --no-default-features --features app
fi

APP="target/app/release/cantype"
if [[ ! -f "$APP" ]]; then
    echo "build-release.sh: khong tim thay $APP. Chay lai (khong --check)." >&2
    exit 1
fi

# --- Verify .so không chứa Slint ---
echo "verify libcantype.so khong chua Slint..."
slint_ldd=$(ldd "$SO" 2>&1 | grep -i slint || true)
slint_nm=$(nm -C "$SO" 2>&1 | grep -i slint || true)
if [[ -n "$slint_ldd" ]]; then
    echo "LOI: .so link Slint (ldd):" >&2
    echo "$slint_ldd" >&2
    exit 1
fi
if [[ -n "$slint_nm" ]]; then
    echo "LOI: .so co symbol Slint (nm):" >&2
    echo "$slint_nm" >&2
    exit 1
fi

# --- Verify app không link Fcitx5 ---
echo "verify cantype binary khong link Fcitx5..."
fcitx_ldd=$(ldd "$APP" 2>&1 | grep -i fcitx || true)
if [[ -n "$fcitx_ldd" ]]; then
    echo "LOI: app link Fcitx5 (ldd):" >&2
    echo "$fcitx_ldd" >&2
    exit 1
fi

echo ""
echo "build canonical OK:"
echo "  $SO  ($(stat -c%s "$SO") bytes)"
echo "  $APP  ($(stat -c%s "$APP") bytes)"
echo "  .so: khong Slint, app: khong Fcitx5"
