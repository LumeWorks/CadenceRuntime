// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Phiên nhập - state machine tối thiểu, zero-preedit, per-context.
//!
//! Mỗi ô nhập liệu có một [`PhienNhap`] riêng; không có state global. Runtime
//! nhận [`SuKienNhap`] trung lập (không biết keycode nền tảng), đưa vào Cadence
//! qua [`PhienCadence`](crate::cadence::PhienCadence), so sánh `da_hien_thi`
//! với snapshot mới qua [`KeHoachSua`](crate::sua::KeHoachSua), rồi gửi đúng
//! một [`HanhDong`](crate::HanhDong) tới host. State Cadence mới chỉ được chấp
//! nhận khi host xác nhận [`DaPhat`](crate::KetQuaHost::DaPhat).

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

/// Số sự kiện tối đa trong lịch sử replay. Vượt giới hạn → relinquish để chống
/// input độc hại tạo composition dài không thực tế khiến replay O(n) đắt.
///
/// Cadence nội bộ giới hạn 128 thao tác (`gioi_han_thao_tac`), nhưng `xoa_lui`
/// rút ngắn history Cadence mà vẫn thêm entry vào `lich_su` runtime. Giới hạn
/// này chặn `lich_su` tăng không hợp do xen kẽ them/xoa.
const GIOI_HAN_LICH_SU: usize = 256;

