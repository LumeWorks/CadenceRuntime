// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Host abstraction tối thiểu: runtime không biết Fcitx, Wayland hay Windows,
//! chỉ biết ngữ cảnh nhập và ba kết quả thực thi logic.

use crate::sua::KeHoachSua;

/// Định danh một ô nhập liệu (input context).
///
/// Mỗi context có một [`PhienNhap`](crate::PhienNhap) riêng; hai context không
/// chia sẻ state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContextId(pub u64);

/// Ảnh chụp ngữ cảnh nhập tại thời điểm runtime cần quyết định.
///
/// `van_ban_truoc_con_tro` là `Option`: nhiều host thật không cung cấp
/// surrounding text (ví dụ game, terminal). Runtime không giả định nó luôn tồn
/// tại; khi `None`, verify-before-mutate không thể kiểm tra suffix và đi đường
/// an toàn (xem [`PhienNhap`](crate::PhienNhap)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoiCanhNhap {
    /// Context đang giữ focus.
    pub context_id: ContextId,
    /// Thế hệ focus hiện tại. Tăng khi context mất rồi lại có focus.
    pub the_he_focus: u64,
    /// `true` nếu context đang có focus nhập.
    pub dang_co_focus: bool,
    /// Văn bản trước con trỏ (surrounding text), nếu host cung cấp.
    pub van_ban_truoc_con_tro: Option<String>,
}

/// Hành động logic runtime yêu cầu host thực thi.
///
/// Cố tình không có nhánh preedit: zero-preedit là bất biến kiến trúc. Mọi chữ
/// user nhìn thấy đều đi qua `Chen` hoặc `ThayThe`. `ChuyenTiep` báo host chuyển
/// tiếp sự kiện gốc (phím nguyên thủy) cho ứng dụng, không chèn/replace gì.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HanhDong {
    /// Chèn committed text tại con trỏ.
    Chen(String),
    /// Thay thế committed suffix: xóa `xoa_truoc` trước con trỏ rồi chèn `chen`.
    ThayThe(KeHoachSua),
    /// Chuyển tiếp sự kiện gốc cho ứng dụng (không thay đổi văn bản).
    ChuyenTiep,
}

/// Kết quả host thực thi một [`HanhDong`].
///
/// Runtime không được coi timeout hoặc lỗi không rõ ràng là [`DaApDung`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KetQuaHost {
    /// Host xác nhận action đã được thực thi đúng contract.
    DaApDung,
    /// Action chắc chắn chưa thay đổi văn bản (ví dụ bị từ chối).
    KhongApDung,
    /// Không thể biết ứng dụng đã nhận một phần hay toàn bộ action.
    KhongChac,
}

/// Seam giữa runtime và môi trường nhập liệu nền tảng.
///
/// Phase 2 adapter Fcitx5 triển khai trait này; Phase 1 chỉ dùng [`HostMoPhong`]
/// (trong test) để kiểm chứng runtime mà không cần Fcitx.
///
/// [`HostMoPhong`]: ../../tests/index.html
pub trait Host {
    /// Trả ngữ cảnh nhập hiện tại.
    fn boi_canh(&self) -> BoiCanhNhap;
    /// Thực thi một hành động logic, trả kết quả xác nhận.
    fn thuc_thi(&mut self, hanh_dong: &HanhDong) -> KetQuaHost;
}
