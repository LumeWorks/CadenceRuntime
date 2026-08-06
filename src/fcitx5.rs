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

/// Lát cắt byte UTF-8 không sở hữu (khớp `CadenceSlice` trong C ABI).
#[repr(C)]
pub struct CadenceSlice {
    /// Con trỏ byte, hợp lệ trong thời gian lời gọi FFI.
    pub ptr: *const u8,
    /// Số byte.
    pub len: usize,
}

/// Phân loại phím đặc biệt (khớp `CadenceKeyDacBiet`).
///
/// Giá trị do C++ điền từ `Key::sym()`; Rust chỉ đọc (match). Các biến thể
/// "không construct" theo góc nhìn Rust vì C++ tạo chúng — `allow(dead_code)`
/// cho ABI type.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CadenceKeyDacBiet {
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
}

/// Snapshot phím (khớp `CadenceKeySnapshot`). C++ điền từ `KeyEvent`/`Key`.
#[repr(C)]
pub struct CadenceKeySnapshot {
    /// `true` (≠0) nếu là key release.
    pub is_release: i32,
    /// `true` nếu arrow/page (cursor move).
    pub is_cursor_move: i32,
    /// `true` nếu phím modifier thuần.
    pub is_modifier: i32,
    /// `true` nếu có Ctrl/Alt/Super/Hyper/Meta (không tính Shift).
    pub has_modifier: i32,
    /// Phím đặc biệt.
    pub dac_biet: CadenceKeyDacBiet,
    /// `keySymToUTF8` của normalized key; rỗng nếu không printable.
    pub utf8: CadenceSlice,
}

/// Snapshot ngữ cảnh nhập (khớp `CadenceContextSnapshot`).
#[repr(C)]
pub struct CadenceContextSnapshot {
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
    pub text: CadenceSlice,
}

/// Kết quả xử lý phím (khớp `CadenceXuLyKetQua`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CadenceXuLyKetQua {
    /// Chuyển tiếp (passthrough).
    ChuyenTiep = 0,
    /// Đã áp dụng (host DaPhat, state tiến).
    DaApDung = 1,
    /// Mất đồng bộ.
    MatDongBo = 2,
}

/// Bảng callback C++ (khớp `CadenceHostBang`). Rust chỉ truyền lại `ic` cho
/// callback, không deref. Việc gọi callback (unsafe) nằm trong [`ffi`](crate::ffi).
#[repr(C)]
pub struct CadenceHostBang {
    /// Opaque `fcitx::InputContext*`.
    pub ic: *mut core::ffi::c_void,
    /// Callback điền snapshot ngữ cảnh.
    pub lay_boi_canh:
        Option<extern "C" fn(*mut core::ffi::c_void, *mut CadenceContextSnapshot) -> i32>,
    /// Callback commit string.
    pub chen: Option<extern "C" fn(*mut core::ffi::c_void, *const u8, usize) -> i32>,
    /// Callback xóa `xoa_ky_tu` ký tự trước con trỏ rồi commit.
    pub thay_the: Option<extern "C" fn(*mut core::ffi::c_void, u32, *const u8, usize) -> i32>,
}

/// Phím dạng Rust-friendly (không con trỏ). [`ffi`](crate::ffi) chuyển từ
/// [`CadenceKeySnapshot`] (unsafe) sang kiểu này trước khi gọi [`anh_xa_phim`].
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
    pub dac_biet: CadenceKeyDacBiet,
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
/// 7. **Cursor move** (arrow/page) → [`SuKienNhap::DiChuyenConTro`].
/// 8. **Space** → [`SuKienNhap::RanhGioiTu`] — kết thúc composition, chèn space.
/// 9. **Printable khác** → [`SuKienNhap::KyTu`] — đưa vào Cadence.
/// 10. **Khác** → [`AnhXaPhim::BoQua`].
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
    // 4-6. Phím đặc biệt.
    match key.dac_biet {
        CadenceKeyDacBiet::Backspace => return AnhXaPhim::SuKien(SuKienNhap::XoaLui),
        CadenceKeyDacBiet::Escape => return AnhXaPhim::SuKien(SuKienNhap::DatLai),
        CadenceKeyDacBiet::Enter => return AnhXaPhim::SuKien(SuKienNhap::DatLai),
        CadenceKeyDacBiet::Tab => return AnhXaPhim::SuKien(SuKienNhap::DatLai),
        CadenceKeyDacBiet::Khac => {}
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
pub(crate) fn chuyen_boi_canh(snapshot: &CadenceContextSnapshot, text: &str) -> BoiCanhNhap {
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
