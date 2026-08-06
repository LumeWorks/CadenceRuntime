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
#   --cleanup-old     xóa artifact CadenceRuntime cũ (libcadence_runtime.so,
#                     cadence-runtime.conf) trước khi cài CanType
#   --enable-profile  thêm CanType vào ~/.config/fcitx5/profile (group Default).
#                     Backup atomic, validate, rollback nếu lỗi. Mặc định chỉ
#                     in hướng dẫn mở fcitx5-configtool — không sửa profile.
#   -h, --help        in hướng dẫn

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
Cài đặt: scripts/install-user.sh [--cleanup-old] [--enable-profile]
  --cleanup-old     xóa artifact CadenceRuntime cũ trước khi cài CanType
  --enable-profile  thêm CanType vào profile fcitx5 (backup + validate)
  -h, --help        in hướng dẫn này

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
    # Thư mục số nhiều cũ (sai tên) do phiên bản script trước tạo.
    for d in "$data_home/fcitx5/addons" "$data_home/fcitx5/inputmethods"; do
        if [[ -d "$d" ]]; then
            echo "  xoa thu muc cu sai ten: $d"
            rmdir "$d" 2>/dev/null || true
        fi
    done
}

# Đường dẫn profile fcitx5 (XDG config).
readonly PROFILE_FCITX="${FCITX_CONFIG_HOME:-${XDG_CONFIG_HOME:-$HOME/.config}}/fcitx5/profile"

# Kiểm tra CanType đã có trong profile fcitx5 chưa. Trả 0=đã có, 1=chưa có/lỗi.
kiem_tra_profile() {
    if [[ ! -f "$PROFILE_FCITX" ]]; then
        return 1
    fi
    # Match chính xác `Name=cantype` (không match `Name=cantype2`).
    grep -q '^Name=cantype$' "$PROFILE_FCITX" 2>/dev/null
}

# In hướng dẫn kích hoạt CanType qua fcitx5-configtool (không sửa profile).
huong_dan_profile() {
    echo ""
    echo "!!! CanType chua co trong profile fcitx5. Fcitx5 khong switch duoc."
    echo "!!! Them CanType bang mot trong cac cach:"
    echo "  1. Mo fcitx5-configtool -> Configure -> them 'CanType' vao group"
    echo "  2. Chay lai script voi --enable-profile:"
    echo "       scripts/install-user.sh --enable-profile"
    echo "  3. Suy ra: fcitx5-remote -s cantype se that bai (silent) cho den khi"
    echo "     CanType nam trong danh sach input method cua group hien tai."
}

# Thêm CanType vào profile fcitx5 (group Default) với backup + validate + rollback.
# Chỉ gọi khi --enable-profile và kiem_tra_profile trả 1 (chưa có).
them_profile() {
    if [[ ! -f "$PROFILE_FCITX" ]]; then
        echo "install-user.sh: profile khong ton tai: $PROFILE_FCITX" >&2
        echo "  Chay fcitx5 mot lan de tao profile mac dinh, roi chay lai." >&2
        return 1
    fi

    local backup
    backup="$PROFILE_FCITX.bak.cantype-$(date +%Y%m%d%H%M%S)"
    cp -f "$PROFILE_FCITX" "$backup"
    echo "  backup profile -> $backup"

    # Tìm số item lớn nhất trong [Groups/0/Items/N].
    local so_item
    so_item=$(grep -oE '^\[Groups/0/Items/[0-9]+\]' "$PROFILE_FCITX" \
              | grep -oE '[0-9]+' | sort -n | tail -1)
    if [[ -z "$so_item" ]]; then
        so_item=-1
    fi
    local item_moi=$((so_item + 1))

    # Chèn item mới trước [GroupOrder], giữ nguyên phần còn lại.
    local tmp
    tmp=$(mktemp)
    if ! awk -v item="$item_moi" '
        BEGIN { da_chen=0 }
        /^\[GroupOrder\]/ && !da_chen {
            printf "[Groups/0/Items/%s]\n", item
            print "# Name"
            print "Name=cantype"
            print "# Layout"
            print "Layout="
            print ""
            da_chen=1
        }
        { print }
        END {
            if (!da_chen) {
                printf "[Groups/0/Items/%s]\n", item
                print "# Name"
                print "Name=cantype"
                print "# Layout"
                print "Layout="
                print ""
            }
        }
    ' "$PROFILE_FCITX" > "$tmp"; then
        echo "install-user.sh: awk tao profile that bai, rollback" >&2
        rm -f "$tmp"
        cp -f "$backup" "$PROFILE_FCITX"
        return 1
    fi

    # Validate: file tạm phải có [Groups/0], [GroupOrder], và Name=cantype.
    if ! grep -q '^\[Groups/0\]' "$tmp" \
       || ! grep -q '^\[GroupOrder\]' "$tmp" \
       || ! grep -q '^Name=cantype$' "$tmp"; then
        echo "install-user.sh: validate profile that bai, rollback" >&2
        rm -f "$tmp"
        cp -f "$backup" "$PROFILE_FCITX"
        return 1
    fi

    # Validate: không thêm trùng (chỉ một Name=cantype).
    local so_luong
    so_luong=$(grep -c '^Name=cantype$' "$tmp" || true)
    if [[ "$so_luong" -ne 1 ]]; then
        echo "install-user.sh: cantype xuat hien $so_luong lan (trung?), rollback" >&2
        rm -f "$tmp"
        cp -f "$backup" "$PROFILE_FCITX"
        return 1
    fi

    # Atomic move.
    mv -f "$tmp" "$PROFILE_FCITX"
    echo "  them cantype vao group Default (item $item_moi)"
    return 0
}

# --- Parse args ---
cleanup_old_flag=0
enable_profile_flag=0
for arg in "$@"; do
    case "$arg" in
        --cleanup-old) cleanup_old_flag=1 ;;
        --enable-profile) enable_profile_flag=1 ;;
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
echo "Luu y: developer mode can FCITX_ADDON_DIRS gom ca user dir (~/.local/"
echo "lib/fcitx5) va system dir (/usr/lib/.../fcitx5) de Fcitx5 tim ca"
echo "libcantype.so lan frontend xcb/dbus. Script run-fcitx-dev.sh tu set."

# --- Kiểm tra/Thêm CanType vào profile fcitx5 ---
if [[ $enable_profile_flag -eq 1 ]]; then
    echo ""
    echo "them CanType vao profile fcitx5:"
    if kiem_tra_profile; then
        echo "  CanType da co trong profile, khong can them."
    else
        if ! them_profile; then
            echo "install-user.sh: them profile that bai" >&2
            exit 1
        fi
    fi
else
    if ! kiem_tra_profile; then
        huong_dan_profile
    else
        echo ""
        echo "CanType da co trong profile fcitx5."
    fi
fi
