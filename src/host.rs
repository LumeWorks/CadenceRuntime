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
/// surrounding text (ví dụ game, terminal). Khi `None`, runtime không thể
/// verify text đã commit có còn ở đúng vị trí không, nên chỉ cho phép `Chen`
/// (insert thuần) và chặn `ThayThe` (destructive replace). Xem
/// [`PhienNhap`](crate::PhienNhap) cho chi tiết contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoiCanhNhap {
    /// Context đang giữ focus.
    pub context_id: ContextId,
    /// Thế hệ focus hiện tại. Tăng khi context mất rồi lại có focus.
    pub the_he_focus: u64,
    /// `true` nếu context đang có focus nhập.
    pub dang_co_focus: bool,
    /// Văn bản trước con trỏ (surrounding text), nếu host cung cấp.
    ///
    /// Phase 1 verify cursor gián tiếp qua trường này: nếu `Some` và kết thúc
    /// bằng `da_hien_thi` runtime, con trỏ được coi là ngay sau composition.
    /// Nếu `None`, runtime không verify được vị trí cursor nên chỉ cho phép
    /// insert thuần, không delete.
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
/// Tên biến thể trung thực về việc **phát** lệnh vào nền tảng, không tuyên bố
/// ứng dụng đã ACK: Fcitx (và đa số host IM) không trả kết quả từ ứng dụng cho
/// `commitString`/`deleteSurroundingText`. Runtime không được coi timeout hoặc
/// lỗi không rõ ràng là [`DaPhat`](KetQuaHost::DaPhat). Mỗi biến thể có ngữ
/// nghĩa chặt:
///
/// * [`DaPhat`](KetQuaHost::DaPhat): host đã phát toàn bộ lệnh của logical
///   action theo đúng thứ tự vào nền tảng. Runtime được giữ state mới **có
///   điều kiện**: trước mọi action tiếp theo khi `da_hien_thi` không rỗng,
///   surrounding phải được verify lại. Không mô tả là app đã ACK.
/// * [`KhongPhat`](KetQuaHost::KhongPhat): host chắc chắn **chưa** phát lệnh
///   text mutation nào. Runtime quay lui state Cadence, trả
///   [`ChuyenTiep`](crate::KetQuaXuLy::ChuyenTiep).
/// * [`KhongChac`](KetQuaHost::KhongChac): có khả năng chỉ một phần action đã
///   được phát. Runtime reset an toàn, trả
///   [`MatDongBo`](crate::KetQuaXuLy::MatDongBo). Adapter không chuyển tiếp
///   phím gốc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KetQuaHost {
    /// Host đã phát toàn bộ logical action theo đúng thứ tự vào nền tảng.
    /// Không có nghĩa ứng dụng đã ACK.
    DaPhat,
    /// Host chắc chắn chưa phát bất kỳ phần nào của action.
    KhongPhat,
    /// Có khả năng chỉ một phần action đã được phát.
    KhongChac,
}

/// Seam giữa runtime và môi trường nhập liệu nền tảng.
///
/// Phase 2 adapter Fcitx5 triển khai trait này; Phase 1 chỉ dùng host mô phỏng
/// trong test (`tests/common/mod.rs`) để kiểm chứng runtime mà không cần Fcitx.
pub trait Host {
    /// Trả ngữ cảnh nhập hiện tại.
    fn boi_canh(&self) -> BoiCanhNhap;
    /// Thực thi một hành động logic, trả kết quả xác nhận.
    fn thuc_thi(&mut self, hanh_dong: &HanhDong) -> KetQuaHost;
}
