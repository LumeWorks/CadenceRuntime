// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Phiên nhập - state machine tối thiểu, zero-preedit, per-context.
//!
//! Mỗi ô nhập liệu có một [`PhienNhap`] riêng; không có state global. Runtime
//! nhận [`SuKienNhap`] trung lập (không biết keycode nền tảng), đưa vào Cadence
//! qua [`PhienCadence`](crate::cadence::PhienCadence), so sánh `da_hien_thi`
//! với snapshot mới qua [`KeHoachSua`](crate::sua::KeHoachSua), rồi gửi đúng
//! một [`HanhDong`](crate::HanhDong) tới host. State Cadence mới chỉ được chấp
//! nhận khi host xác nhận [`DaApDung`](crate::KetQuaHost::DaApDung).

use crate::cadence::{KetQuaCadence, PhienCadence};
use crate::host::{BoiCanhNhap, ContextId, HanhDong, Host, KetQuaHost};
use crate::sua::KeHoachSua;

/// Sự kiện nhập liệu trung lập nền tảng.
///
/// Phase 2 adapter Fcitx chịu trách nhiệm chuyển key event nền tảng (keycode,
/// modifier mask) thành các biến thể này. Runtime không biết physical keycode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuKienNhap {
    /// Một ký tự được nhập (Telex biến đổi khi Cadence áp dụng).
    KyTu(char),
    /// Xóa lùi (backspace) trong composition.
    XoaLui,
    /// Ký tự ranh giới từ (space, punctuation) - kết thúc composition hiện tại
    /// và chèn ký tự này như văn bản thường.
    RanhGioiTu(char),
    /// Con trỏ di chuyển ngoài runtime - relinquish composition.
    DiChuyenConTro,
    /// Đặt lại composition (ví dụ Escape) - relinquish, không xóa text đã commit.
    DatLai,
}

/// Trạng thái phiên runtime, tối thiểu ba trạng thái.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrangThaiPhien {
    /// Chưa có composition runtime đang sở hữu.
    Rong,
    /// CadenceRuntime đang theo dõi committed suffix do nó tạo.
    DangGo,
    /// Runtime không còn chắc ứng dụng hiển thị đúng nội dung dự kiến.
    MatDongBo,
}

/// Sự kiện xây composition đã được host chấp nhận - để replay khi quay lui.
///
/// Chỉ `KyTu` và `XoaLui` xây composition; các sự kiện relinquish xóa lịch sử,
/// nên không bao giờ xuất hiện trong `lich_su`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SuKienNhapDaChapNhan {
    /// Ký tự đã được host chấp nhận.
    KyTu(char),
    /// Backspace đã được host chấp nhận.
    XoaLui,
}

/// Kết quả xử lý một sự kiện nhập.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KetQuaXuLy {
    /// Action đã gửi và host xác nhận [`DaApDung`](crate::KetQuaHost::DaApDung);
    /// state đã tiến.
    DaApDung,
    /// Runtime chuyển tiếp sự kiện gốc cho ứng dụng (host từ chối, hoặc sự kiện
    /// relinquish, hoặc Cadence không đổi). Adapter nên chuyển tiếp phím gốc.
    ChuyenTiep,
    /// Host không chắc chắn; phiên đã mất đồng bộ an toàn.
    MatDongBo,
}

/// Phiên nhập của một ô nhập liệu.
///
/// Một [`PhienNhap`] per context. Hai phiên không chia sẻ state: mỗi phiên giữ
/// `PhienCadence`, `da_hien_thi` và lịch sử riêng.
pub struct PhienNhap {
    /// Context mà phiên này sở hữu.
    context_id: ContextId,
    /// Boundary Cadence.
    cadence: PhienCadence,
    /// Văn bản runtime tin ứng dụng đang hiển thị do composition này tạo.
    da_hien_thi: String,
    /// Thế hệ focus đã adopt; `None` đến khi nhận focus đầu tiên.
    the_he_focus: Option<u64>,
    /// Trạng thái phiên.
    trang_thai: TrangThaiPhien,
    /// Lịch sử sự kiện đã chấp nhận, để replay khi host từ chối.
    lich_su: Vec<SuKienNhapDaChapNhan>,
}

impl PhienNhap {
    /// Tạo phiên rỗng cho một context.
    #[must_use]
    pub fn moi(context_id: ContextId) -> Self {
        Self {
            context_id,
            cadence: PhienCadence::moi(),
            da_hien_thi: String::new(),
            the_he_focus: None,
            trang_thai: TrangThaiPhien::Rong,
            lich_su: Vec::new(),
        }
    }

