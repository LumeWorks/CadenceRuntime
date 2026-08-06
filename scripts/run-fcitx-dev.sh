#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Copyright (c) 2026 Lê Hùng Quang Minh
#
# Chạy fcitx5 ở chế độ developer với CanType addon từ ~/.local/lib/fcitx5.
#
# Set FCITX_ADDON_DIRS để Fcitx5 quét thư mục user tìm libcantype.so. Không kill
# process tùy tiện: nếu fcitx5 đang chạy, chỉ reload addon qua fcitx5-remote -r;
# nếu không chạy, start fcitx5 mới.
#
# Yêu cầu: đã chạy scripts/install-user.sh trước đó.
#
# Tùy chọn:
#   -r, --reload    chỉ reload addon nếu fcitx5 đang chạy (không start mới)
#   -h, --help      in hướng dẫn

set -euo pipefail

trap 'echo "run-fcitx-dev.sh: bi ngat" >&2; exit 130' INT TERM

readonly LIB_DIR="$HOME/.local/lib/fcitx5"

in_su_dung() {
    cat <<'EOF'
Cài đặt: scripts/run-fcitx-dev.sh [-r|--reload]
  -r, --reload  chỉ reload addon nếu fcitx5 đang chạy (không start mới)
  -h, --help    in hướng dẫn này

Yêu cầu: đã chạy scripts/install-user.sh trước đó.
EOF
}

# --- Parse args ---
reload_only=0
for arg in "$@"; do
    case "$arg" in
        -r|--reload) reload_only=1 ;;
        -h|--help) in_su_dung; exit 0 ;;
        *) echo "run-fcitx-dev.sh: tham so khong hop le: $arg" >&2; in_su_dung; exit 2 ;;
    esac
done

# --- Xác minh install ---
if [[ ! -f "$LIB_DIR/libcantype.so" ]]; then
    echo "run-fcitx-dev.sh: khong tim thay $LIB_DIR/libcantype.so" >&2
    echo "Chay scripts/install-user.sh truoc." >&2
    exit 1
fi

# --- Set env dev ---
export FCITX_ADDON_DIRS="$LIB_DIR"
echo "FCITX_ADDON_DIRS=$FCITX_ADDON_DIRS"

# --- Kiểm tra fcitx5 đang chạy ---
if fcitx5-remote -q >/dev/null 2>&1; then
    echo "fcitx5 dang chay. Reload addon..."
    fcitx5-remote -r
    echo "da reload. CanType addon (neu da chon input method) se nap lai."
    exit 0
fi

if [[ $reload_only -eq 1 ]]; then
    echo "fcitx5 khong chay va da chon -r/--reload: khong start moi." >&2
    exit 1
fi

# --- Start fcitx5 mới ---
echo "fcitx5 khong chay. Start fcitx5 dev..."
echo "(bam Ctrl-C de dung; fcitx5 se tu thoat)"
exec fcitx5 -d --replace
