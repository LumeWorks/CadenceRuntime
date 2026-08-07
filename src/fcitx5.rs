// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Adapter Fcitx5 — logic thuần Rust, không `unsafe`.
//!
//! Module chứa ánh xạ phím sang [`SuKienNhap`] và trích xuất [`BoiCanhNhap`]
//! từ snapshot ngữ cảnh. Kiểu `#[repr(C)]` khớp `native/fcitx5_ffi.h`. Phần
//! `unsafe` (đọc con trỏ C++, gọi callback) nằm trong [`ffi`](crate::ffi).
//!
//! Module luôn compile (thuần Rust, không phụ thuộc Fcitx) để test key mapping
//! và surrounding chạy không cần Fcitx5 dev. Khi feature `fcitx5` tắt, các kiểu
//! và hàm không được `ffi` dùng → `allow(dead_code)` (chỉ `ffi` dùng, và `ffi`
//! gate theo feature).

#![cfg_attr(not(feature = "fcitx5"), allow(dead_code))]

use crate::host::{BoiCanhNhap, ContextId};
use crate::phien::SuKienNhap;

/// Lát cắt byte UTF-8 không sở hữu (khớp `CanTypeSlice` trong C ABI).
#[repr(C)]
pub struct CanTypeSlice {
    /// Con trỏ byte, hợp lệ trong thời gian lời gọi FFI.
    pub ptr: *const u8,
    /// Số byte.
    pub len: usize,
}

/// Phân loại phím đặc biệt (khớp `CanTypeKeyDacBiet`).
///
/// Giá trị do C++ điền từ `Key::sym()`; Rust chỉ đọc (match). Các biến thể
/// "không construct" theo góc nhìn Rust vì C++ tạo chúng — `allow(dead_code)`
/// cho ABI type.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CanTypeKeyDacBiet {
    /// Không đặc biệt.
    Khac = 0,
    /// Backspace.
    Backspace = 1,
    /// Escape.
    Escape = 2,
    /// Enter/Return.
    Enter = 3,
    /// Tab.
    Tab = 4,
    /// Delete (Forward Delete). Không phải Backspace — không đưa U+007F vào
    /// Cadence; runtime relinquish rồi passthrough để app tự xóa phía trước.
    Delete = 5,
}

/// Snapshot phím (khớp `CanTypeKeySnapshot`). C++ điền từ `KeyEvent`/`Key`.
#[repr(C)]
pub struct CanTypeKeySnapshot {
    /// `true` (≠0) nếu là key release.
    pub is_release: i32,
    /// `true` nếu arrow/page (cursor move).
    pub is_cursor_move: i32,
    /// `true` nếu phím modifier thuần.
    pub is_modifier: i32,
    /// `true` nếu có Ctrl/Alt/Super/Hyper/Meta (không tính Shift).
    pub has_modifier: i32,
    /// Phím đặc biệt.
    pub dac_biet: CanTypeKeyDacBiet,
    /// `keySymToUTF8` của normalized key; rỗng nếu không printable.
    pub utf8: CanTypeSlice,
}

/// Snapshot ngữ cảnh nhập (khớp `CanTypeContextSnapshot`).
#[repr(C)]
pub struct CanTypeContextSnapshot {
    /// Context id.
    pub context_id: u64,
    /// Thế hệ focus.
    pub focus_generation: u64,
    /// `true` (≠0) nếu đang có focus.
    pub has_focus: i32,
    /// `true` (≠0) nếu surrounding text hợp lệ.
    pub surrounding_valid: i32,
    /// Offset con trỏ theo ký tự.
    pub cursor: u32,
    /// Offset anchor theo ký tự.
    pub anchor: u32,
    /// Toàn bộ surrounding text (UTF-8).
    pub text: CanTypeSlice,
    /// `ic->frontend()` (const char* NUL-terminated) — Phase 3 metadata cho
    /// frontend classification và diagnostic. KHÔNG dùng cho verify-before-
    /// mutate (chỉ metadata, không phải text user gõ).
    pub frontend: CanTypeSlice,
    /// `ic->program()` (std::string UTF-8) — Phase 3 metadata. Tên binary app.
    pub program: CanTypeSlice,
    /// `ic->capabilityFlags().toInteger()` — bitmask CapabilityFlag. Phase 3
    /// dùng để phát hiện Password/Sensitive/Terminal/SurroundingText.
    pub capability: u64,
}

/// Kết quả xử lý phím (khớp `CanTypeXuLyKetQua`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanTypeXuLyKetQua {
    /// Chuyển tiếp (passthrough).
    ChuyenTiep = 0,
    /// Đã áp dụng (host DaPhat, state tiến).
    DaApDung = 1,
    /// Mất đồng bộ.
    MatDongBo = 2,
}

