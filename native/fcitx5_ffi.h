// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

/// Hợp đồng C ABI giữa C++ shim Fcitx5 (`native/fcitx5.cpp`) và Rust runtime
/// (`src/ffi.rs`). Chỉ truyền kiểu POD; không truyền String/Vec/exception qua
/// ABI. Mọi chuỗi đi qua ABI dưới dạng `CadenceSlice { ptr, len }` (con trỏ +
/// độ dài byte UTF-8), ownership rõ ràng theo từng hàm.
///
/// Chiều dữ liệu:
///   C++ KeyEvent/InputContext → CadenceKeySnapshot/CadenceContextSnapshot →
///   Rust (src/ffi.rs) → PhienNhap + FcitxHost → KetQuaXuLy → C++ accept/filter.
///
/// C++ gọi Rust qua `cadence_*`. Rust gọi C++ qua bảng callback
/// `CadenceHostBang` (function pointers + opaque `ic`).

#ifndef CADENCE_FCITX5_FFI_H
#define CADENCE_FCITX5_FFI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Một lát cắt byte UTF-8 không sở hữu (con trỏ + độ dài).
typedef struct {
    const uint8_t *ptr;
    size_t len;
} CadenceSlice;

/// Phân loại phím đặc biệt do C++ điền từ `Key::sym()` (giữ keysym knowledge
/// trong C++, Rust chỉ dùng logic). Mọi keysym khác là `CadenceKeyKhac`.
typedef enum {
    CadenceKeyKhac = 0,
    CadenceKeyBackspace = 1,
    CadenceKeyEscape = 2,
    CadenceKeyEnter = 3,
    CadenceKeyTab = 4,
} CadenceKeyDacBiet;

/// Snapshot phím tại thời điểm xử lý. C++ điền từ `KeyEvent`/`Key`.
///
/// `has_modifier` = có Ctrl/Alt/Super/Hyper/Meta (KHÔNG tính Shift): shortcut.
/// `is_modifier` = phím modifier thuần (Shift/Ctrl/Alt/Super/CapsLock press).
/// `is_cursor_move` = arrow/page (từ `Key::isCursorMove`).
/// `utf8` = `Key::keySymToUTF8(key.normalize().sym())`; rỗng nếu không printable.
typedef struct {
    int is_release;        /* bool */
    int is_cursor_move;    /* bool */
    int is_modifier;       /* bool */
    int has_modifier;      /* bool: Ctrl/Alt/Super/Hyper/Meta, không Shift */
    CadenceKeyDacBiet dac_biet;
    CadenceSlice utf8;
} CadenceKeySnapshot;

/// Snapshot ngữ cảnh nhập do C++ điền cho `Host::boi_canh`.
///
/// `cursor`/`anchor` là offset **ký tự** (code point), theo `SurroundingText`.
/// `text` là toàn bộ surrounding text (UTF-8), có thể bị C++ cắt giới hạn.
typedef struct {
    uint64_t context_id;
    uint64_t focus_generation;
    int has_focus;         /* bool */
    int surrounding_valid; /* bool */
    uint32_t cursor;       /* offset ký tự */
    uint32_t anchor;       /* offset ký tự */
    CadenceSlice text;      /* surrounding text UTF-8 */
} CadenceContextSnapshot;

/// Bảng callback C++ cấp cho Rust triển khai `Host`. `ic` là opaque
/// `fcitx::InputContext*`; Rust chỉ truyền lại cho callback, không deref.
typedef struct {
    void *ic;
    /* Điền `out` từ InputContext. Trả 0=ok, nonzero=lỗi. */
    int (*lay_boi_canh)(void *ic, CadenceContextSnapshot *out);
    /* Commit string (UTF-8). Trả 0=DaPhat, 1=KhongPhat, 2=KhongChac. */
    int (*chen)(void *ic, const uint8_t *ptr, size_t len);
    /* Xóa `xoa_ky_tu` ký tự trước con trỏ rồi commit string. Trả 0/1/2. */
    int (*thay_the)(void *ic, uint32_t xoa_ky_tu, const uint8_t *ptr, size_t len);
} CadenceHostBang;

/// Kết quả xử lý phím (KetQuaXuLy). C++ dùng để quyết accept/filter.
///   DaApDung    → filterAndAccept (không forward physical key)
///   ChuyenTiep  → không filter, không accept (forward physical key)
///   MatDongBo   → filterAndAccept (không forward, host có thể đã phát một phần)
typedef enum {
    CadenceXuLy_ChuyenTiep = 0,
    CadenceXuLy_DaApDung = 1,
    CadenceXuLy_MatDongBo = 2,
} CadenceXuLyKetQua;

/* --- Hàm Rust xuất sang C++ (triển khai trong src/ffi.rs) --- */

/// Tạo `PhienNhap` mới cho `context_id`, trả opaque handle. Trả NULL nếu lỗi.
void *cadence_phien_tao(uint64_t context_id);

/// Giải phóng `PhienNhap` handle. An toàn với NULL. Mỗi handle đúng một lần.
void cadence_phien_giai_phong(void *phien);

/// Đặt lại phiên (relinquish): dùng cho reset/deactivate/focus-out lifecycle.
void cadence_phien_dat_lai(void *phien);

/// Xử lý một phím. `phien` phải hợp lệ (không NULL). `bang` callback table.
/// Trả `CadenceXuLyKetQua`. Panic trong Rust không vượt FFI (catch_unwind).
CadenceXuLyKetQua cadence_xu_ly_phim(void *phien, const CadenceKeySnapshot *key,
                                     const CadenceHostBang *bang);

/// Entry factory do Fcitx loader dlsym. Triển khai trong Rust (`#[no_mangle]`)
/// để попад vào version script export của cdylib Rust, gọi `cadence_native_
/// factory` của C++ trả static `AddonFactory*`. Lý do: Rust cdylib chỉ export
/// symbol `#[no_mangle]` Rust; nếu định nghĩa `fcitx_addon_factory_instance`
/// trong C++ thì linker localize nó (không vào dynsym).
void *fcitx_addon_factory_instance(void);

/* --- Hàm C++ xuất sang Rust (triển khai trong native/fcitx5.cpp) --- */

/// Trả static `fcitx::AddonFactory*` (CadenceFactory). Rust
/// `fcitx_addon_factory_instance` gọi hàm này. Function-local static,
/// thread-safe init (C++11).
void *cadence_native_factory(void);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* CADENCE_FCITX5_FFI_H */
