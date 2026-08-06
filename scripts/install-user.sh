#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Copyright (c) 2026 Lê Hùng Quang Minh
#
# Cài CanType addon Fcitx5 vào thư mục user (~/.local) cho developer mode.
#
# Không dùng sudo. Không sửa shell profile. Không sửa /etc/environment. Không
# kill process. Idempotent: chạy nhiều lần an toàn. In từng file đã cài.
#
# System-wide packaging chuyển Phase 3. Developer mode cần set
# FCITX_ADDON_DIRS=~/.local/lib/fcitx5 khi chạy fcitx5 (xem run-fcitx-dev.sh).
#
# Tùy chọn:
#   --cleanup-old   xóa artifact CadenceRuntime cũ (libcadence_runtime.so,
#                   cadence-runtime.conf) trước khi cài CanType
#   -h, --help      in hướng dẫn

set -euo pipefail

# Thoát nếu curl/nhấn Ctrl-C.
trap 'echo "install-user.sh: bi ngat" >&2; exit 130' INT TERM

# --- Đường dẫn ---
readonly TIEU_DE_SO="$(pwd)/target/release/libcantype.so"
readonly TIEU_DE_DEBUG="$(pwd)/target/debug/libcantype.so"
readonly ADDON_CONF="$(pwd)/data/fcitx5/cantype-addon.conf"
readonly IM_CONF="$(pwd)/data/fcitx5/cantype-inputmethod.conf"

# Thư mục user theo XDG (fallback ~/.local).
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
readonly LIB_DIR="$HOME/.local/lib/fcitx5"
readonly ADDON_DIR="$data_home/fcitx5/addon"
readonly IM_DIR="$data_home/fcitx5/inputmethod"

# --- Hàm ---
in_su_dung() {
    cat <<'EOF'
Cài đặt: scripts/install-user.sh [--cleanup-old]
  --cleanup-old  xóa artifact CadenceRuntime cũ trước khi cài CanType
  -h, --help     in hướng dẫn này

Yêu cầu: đã build 'cargo build --release --no-default-features --features fcitx5'
EOF
}

# Xác minh file .so tồn tại, ưu tiên release.
lay_so() {
    if [[ -f "$TIEU_DE_SO" ]]; then
        echo "$TIEU_DE_SO"
    elif [[ -f "$TIEU_DE_DEBUG" ]]; then
        echo "install-user.sh: canh bao: dung build debug (target/debug)" >&2
        echo "$TIEU_DE_DEBUG"
    else
        echo "install-user.sh: khong tim thay libcantype.so. Chay:" >&2
        echo "  cargo build --release --no-default-features --features fcitx5" >&2
        exit 1
    fi
}

# Cleanup artifact CadenceRuntime cũ. Chỉ xóa file có tên chính xác do dự án cũ
# tạo — không glob rộng, không xóa addon khác.
cleanup_old() {
    echo "cleanup artifact CadenceRuntime cu:"
    local f
    # .so cũ (tên lib + crate name cũ, không có prefix lib ở crate output).
    for f in "$LIB_DIR/cadence_runtime.so" "$LIB_DIR/libcadence_runtime.so"; do
        if [[ -f "$f" ]]; then
            echo "  xoa $f"
            rm -f "$f"
        fi
    done
    # Metadata addon/inputmethod cũ.
    for f in "$ADDON_DIR/cadence-runtime.conf" "$IM_DIR/cadence-runtime.conf"; do
        if [[ -f "$f" ]]; then
            echo "  xoa $f"
            rm -f "$f"
        fi
    done
}

# --- Parse args ---
cleanup_old_flag=0
for arg in "$@"; do
    case "$arg" in
        --cleanup-old) cleanup_old_flag=1 ;;
        -h|--help) in_su_dung; exit 0 ;;
        *) echo "install-user.sh: tham so khong hop le: $arg" >&2; in_su_dung; exit 2 ;;
    esac
done

# --- Thực thi ---
if [[ $cleanup_old_flag -eq 1 ]]; then
    cleanup_old
fi

echo "cai CanType addon vao thu muc user:"
mkdir -p "$LIB_DIR" "$ADDON_DIR" "$IM_DIR"

so_path="$(lay_so)"
echo "  copy $so_path -> $LIB_DIR/libcantype.so"
cp -f "$so_path" "$LIB_DIR/libcantype.so"

echo "  copy $ADDON_CONF -> $ADDON_DIR/cantype.conf"
cp -f "$ADDON_CONF" "$ADDON_DIR/cantype.conf"

echo "  copy $IM_CONF -> $IM_DIR/cantype.conf"
cp -f "$IM_CONF" "$IM_DIR/cantype.conf"

echo ""
echo "cai xong. Chay fcitx5 dev voi:"
echo "  scripts/run-fcitx-dev.sh"
echo ""
echo "Luu y: developer mode can FCITX_ADDON_DIRS=~/.local/lib/fcitx5 de Fcitx5"
echo "tim libcantype.so. System-wide packaging chuyen Phase 3."