/// Kết quả xử lý một sự kiện nhập.
///
/// Bất biến tối quan trọng: **mỗi input event xuất hiện tối đa một lần trong
/// ứng dụng**. Ba biến thể below chỉ đạo adapter Phase 2 cách xử lý sự kiện
/// gốc:
///
/// * [`DaApDung`]: runtime đã gửi action và host đã phát ([`DaPhat`]). Adapter
///   **không** chuyển tiếp phím gốc — text đã được phát đúng một lần (không có
///   app ACK, nhưng runtime verify lại surrounding ở phím kế tiếp).
/// * [`ChuyenTiep`]: runtime không áp dụng text change nào (host [`KhongPhat`],
///   relinquish, hoặc Cadence không đổi). Adapter **nên** chuyển tiếp phím
///   gốc — sự kiện sẽ xuất hiện đúng một lần qua phím gốc.
/// * [`MatDongBo`]: host [`KhongChac`] (có thể đã phát một phần). Adapter
///   **không** chuyển tiếp phím gốc — sự kiện có thể đã xuất hiện (tối đa một
///   lần), chuyển tiếp sẽ tạo bản sao.
///
/// Một sự kiện tạo tối đa một lời gọi `host.thuc_thi`. Runtime không bao giờ
/// retry destructive action sau [`KhongPhat`](crate::KetQuaHost::KhongPhat),
/// và không replay mù phím đã xử lý sau
/// [`KhongChac`](crate::KetQuaHost::KhongChac).
///
/// [`DaPhat`]: crate::KetQuaHost::DaPhat
/// [`KhongPhat`]: crate::KetQuaHost::KhongPhat
/// [`KhongChac`]: crate::KetQuaHost::KhongChac
/// [`DaApDung`]: KetQuaXuLy::DaApDung
/// [`ChuyenTiep`]: KetQuaXuLy::ChuyenTiep
/// [`MatDongBo`]: KetQuaXuLy::MatDongBo
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KetQuaXuLy {
    /// Action đã gửi và host đã phát [`DaPhat`](crate::KetQuaHost::DaPhat);
    /// state đã tiến. Adapter không chuyển tiếp phím gốc.
    DaApDung,
    /// Runtime không áp dụng text change. Adapter nên chuyển tiếp phím gốc.
    ChuyenTiep,
    /// Host không chắc chắn; phiên đã mất đồng bộ an toàn. Adapter không
    /// chuyển tiếp phím gốc (host có thể đã áp dụng một phần).
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
    /// Tổng số operation Cadence (them_ky_tu/xoa_lui) kể cả replay. Chỉ dùng
    /// cho test đo chi phí; 0 trong production build.
    #[cfg(test)]
    so_op_cadence: usize,
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
            #[cfg(test)]
            so_op_cadence: 0,
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

    /// Đặt lại phiên: relinquish composition (đặt lại Cadence, xóa ownership
    /// suffix, xóa lịch sử, về `Rong`). Không xóa committed text đã trong ứng
    /// dụng. Dùng cho lifecycle reset/deactivate/focus-out của adapter.
    ///
    /// Khác với [`PhienNhap::xu_ly`] với [`SuKienNhap::DatLai`], phương thức này
    /// relinquish trực tiếp không cần `Host` — không kiểm tra focus/context
    /// (caller đã biết context cần reset).
    pub fn dat_lai(&mut self) {
        self.relinquish();
    }

    /// Xử lý một sự kiện nhập qua host, trả kết quả.
    ///
    /// Đây là entry point duy nhất. Một sự kiện tạo tối đa một lời gọi
    /// `host.thuc_thi` (bất biến zero-preedit: một sự kiện - một action logic).
    /// Xem [`KetQuaXuLy`] cho contract "tối đa một lần xuất hiện" và hướng dẫn
    /// adapter khi nào chuyển tiếp phím gốc.
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
        // Giới hạn lịch sử: nếu quá dài, relinquish để chống input độc hại tạo
        // composition dài không thực tế khiến replay O(n) đắt. Composition đã
        // commit trong host; runtime chỉ ngừng theo dõi suffix.
        if self.lich_su.len() >= GIOI_HAN_LICH_SU {
            self.relinquish();
        }
        let noi_dung_cu = self.da_hien_thi.clone();
        // Áp dụng vào Cadence (mutates). Nếu KhongDoi → forward.
        let ket_qua_cadence = match su_kien {
            SuKienXay::KyTu(c) => self.cadence.them_ky_tu(c),
            SuKienXay::XoaLui => self.cadence.xoa_lui(),
        };
        #[cfg(test)]
        {
            self.so_op_cadence += 1;
        }
        if matches!(ket_qua_cadence, KetQuaCadence::KhongDoi) {
            // Cadence không đổi (giới hạn, hoặc xóa khi rỗng) - forward.
            return KetQuaXuLy::ChuyenTiep;
        }
        let noi_dung_moi = self.cadence.ban_chup().noi_dung;
        // Verify-before-mutate: khi runtime đang sở hữu suffix (da_hien_thi
        // không rỗng), mọi action tiếp tục composition (cả Chen lẫn ThayThe)
        // đều phải verify surrounding. Chen khi đang sở hữu suffix vẫn chèn
        // vào vị trí cursor — nếu cursor lệch (surrounding None/mismatch), chèn
        // sai vị trí rồi đầu độc state, phím sau mới gây lỗi nặng. Khi
        // da_hien_thi rỗng (composition mới), phím đầu tiên là Chen thuần, an
        // toàn không cần surrounding.
        if !self.da_hien_thi.is_empty() && !self.khop_surrounding(boi_canh) {
            self.relinquish();
            return KetQuaXuLy::ChuyenTiep;
        }
        // Tính kế hoạch sửa từ da_hien_thi sang rendered mới.
        let ke = KeHoachSua::tinh(&noi_dung_cu, &noi_dung_moi);
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
            KetQuaHost::DaPhat => {
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
            KetQuaHost::KhongPhat => {
                // Không chấp nhận state mới. Quay lui Cadence về trước sự kiện.
                self.cadence = xay_lai_cadence(&self.lich_su);
                #[cfg(test)]
                {
                    self.so_op_cadence += self.lich_su.len();
                }
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
            KetQuaHost::DaPhat => {
                self.relinquish();
                KetQuaXuLy::DaApDung
            }
            KetQuaHost::KhongPhat => {
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
    /// Trả `true` khi host cung cấp surrounding (`Some`) và nó kết thúc bằng
    /// `da_hien_thi` - tức composition runtime theo dõi vẫn ở đúng vị trí trước
    /// con trỏ. Trả `false` khi surrounding `None` (không thể verify) hoặc khi
    /// surrounding không khớp (text đã thay đổi ngoài runtime).
    ///
    /// Phase 1 chỉ cho phép destructive replace (ThayThe) khi host cung cấp
    /// surrounding text đủ để verify. Khi `None`, runtime không được đoán text
    /// đã commit có còn ở đúng vị trí không.
    fn khop_surrounding(&self, boi_canh: &BoiCanhNhap) -> bool {
        match &boi_canh.van_ban_truoc_con_tro {
            Some(vb) => vb.ends_with(&self.da_hien_thi),
            None => false,
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

#[cfg(test)]
impl PhienNhap {
    /// Trả tổng số operation Cadence (them_ky_tu/xoa_lui) kể cả replay.
    /// Dùng cho test đo chi phí: hot path DaApDung tăng 1 mỗi phím, replay
    /// KhongApDung tăng thêm N (N = lich_su.len()).
    pub(crate) fn so_op_cadence(&self) -> usize {
        self.so_op_cadence
    }

    /// Trả độ dài lịch sử sự kiện đã chấp nhận. Dùng cho test giới hạn và
    /// verify history được cắt tại word boundary/reset.
    pub(crate) fn do_dai_lich_su(&self) -> usize {
        self.lich_su.len()
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

#[cfg(test)]
mod test_noi_bo {
    //! Test nội bộ: đo chi phí replay, giới hạn lịch sử, và history cắt tại
    //! word boundary. Truy cập field private qua #[cfg(test)] methods.

    use super::*;
    use crate::{BoiCanhNhap, ContextId, HanhDong, Host, KetQuaHost};

    /// Host đơn giản cho unit test: giữ text, cursor luôn cuối, surrounding
    /// luôn `Some`.
    struct HostDonGian {
        context_id: ContextId,
        van_ban: String,
        ket_qua: KetQuaHost,
    }

    impl HostDonGian {
        fn moi(context_id: ContextId) -> Self {
            Self {
                context_id,
                van_ban: String::new(),
                ket_qua: KetQuaHost::DaPhat,
            }
        }
    }

    impl Host for HostDonGian {
        fn boi_canh(&self) -> BoiCanhNhap {
            BoiCanhNhap {
                context_id: self.context_id,
                the_he_focus: 1,
                dang_co_focus: true,
                van_ban_truoc_con_tro: Some(self.van_ban.clone()),
            }
        }
        fn thuc_thi(&mut self, hanh_dong: &HanhDong) -> KetQuaHost {
            match hanh_dong {
                HanhDong::Chen(s) => self.van_ban.push_str(s),
                HanhDong::ThayThe(ke) => {
                    let xoa = ke.xoa_truoc.byte_utf8;
                    let len = self.van_ban.len();
                    let bat_dau = len.saturating_sub(xoa);
                    self.van_ban.replace_range(bat_dau..len, &ke.chen);
                }
                HanhDong::ChuyenTiep => {}
            }
            self.ket_qua
        }
    }

    #[test]
    fn hot_path_da_ap_dung_khong_replay() {
        // Chứng minh: DaPhat chỉ tốn 1 operation Cadence mỗi phím (không
        // replay). 50 phím → đúng 50 operation.
        let mut phien = PhienNhap::moi(ContextId(1));
        let mut host = HostDonGian::moi(ContextId(1));
        for _ in 0..50 {
            phien.xu_ly(&mut host, &SuKienNhap::KyTu('k'));
        }
        assert_eq!(
            phien.so_op_cadence(),
            50,
            "hot path phai la O(1) moi phim, khong replay"
        );
    }

    #[test]
    fn khong_ap_dung_replay_tang_chi_phi() {
        // Chứng minh: KhongPhat tốn thêm N operation replay (N = lich_su).
        // 50 DaPhat → 50 op. 1 KhongPhat → +1 (event) +50 (replay) = 101.
        let mut phien = PhienNhap::moi(ContextId(1));
        let mut host = HostDonGian::moi(ContextId(1));
        for _ in 0..50 {
            phien.xu_ly(&mut host, &SuKienNhap::KyTu('k'));
        }
        assert_eq!(phien.so_op_cadence(), 50);

        host.ket_qua = KetQuaHost::KhongPhat;
        phien.xu_ly(&mut host, &SuKienNhap::KyTu('k'));
        assert_eq!(
            phien.so_op_cadence(),
            101,
            "replay phai them N op (N=lich_su), tong = 50+1+50"
        );
    }

    #[test]
    fn ranh_gioi_tu_xoa_lich_su() {
        // RanhGioiTu (word boundary) → relinquish → lich_su cleared.
        let mut phien = PhienNhap::moi(ContextId(1));
        let mut host = HostDonGian::moi(ContextId(1));
        for _ in 0..10 {
            phien.xu_ly(&mut host, &SuKienNhap::KyTu('k'));
        }
        assert_eq!(phien.do_dai_lich_su(), 10);

        phien.xu_ly(&mut host, &SuKienNhap::RanhGioiTu(' '));
        assert_eq!(phien.do_dai_lich_su(), 0, "RanhGioiTu phai xoa lich_su");

        // Gõ tiếp: lich_su grows from 0.
        phien.xu_ly(&mut host, &SuKienNhap::KyTu('k'));
        assert_eq!(phien.do_dai_lich_su(), 1);
    }

    #[test]
    fn gioi_han_lich_su_relinquish_khi_vuot() {
        // lich_su đạt GIOI_HAN_LICH_SU → relinquish trước sự kiện kế tiếp.
        // Xen kẽ 'k' và XoaLui: mỗi cặp thêm 2 vào lich_su, Cadence history
        // không tăng (xoa undo them).
        let mut phien = PhienNhap::moi(ContextId(1));
        let mut host = HostDonGian::moi(ContextId(1));
        for _ in 0..128 {
            phien.xu_ly(&mut host, &SuKienNhap::KyTu('k'));
            phien.xu_ly(&mut host, &SuKienNhap::XoaLui);
        }
        assert_eq!(phien.do_dai_lich_su(), 256);

        // Sự kiện 257: runtime relinquish (vượt GIOI_HAN_LICH_SU).
        phien.xu_ly(&mut host, &SuKienNhap::KyTu('k'));
        assert_eq!(
            phien.do_dai_lich_su(),
            1,
            "sau relinquish, lich_su phai reset ve 1 (chi su kien moi)"
        );
    }
}