/// Bảng callback C++ (khớp `CanTypeHostBang`). Rust chỉ truyền lại `ic` cho
/// callback, không deref. Việc gọi callback (unsafe) nằm trong [`ffi`](crate::ffi).
#[repr(C)]
pub struct CanTypeHostBang {
    /// Opaque `fcitx::InputContext*`.
    pub ic: *mut core::ffi::c_void,
    /// Callback điền snapshot ngữ cảnh.
    pub lay_boi_canh:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut CanTypeContextSnapshot) -> i32>,
    /// Callback commit string.
    pub chen: Option<extern "C" fn(*mut core::ffi::c_void, *const u8, usize) -> i32>,
    /// Callback xóa `xoa_ky_tu` ký tự trước con trỏ rồi commit.
    pub thay_the: Option<extern "C" fn(*mut core::ffi::c_void, u32, *const u8, usize) -> i32>,
}

/// Phím dạng Rust-friendly (không con trỏ). [`ffi`](crate::ffi) chuyển từ
/// [`CanTypeKeySnapshot`] (unsafe) sang kiểu này trước khi gọi [`anh_xa_phim`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhimRust {
    /// `true` nếu là key release.
    pub is_release: bool,
    /// `true` nếu arrow/page (cursor move).
    pub is_cursor_move: bool,
    /// `true` nếu phím modifier thuần (Shift/Ctrl/Alt press).
    pub is_modifier: bool,
    /// `true` nếu có Ctrl/Alt/Super/Hyper/Meta (không tính Shift).
    pub has_modifier: bool,
    /// Phím đặc biệt.
    pub dac_biet: CanTypeKeyDacBiet,
    /// Ký tự printable từ `keySymToUTF8`; `None` nếu không printable.
    pub ky_tu: Option<char>,
}

/// Kết quả ánh xạ phím: bỏ qua (passthrough, không động vào phiên) hoặc một
/// sự kiện runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AnhXaPhim {
    /// Passthrough: không gọi `PhienNhap::xu_ly` (release, modifier-only).
    BoQua,
    /// Xử lý sự kiện này qua `PhienNhap::xu_ly`.
    SuKien(SuKienNhap),
}

/// Ánh xạ [`PhimRust`] sang [`AnhXaPhim`].
///
/// Thứ tự kiểm tra:
/// 1. **Release** → [`AnhXaPhim::BoQua`] — không xử lý phím nhả.
/// 2. **Modifier-only** (Shift/Ctrl/Alt press thuần) → [`AnhXaPhim::BoQua`] —
///    không đưa modifier vào Cadence.
/// 3. **Shortcut** (Ctrl/Alt/Super/Hyper/Meta + key) → [`AnhXaPhim::BoQua`] —
///    để ứng dụng xử lý shortcut.
/// 4. **Backspace** → [`SuKienNhap::XoaLui`].
/// 5. **Escape** → [`SuKienNhap::DatLai`] — relinquish + passthrough.
/// 6. **Enter/Tab** → [`SuKienNhap::DatLai`] — relinquish + passthrough (không
///    commit Enter/Tab, để app xử lý).
/// 7. **Delete** (forward) → [`SuKienNhap::DatLai`] — relinquish + passthrough
///    (không biến thành Backspace, không commit U+007F, để app xóa phía trước).
/// 8. **Cursor move** (arrow/page) → [`SuKienNhap::DiChuyenConTro`].
/// 9. **Space** → [`SuKienNhap::RanhGioiTu`] — kết thúc composition, chèn space.
/// 10. **Printable khác** → [`SuKienNhap::KyTu`] — đưa vào Cadence.
/// 11. **Khác** → [`AnhXaPhim::BoQua`].
pub(crate) fn anh_xa_phim(key: &PhimRust) -> AnhXaPhim {
    // 1. Release: passthrough.
    if key.is_release {
        return AnhXaPhim::BoQua;
    }
    // 2. Modifier-only press: passthrough (không đưa modifier vào Cadence).
    if key.is_modifier {
        return AnhXaPhim::BoQua;
    }
    // 3. Shortcut (Ctrl/Alt/Super/Hyper/Meta): passthrough.
    if key.has_modifier {
        return AnhXaPhim::BoQua;
    }
    // 4-8. Phím đặc biệt.
    match key.dac_biet {
        CanTypeKeyDacBiet::Backspace => return AnhXaPhim::SuKien(SuKienNhap::XoaLui),
        CanTypeKeyDacBiet::Escape => return AnhXaPhim::SuKien(SuKienNhap::DatLai),
        CanTypeKeyDacBiet::Enter => return AnhXaPhim::SuKien(SuKienNhap::DatLai),
        CanTypeKeyDacBiet::Tab => return AnhXaPhim::SuKien(SuKienNhap::DatLai),
        // Delete phía trước: relinquish composition rồi passthrough — không
        // biến thành Backspace (xóa lùi trong Cadence) và không commit U+007F.
        CanTypeKeyDacBiet::Delete => return AnhXaPhim::SuKien(SuKienNhap::DatLai),
        CanTypeKeyDacBiet::Khac => {}
    }
    // 7. Cursor move: relinquish + passthrough.
    if key.is_cursor_move {
        return AnhXaPhim::SuKien(SuKienNhap::DiChuyenConTro);
    }
    // 8-9. Printable.
    match key.ky_tu {
        Some(' ') => AnhXaPhim::SuKien(SuKienNhap::RanhGioiTu(' ')),
        Some(c) => AnhXaPhim::SuKien(SuKienNhap::KyTu(c)),
        None => AnhXaPhim::BoQua,
    }
}

