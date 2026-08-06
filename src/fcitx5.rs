// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Adapter Fcitx5 — logic thuần Rust, không `unsafe`.
//!
//! Module chứa ánh xạ `KeyEvent` (snapshot C ABI) sang [`SuKienNhap`] và trích
//! xuất `BoiCanhNhap` từ snapshot ngữ cảnh. Kiểu `#[repr(C)]` khớp đúng
//! `native/fcitx5_ffi.h`. Phần `unsafe` (gọi callback C++, deref con trỏ) nằm
//! trong [`ffi`](crate::ffi).
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

/// Kết quả ánh xạ phím: bỏ qua (passthrough, không động vào phiên) hoặc một
/// sự kiện runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AnhXaPhim {
    /// Passthrough: không gọi `PhienNhap::xu_ly` (release, modifier-only).
    BoQua,
    /// Xử lý sự kiện này qua `PhienNhap::xu_ly`.
    SuKien(SuKienNhap),
}

/// Ánh xạ `CadenceKeySnapshot` sang [`AnhXaPhim`].
///
/// Commit này là bản tối thiểu: release → [`AnhXaPhim::BoQua`], mọi phím khác →
/// [`SuKienNhap::DatLai`] (relinquish + passthrough). Key mapping đầy đủ
/// (printable, shift, backspace, space, arrow, shortcut, ...) ở commit kế.
pub(crate) fn anh_xa_phim(key: &CadenceKeySnapshot) -> AnhXaPhim {
    // Release: không xử lý Cadence, passthrough.
    if key.is_release != 0 {
        return AnhXaPhim::BoQua;
    }
    // Stub: mọi phím press → DatLai (relinquish composition + passthrough).
    AnhXaPhim::SuKien(SuKienNhap::DatLai)
}

/// Trích xuất [`BoiCanhNhap`] từ snapshot ngữ cảnh.
///
/// Commit này là bản tối thiểu: surrounding luôn `None`. Trích xuất đầy đủ
/// (char→byte, selection/invalid/out-of-range, UTF-8) ở commit kế.
pub(crate) fn chuyen_boi_canh(snapshot: &CadenceContextSnapshot, _text: &str) -> BoiCanhNhap {
    BoiCanhNhap {
        context_id: ContextId(snapshot.context_id),
        the_he_focus: snapshot.focus_generation,
        dang_co_focus: snapshot.has_focus != 0,
        van_ban_truoc_con_tro: None,
    }
}