    /// Trả `true` nếu runtime không đang theo dõi composition nào.
    #[must_use]
    pub fn dang_rong(&self) -> bool {
        matches!(self.trang_thai, TrangThaiPhien::Rong) || self.da_hien_thi.is_empty()
    }

    /// Trả `true` nếu phiên đang mất đồng bộ (không gửi delete dựa state cũ).
    #[must_use]
    pub fn dang_mat_dong_bo(&self) -> bool {
        matches!(self.trang_thai, TrangThaiPhien::MatDongBo)
    }

    /// Trả nội dung runtime tin ứng dụng đang hiển thị do composition tạo.
    #[must_use]
    pub fn da_hien_thi(&self) -> &str {
        &self.da_hien_thi
    }

    /// Xử lý một sự kiện nhập qua host, trả kết quả.
    ///
    /// Đây là entry point duy nhất. Một sự kiện tạo tối đa một lời gọi
    /// `host.thuc_thi` (bất biến zero-preedit: một sự kiện - một action logic).
    pub fn xu_ly<H: Host>(&mut self, host: &mut H, su_kien: &SuKienNhap) -> KetQuaXuLy {
        let boi_canh = host.boi_canh();
        // 1. Đồng bộ focus generation. Lần đầu adopt; nếu đổi → relinquish + forward.
        match self.the_he_focus {
            None => self.the_he_focus = Some(boi_canh.the_he_focus),
            Some(g) if g == boi_canh.the_he_focus => {}
            Some(_) => {
                // Focus regenerated: không xóa dựa suffix cũ.
                self.relinquish();
                self.the_he_focus = Some(boi_canh.the_he_focus);
                return KetQuaXuLy::ChuyenTiep;
            }
        }
        // 2. Mất focus → relinquish + forward.
        if !boi_canh.dang_co_focus {
            self.relinquish();
            return KetQuaXuLy::ChuyenTiep;
        }
        // 3. Context lệch → relinquish + forward.
        if boi_canh.context_id != self.context_id {
            self.relinquish();
            return KetQuaXuLy::ChuyenTiep;
        }
        // 4. MatDongBo: phục hồi an toàn khi da_hien_thi rỗng (không suffix cũ).
        if matches!(self.trang_thai, TrangThaiPhien::MatDongBo) {
            if self.da_hien_thi.is_empty() {
                self.trang_thai = TrangThaiPhien::Rong;
            } else {
                // Vẫn có suffix cũ không khớp thực tế - không delete.
                self.relinquish();
                return KetQuaXuLy::ChuyenTiep;
            }
        }
        // 5. Phân nhánh theo loại sự kiện.
        match su_kien {
            SuKienNhap::KyTu(c) => self.xu_ly_xay(host, &boi_canh, SuKienXay::KyTu(*c)),
            SuKienNhap::XoaLui => self.xu_ly_xay(host, &boi_canh, SuKienXay::XoaLui),
            SuKienNhap::RanhGioiTu(c) => self.xu_ly_ranh_gioi(host, *c),
            SuKienNhap::DiChuyenConTro => {
                self.relinquish();
                KetQuaXuLy::ChuyenTiep
            }
            SuKienNhap::DatLai => {
                self.relinquish();
                KetQuaXuLy::ChuyenTiep
            }
        }
    }