/// Trích xuất [`BoiCanhNhap`] từ snapshot ngữ cảnh.
///
/// `text` là surrounding text UTF-8 (đã đọc an toàn bởi [`ffi`](crate::ffi)).
/// `cursor`/`anchor` trong snapshot là offset **ký tự** (code point); hàm này
/// cắt text trước con trỏ: `text[..cursor_byte]` sau khi chuyển offset ký tự
/// sang offset byte. Nếu surrounding không hợp lệ hoặc cursor ngoài range →
/// `van_ban_truoc_con_tro = None`.
pub(crate) fn chuyen_boi_canh(snapshot: &CanTypeContextSnapshot, text: &str) -> BoiCanhNhap {
    let van_ban_truoc_con_tro = if snapshot.surrounding_valid != 0 {
        // cursor là offset ký tự (code point). Chuyển sang offset byte UTF-8.
        // Nếu cursor vượt text → None (không verify được).
        let cursor_byte = text
            .char_indices()
            .nth(u32_to_usize(snapshot.cursor))
            .map(|(byte, _)| byte)
            .or_else(|| {
                // cursor == len (ký tự) → byte offset == text.len()
                if u32_to_usize(snapshot.cursor) == text.chars().count() {
                    Some(text.len())
                } else {
                    None
                }
            });
        cursor_byte.map(|cb| text[..cb].to_string())
    } else {
        None
    };
    BoiCanhNhap {
        context_id: ContextId(snapshot.context_id),
        the_he_focus: snapshot.focus_generation,
        dang_co_focus: snapshot.has_focus != 0,
        van_ban_truoc_con_tro,
    }
}

/// Chuyển `u32` sang `usize` (trên nền tảng 32/64-bit đều an toàn).
fn u32_to_usize(v: u32) -> usize {
    usize::try_from(v).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    //! Unit test cho `anh_xa_phim`: ánh xạ phím đặc biệt (đặc biệt Delete).

    use super::*;

    /// Tạo `PhimRust` press mặc định (không modifier, không release).
    fn phim_press(dac_biet: CanTypeKeyDacBiet, ky_tu: Option<char>) -> PhimRust {
        PhimRust {
            is_release: false,
            is_cursor_move: false,
            is_modifier: false,
            has_modifier: false,
            dac_biet,
            ky_tu,
        }
    }

    /// Delete press → `DatLai` (relinquish + passthrough), KHÔNG `XoaLui`.
    /// Đảm bảo Delete không bị biến thành Backspace trong Cadence.
    #[test]
    fn delete_press_sang_dat_lai_khong_xoa_lui() {
        let phim = phim_press(CanTypeKeyDacBiet::Delete, None);
        let kq = anh_xa_phim(&phim);
        assert_eq!(kq, AnhXaPhim::SuKien(SuKienNhap::DatLai));
        assert_ne!(kq, AnhXaPhim::SuKien(SuKienNhap::XoaLui));
    }

    /// Delete release → `BoQua` (passthrough, không động vào phiên).
    #[test]
    fn delete_release_sang_bo_qua() {
        let phim = PhimRust {
            is_release: true,
            is_cursor_move: false,
            is_modifier: false,
            has_modifier: false,
            dac_biet: CanTypeKeyDacBiet::Delete,
            ky_tu: None,
        };
        assert_eq!(anh_xa_phim(&phim), AnhXaPhim::BoQua);
    }

    /// Delete có `ky_tu = Some('\u{7f}')` (U+007F từ keySymToUTF8) vẫn map
    /// sang `DatLai`, không đưa DEL vào Cadence. Regression: trước fix,
    /// Delete bị `dac_biet = Khac` → DEL được commit như ký tự in được.
    #[test]
    fn delete_voi_utf8_del_van_dat_lai_khong_commit() {
        let phim = phim_press(CanTypeKeyDacBiet::Delete, Some('\u{7f}'));
        let kq = anh_xa_phim(&phim);
        assert_eq!(kq, AnhXaPhim::SuKien(SuKienNhap::DatLai));
        // Không phải KyTu('\u{7f}') — phải là DatLai.
        assert_ne!(kq, AnhXaPhim::SuKien(SuKienNhap::KyTu('\u{7f}')));
    }

    /// Backspace press → `XoaLui` (không bị nhầm với Delete).
    #[test]
    fn backspace_press_sang_xoa_lui() {
        let phim = phim_press(CanTypeKeyDacBiet::Backspace, None);
        assert_eq!(anh_xa_phim(&phim), AnhXaPhim::SuKien(SuKienNhap::XoaLui));
    }
}
