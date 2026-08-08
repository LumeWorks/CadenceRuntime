// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Host abstraction tối thiểu: runtime không biết Fcitx, Wayland hay Windows,
//! chỉ biết ngữ cảnh nhập và ba kết quả thực thi logic.
//!
//! Phase 3C mở rộng contract cho client composition: runtime có thể yêu cầu host
//! cập nhật/xóa/kết thúc client preedit thay vì commit text trung gian vào
//! document. Xem [`HanhDong`] cho ba đường output (VerifiedReplace,
//! PlainComposition, Passthrough).

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
/// surrounding text (ví dụ game, terminal, Chrome, VS Code). Khi `None`,
/// runtime không thể verify text đã commit có còn ở đúng vị trí không, nên
/// VerifiedReplace (delete + commit) không an toàn. Phase 3C thêm
/// `co_preedit` và `co_sensitive` để runtime chọn đường PlainComposition
/// (client preedit) khi VerifiedReplace không khả dụng nhưng client hỗ trợ
/// preedit. Xem [`PhienNhap`](crate::PhienNhap) cho chi tiết route selection.
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
    /// Nếu `None`, runtime không verify được vị trí cursor nên VerifiedReplace
    /// (destructive replace) không an toàn — runtime chọn PlainComposition nếu
    /// `co_preedit`, hoặc Passthrough.
    ///
    /// Lưu ý: `None` không nhất thiết nghĩa là app không hỗ trợ surrounding —
    /// nhiều app (Qt text widget) chỉ báo `sur_valid=1` từ phím thứ 2 (sau khi
    /// có text để report). Phím đầu `sur_valid=0` nhưng `co_surrounding=true`
    /// → VerifiedReplace vẫn khả dụng (phím đầu là Chen thuần, phím 2 verify).
    pub van_ban_truoc_con_tro: Option<String>,
    /// `true` nếu client hỗ trợ surrounding text (`CapabilityFlag::SurroundingText`
    /// trong Fcitx). Khác `van_ban_truoc_con_tro.is_some()` — capability báo app
    /// *hỗ trợ* surrounding, nhưng surrounding có thể `None` tạm thời (phím đầu,
    /// hoặc app chưa report). Route selection dùng capability này: nếu
    /// `co_surrounding=true`, VerifiedReplace khả dụng ngay cả khi
    /// `van_ban_truoc_con_tro=None` ở phím đầu.
    pub co_surrounding: bool,
    /// `true` nếu client hỗ trợ client preedit/composition
    /// (`CapabilityFlag::Preedit` trong Fcitx). Khi `true` và surrounding không
    /// hợp lệ, runtime có thể dùng PlainComposition (cập nhật preedit thay vì
    /// commit text trung gian vào document).
    pub co_preedit: bool,
    /// `true` nếu context nhạy cảm (password, sensitive). Khi `true`, runtime
    /// ưu tiên Passthrough — không gõ tiếng Việt, không preedit, không
    /// diagnostic text, để tránh đầu độc trường bảo mật.
    pub co_sensitive: bool,
}

/// Hành động logic runtime yêu cầu host thực thi.
///
/// Phase 3C có ba đường output (xem [`PhienNhap`](crate::PhienNhap)):
///
/// * **VerifiedReplace** (`Chen`/`ThayThe`): commit text trung gian vào document
///   mỗi phím, verify surrounding. Dùng khi surrounding hợp lệ.
/// * **PlainComposition** (`CapNhatSoanThao`/`KetThucSoanThao`/`XoaSoanThao`):
///   cập nhật client preedit (không decoration) thay vì commit. Text chỉ commit
///   tại boundary. Dùng khi VerifiedReplace không khả dụng nhưng client hỗ trợ
///   preedit. `CapNhatSoanThao` gửi text với **NO formatting flags**
///   (`TextFormatFlag::NoFlag` trong Fcitx) — zero visible decoration.
/// * **Passthrough** (`ChuyenTiep`): chuyển tiếp sự kiện gốc cho ứng dụng, không
///   thay đổi văn bản.
///
/// `ChuyenTiep` báo host chuyển tiếp sự kiện gốc (phím nguyên thủy) cho ứng dụng.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HanhDong {
    /// Chèn committed text tại con trỏ (VerifiedReplace).
    Chen(String),
    /// Thay thế committed suffix: xóa `xoa_truoc` trước con trỏ rồi chèn `chen`
    /// (VerifiedReplace).
    ThayThe(KeHoachSua),
    /// Cập nhật client preedit thành `text` (PlainComposition). Text chưa commit
    /// vào document; nằm trong client composition. Phải gửi với NO formatting
    /// flags (zero visible decoration). Con trỏ composition đặt cuối text.
    CapNhatSoanThao(String),
    /// Kết thúc composition: commit `text` vào document rồi clear client
    /// preedit (PlainComposition). Dùng tại boundary (space) hoặc relinquish.
    KetThucSoanThao(String),
    /// Xóa client preedit (PlainComposition). Dùng khi composition trở thành
    /// rỗng (backspace đến empty).
    XoaSoanThao,
    /// Chuyển tiếp sự kiện gốc cho ứng dụng (không thay đổi văn bản, Passthrough).
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
