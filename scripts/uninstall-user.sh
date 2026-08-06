#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Copyright (c) 2026 Lê Hùng Quang Minh
#
# Gỡ cài CanType addon Fcitx5 khỏi thư mục user (~/.local).
#
# Không dùng sudo. Không sửa shell profile. Không sửa /etc/environment. Không
# kill process. Idempotent: chạy nhiều lần an toàn (không lỗi nếu file không
# tồn tại). Chỉ xóa file có tên chính xác thuộc CanType hoặc artifact
# CadenceRuntime cũ — không glob rộng, không xóa addon khác.
#
# Tùy chọn:
#   --cleanup-old   cũng xóa artifact CadenceRuntime cũ (libcadence_runtime.so,
#                   cadence-runtime.conf)
#   -h, --help      in hướng dẫn

set -euo pipefail

trap 'echo "uninstall-user.sh: bi ngat" >&2; exit 130' INT TERM

# --- Đường dẫn user theo XDG ---
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
readonly LIB_DIR="$HOME/.local/lib/fcitx5"
readonly ADDON_DIR="$data_home/fcitx5/addon"
readonly IM_DIR="$data_home/fcitx5/inputmethod"

# --- Hàm ---
in_su_dung() {
    cat <<'EOF'
Cài đặt: scripts/uninstall-user.sh [--cleanup-old]
  --cleanup-old  cũng xóa artifact CadenceRuntime cũ
  -h, --help     in hướng dẫn này
EOF
}

# Xóa một file nếu tồn tại, in thông báo. Không lỗi nếu không tồn tại.
xoa_neu_ton_tai() {
    local f="$1"
    if [[ -f "$f" ]]; then
        echo "  xoa $f"
        rm -f "$f"
    fi
}

# --- Parse args ---
cleanup_old_flag=0
for arg in "$@"; do
    case "$arg" in
        --cleanup-old) cleanup_old_flag=1 ;;
        -h|--help) in_su_dung; exit 0 ;;
        *) echo "uninstall-user.sh: tham so khong hop le: $arg" >&2; in_su_dung; exit 2 ;;
    esac
done

# --- Thực thi ---
echo "go cai CanType addon khoi thu muc user:"

# Chỉ xóa file có tên chính xác thuộc CanType.
xoa_neu_ton_tai "$LIB_DIR/libcantype.so"
xoa_neu_ton_tai "$ADDON_DIR/cantype.conf"
xoa_neu_ton_tai "$IM_DIR/cantype.conf"

if [[ $cleanup_old_flag -eq 1 ]]; then
    echo "cleanup artifact CadenceRuntime cu:"
    # .so cũ: cả tên crate output (cadence_runtime.so) và dạng lib prefix.
    xoa_neu_ton_tai "$LIB_DIR/cadence_runtime.so"
    xoa_neu_ton_tai "$LIB_DIR/libcadence_runtime.so"
    # Metadata addon/inputmethod cũ.
    xoa_neu_ton_tai "$ADDON_DIR/cadence-runtime.conf"
    xoa_neu_ton_tai "$IM_DIR/cadence-runtime.conf"
fi

echo "go xong."
echo "Luu y: khong restart fcitx5 tu dong. Chay 'fcitx5-remote -r' neu can."
