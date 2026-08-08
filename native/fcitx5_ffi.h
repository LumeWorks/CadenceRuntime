// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

/// Hợp đồng C ABI giữa C++ shim Fcitx5 (`native/fcitx5.cpp`) và Rust runtime
/// (`src/ffi.rs`). Chỉ truyền kiểu POD; không truyền String/Vec/exception qua
/// ABI. Mọi chuỗi đi qua ABI dưới dạng `CanTypeSlice { ptr, len }` (con trỏ +
/// độ dài byte UTF-8), ownership rõ ràng theo từng hàm.
///
/// Chiều dữ liệu:
///   C++ KeyEvent/InputContext → CanTypeKeySnapshot/CanTypeContextSnapshot →
///   Rust (src/ffi.rs) → PhienNhap + FcitxHost → KetQuaXuLy → C++ accept/filter.
///
/// C++ gọi Rust qua `cantype_*`. Rust gọi C++ qua bảng callback
/// `CanTypeHostBang` (function pointers + opaque `ic`).

#ifndef CANTYPE_FCITX5_FFI_H
#define CANTYPE_FCITX5_FFI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Một lát cắt byte UTF-8 không sở hữu (con trỏ + độ dài).
typedef struct {
    const uint8_t *ptr;
    size_t len;
} CanTypeSlice;

/// Phân loại phím đặc biệt do C++ điền từ `Key::sym()` (giữ keysym knowledge
/// trong C++, Rust chỉ dùng logic). Mọi keysym khác là `CanTypeKeyKhac`.
typedef enum {
    CanTypeKeyKhac = 0,
    CanTypeKeyBackspace = 1,
    CanTypeKeyEscape = 2,
    CanTypeKeyEnter = 3,
    CanTypeKeyTab = 4,
    CanTypeKeyDelete = 5,
} CanTypeKeyDacBiet;

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
    CanTypeKeyDacBiet dac_biet;
    CanTypeSlice utf8;
} CanTypeKeySnapshot;

/// Snapshot ngữ cảnh nhập do C++ điền cho `Host::boi_canh`.
///
/// `cursor`/`anchor` là offset **ký tự** (code point), theo `SurroundingText`.
/// `text` là toàn bộ surrounding text (UTF-8), có thể bị C++ cắt giới hạn.
///
/// `frontend`/`program`/`capability` là metadata Phase 3: dùng cho frontend
/// classification và diagnostic logging (KHÔNG cho verify-before-mutate).
/// `frontend` là `ic->frontend()` (const char*), `program` là `ic->program()`
/// (std::string), `capability` là `ic->capabilityFlags().toInteger()`.
typedef struct {
    uint64_t context_id;
    uint64_t focus_generation;
    int has_focus;         /* bool */
    int surrounding_valid; /* bool */
    uint32_t cursor;       /* offset ký tự */
    uint32_t anchor;       /* offset ký tự */
    CanTypeSlice text;      /* surrounding text UTF-8 */
    CanTypeSlice frontend;  /* ic->frontend(), const char* (NUL-terminated) */
    CanTypeSlice program;   /* ic->program(), std::string UTF-8 */
    uint64_t capability;    /* ic->capabilityFlags().toInteger() */
} CanTypeContextSnapshot;

/// Bảng callback C++ cấp cho Rust triển khai `Host`. `ic` là opaque
/// `fcitx::InputContext*`; Rust chỉ truyền lại cho callback, không deref.
typedef struct {
    void *ic;
    /* Điền `out` từ InputContext. Trả 0=ok, nonzero=lỗi. */
    int (*lay_boi_canh)(void *ic, CanTypeContextSnapshot *out);
    /* Commit string (UTF-8). Trả 0=DaPhat, 1=KhongPhat, 2=KhongChac. */
    int (*chen)(void *ic, const uint8_t *ptr, size_t len);
    /* Xóa `xoa_ky_tu` ký tự trước con trỏ rồi commit string. Trả 0/1/2. */
    int (*thay_the)(void *ic, uint32_t xoa_ky_tu, const uint8_t *ptr, size_t len);
    /* Cập nhật client preedit (PlainComposition, NO formatting flags).
       Trả 0/1/2. */
    int (*cap_nhat_soan_thao)(void *ic, const uint8_t *ptr, size_t len);
    /* Kết thúc composition: commit text rồi clear preedit (PlainComposition).
       Trả 0/1/2. */
    int (*ket_thuc_soan_thao)(void *ic, const uint8_t *ptr, size_t len);
    /* Xóa client preedit (PlainComposition). Trả 0/1/2. */
    int (*xoa_soan_thao)(void *ic);
} CanTypeHostBang;

/// Kết quả xử lý phím (KetQuaXuLy). C++ dùng để quyết accept/filter.
///   DaApDung    → filterAndAccept (không forward physical key)
///   ChuyenTiep  → không filter, không accept (forward physical key)
///   MatDongBo   → filterAndAccept (không forward, host có thể đã phát một phần)
typedef enum {
    CanTypeXuLy_ChuyenTiep = 0,
    CanTypeXuLy_DaApDung = 1,
    CanTypeXuLy_MatDongBo = 2,
} CanTypeXuLyKetQua;

/* --- Hàm Rust xuất sang C++ (triển khai trong src/ffi.rs) --- */

/// Tạo `PhienNhap` mới cho `context_id`, trả opaque handle. Trả NULL nếu lỗi.
void *cantype_phien_tao(uint64_t context_id);

/// Giải phóng `PhienNhap` handle. An toàn với NULL. Mỗi handle đúng một lần.
void cantype_phien_giai_phong(void *phien);

/// Đặt lại phiên (relinquish): dùng cho reset/deactivate/focus-out lifecycle.
void cantype_phien_dat_lai(void *phien);

/// Xử lý một phím. `phien` phải hợp lệ (không NULL). `bang` callback table.
/// Trả `CanTypeXuLyKetQua`. Panic trong Rust không vượt FFI (catch_unwind).
CanTypeXuLyKetQua cantype_xu_ly_phim(void *phien, const CanTypeKeySnapshot *key,
                                     const CanTypeHostBang *bang);

/// Entry factory do Fcitx loader dlsym. Triển khai trong Rust (`#[no_mangle]`)
/// để lọt vào dynsym export của cdylib Rust, gọi `cantype_native_factory` của
/// C++ trả static `AddonFactory*`. Lý do: Rust cdylib chỉ export symbol
/// `#[no_mangle]` Rust; nếu định nghĩa `fcitx_addon_factory_instance` trong C++
/// thì linker localize nó (không vào dynsym).
void *fcitx_addon_factory_instance(void);

/* --- Hàm C++ xuất sang Rust (triển khai trong native/fcitx5.cpp) --- */

/// Trả static `fcitx::AddonFactory*` (CanTypeFactory). Rust
/// `fcitx_addon_factory_instance` gọi hàm này. Function-local static,
/// thread-safe init (C++11).
void *cantype_native_factory(void);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* CANTYPE_FCITX5_FFI_H */