    /// Xử lý sự kiện xây composition (KyTu hoặc XoaLui).
    fn xu_ly_xay<H: Host>(
        &mut self,
        host: &mut H,
        boi_canh: &BoiCanhNhap,
        su_kien: SuKienXay,
    ) -> KetQuaXuLy {
        let noi_dung_cu = self.da_hien_thi.clone();
        // Áp dụng vào Cadence (mutates). Nếu KhongDoi → forward.
        let ket_qua_cadence = match su_kien {
            SuKienXay::KyTu(c) => self.cadence.them_ky_tu(c),
            SuKienXay::XoaLui => self.cadence.xoa_lui(),
        };
        if matches!(ket_qua_cadence, KetQuaCadence::KhongDoi) {
            // Cadence không đổi (giới hạn, hoặc xóa khi rỗng) - forward.
            return KetQuaXuLy::ChuyenTiep;
        }
        let noi_dung_moi = self.cadence.ban_chup().noi_dung;
        // Tính kế hoạch sửa từ da_hien_thi sang rendered mới.
        let ke = KeHoachSua::tinh(&noi_dung_cu, &noi_dung_moi);
        // Verify-before-mutate: nếu cần xóa, surrounding phải khớp da_hien_thi.
        if !ke.xoa_truoc.la_rong() && !self.khop_surrounding(boi_canh) {
            // Surrounding lệch - không delete. Quay lui Cadence, forward.
            self.cadence = xay_lai_cadence(&self.lich_su);
            return KetQuaXuLy::ChuyenTiep;
        }
        // Chọn action logic duy nhất (zero-preedit: chỉ Chen/ThayThe/ChuyenTiep).
        let hanh_dong = if ke.la_rong() {
            HanhDong::ChuyenTiep
        } else if ke.la_chen() {
            HanhDong::Chen(ke.chen.clone())
        } else {
            HanhDong::ThayThe(ke)
        };
        // Gửi tới host - một sự kiện tối đa một lời gọi thuc_thi.
        let ket_qua_host = host.thuc_thi(&hanh_dong);
        match ket_qua_host {
            KetQuaHost::DaApDung => {
                self.da_hien_thi = noi_dung_moi;
                self.lich_su.push(match su_kien {
                    SuKienXay::KyTu(c) => SuKienNhapDaChapNhan::KyTu(c),
                    SuKienXay::XoaLui => SuKienNhapDaChapNhan::XoaLui,
                });
                self.trang_thai = if self.cadence.dang_trong() {
                    TrangThaiPhien::Rong
                } else {
                    TrangThaiPhien::DangGo
                };
                KetQuaXuLy::DaApDung
            }
            KetQuaHost::KhongApDung => {
                // Không chấp nhận state mới. Quay lui Cadence về trước sự kiện.
                self.cadence = xay_lai_cadence(&self.lich_su);
                // da_hien_thi và lich_su giữ nguyên. Sự kiện không bị nuốt.
                KetQuaXuLy::ChuyenTiep
            }
            KetQuaHost::KhongChac => {
                // Mất đồng bộ an toàn: reset, không delete dựa state cũ.
                self.cadence.dat_lai();
                self.da_hien_thi.clear();
                self.lich_su.clear();
                self.trang_thai = TrangThaiPhien::MatDongBo;
                KetQuaXuLy::MatDongBo
            }
        }
    }

    /// Xử lý ranh giới từ: relinquish composition, chèn ký tự ranh giới.
    ///
    /// Composition hiện tại (nếu có) đã hiển thị qua các `ThayThe` trước đó; app
    /// đã sở hữu nó. Runtime chỉ cần chèn ký tự ranh giới rồi relinquish.
    fn xu_ly_ranh_gioi<H: Host>(&mut self, host: &mut H, ky_tu: char) -> KetQuaXuLy {
        let hanh_dong = HanhDong::Chen(ky_tu.to_string());
        let ket_qua_host = host.thuc_thi(&hanh_dong);
        match ket_qua_host {
            KetQuaHost::DaApDung => {
                self.relinquish();
                KetQuaXuLy::DaApDung
            }
            KetQuaHost::KhongApDung => {
                // Ký tự ranh giới không được chèn - forward.
                KetQuaXuLy::ChuyenTiep
            }
            KetQuaHost::KhongChac => {
                self.relinquish();
                self.trang_thai = TrangThaiPhien::MatDongBo;
                KetQuaXuLy::MatDongBo
            }
        }
    }

    /// Kiểm tra surrounding text trước con trỏ kết thúc bằng `da_hien_thi`.
    ///
    /// Khi host không cung cấp surrounding (`None`), không thể verify; Phase 1
    /// cho phép (documented limitation; Phase 2 tinh chỉnh theo capability host).
    fn khop_surrounding(&self, boi_canh: &BoiCanhNhap) -> bool {
        match &boi_canh.van_ban_truoc_con_tro {
            Some(vb) => vb.ends_with(&self.da_hien_thi),
            None => true,
        }
    }

    /// Relinquish composition: đặt lại Cadence, xóa ownership, về Rong.
    fn relinquish(&mut self) {
        self.cadence.dat_lai();
        self.da_hien_thi.clear();
        self.lich_su.clear();
        self.trang_thai = TrangThaiPhien::Rong;
    }
}

/// Sự kiện xây composition (dạng nội bộ, đã qua kiểm tra focus/context).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuKienXay {
    /// Ký tự.
    KyTu(char),
    /// Backspace.
    XoaLui,
}

/// Dựng lại `PhienCadence` từ lịch sử sự kiện đã chấp nhận. Dùng khi host từ
/// chối action và runtime cần quay lui state Cadence về điểm trước sự kiện.
fn xay_lai_cadence(lich_su: &[SuKienNhapDaChapNhan]) -> PhienCadence {
    let mut pc = PhienCadence::moi();
    for su_kien in lich_su {
        match su_kien {
            SuKienNhapDaChapNhan::KyTu(c) => {
                pc.them_ky_tu(*c);
            }
            SuKienNhapDaChapNhan::XoaLui => {
                pc.xoa_lui();
            }
        }
    }
    pc
}
