// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Phiên nhập - state machine tối thiểu, per-context, ba đường output.
//!
//! Mỗi ô nhập liệu có một [`PhienNhap`] riêng; không có state global. Runtime
//! nhận [`SuKienNhap`] trung lập (không biết keycode nền tảng), đưa vào Cadence
//! qua [`PhienCadence`](crate::cadence::PhienCadence), so sánh `da_hien_thi`
//! với snapshot mới qua [`KeHoachSua`](crate::sua::KeHoachSua), rồi gửi đúng
//! một [`HanhDong`](crate::HanhDong) tới host. State Cadence mới chỉ được chấp
//! nhận khi host xác nhận [`DaPhat`](crate::KetQuaHost::DaPhat).
//!
//! Phase 3C có ba đường output (xem [`DuongXuat`]):
//! * **VerifiedReplace** — commit text trung gian vào document mỗi phím, verify
//!   surrounding. Dùng khi surrounding hợp lệ.
//! * **PlainComposition** — cập nhật client preedit (NO decoration) thay vì
//!   commit. Text chỉ commit tại boundary. Dùng khi surrounding không hợp lệ
//!   nhưng client hỗ trợ preedit.
//! * **Passthrough** — forward phím gốc (sensitive context, hoặc không route
//!   khả dụng).

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
    /// CanType đang theo dõi committed suffix do nó tạo.
    DangGo,
    /// Runtime không còn chắc ứng dụng hiển thị đúng nội dung dự kiến.
    MatDongBo,
}

/// Đường output của composition hiện tại (route lock).
///
/// Một composition giữ đường từ khi bắt đầu đến khi boundary/reset. Route
/// selection chỉ xảy ra khi composition mới (`duong == None`).
///
/// * [`DuongXuat::VerifiedReplace`] — commit text trung gian vào document mỗi
///   phím, verify surrounding. Dùng khi surrounding hợp lệ.
/// * [`DuongXuat::PlainComposition`] — cập nhật client preedit (NO decoration)
///   thay vì commit. Text chỉ commit tại boundary. Dùng khi surrounding không
///   hợp lệ nhưng client hỗ trợ preedit.
///
/// Passthrough không phải route composition — runtime không sở hữu text, chỉ
/// forward phím gốc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DuongXuat {
    /// Commit text trung gian vào document mỗi phím (Route 1, VerifiedReplace).
    VerifiedReplace,
    /// Cập nhật client preedit, commit tại boundary (Route 2, PlainComposition).
    PlainComposition,
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
    ///
    /// Với VerifiedReplace: text đã commit vào document. Với PlainComposition:
    /// text nằm trong client preedit (chưa commit).
    da_hien_thi: String,
    /// Thế hệ focus đã adopt; `None` đến khi nhận focus đầu tiên.
    the_he_focus: Option<u64>,
    /// Trạng thái phiên.
    trang_thai: TrangThaiPhien,
    /// Đường output hiện tại (route lock). `None` khi composition rỗng.
    duong: Option<DuongXuat>,
    /// Lịch sử sự kiện đã chấp nhận, để replay khi host từ chối.
    lich_su: Vec<SuKienNhapDaChapNhan>,
    /// Tổng số operations Cadence (them_ky_tu/xoa_lui) kể cả replay. Chỉ dùng
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
            duong: None,
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

    /// Trả đường output hiện tại cho diagnostic. `"native"` = VerifiedReplace,
    /// `"plain_composition"` = PlainComposition, `"passthrough"` = không route.
    #[cfg(feature = "diag")]
    #[must_use]
    pub(crate) fn duong_hien_tai(&self) -> &'static str {
        match self.duong {
            None => "passthrough",
            Some(DuongXuat::VerifiedReplace) => "native",
            Some(DuongXuat::PlainComposition) => "plain_composition",
        }
    }

    /// Đặt lại phiên: relinquish composition (đặt lại Cadence, xóa ownership
    /// suffix, xóa lịch sử, về `Rong`) và xóa focus adoption. Không xóa
    /// committed text đã trong ứng dụng. Dùng cho lifecycle reset/deactivate/
    /// focus-out của adapter.
    ///
    /// Khác với [`PhienNhap::xu_ly`] với [`SuKienNhap::DatLai`], phương thức này
    /// relinquish trực tiếp không cần `Host` — không kiểm tra focus/context
    /// (caller đã biết context cần reset).
    ///
    /// Xóa `the_he_focus` về `None` có chủ đích: lifecycle event (C++
    /// activate/deactivate/reset/focus-out) đã xử lý đúng ranh giới; phím kế tiếp
    /// adopt generation mới từ host và **compose tươi**, không bị forward do
    /// mismatch (tránh "ăn" phím đầu sau reset/focus-out — vd sau reset, "as"
    /// phải ra "á" chứ không phải "a" raw rồi "s"). Điều này khác với đường
    /// [`SuKienNhap::DatLai`] qua [`xu_ly`](Self::xu_ly), nơi gen mismatch mà
    /// không có lifecycle reset thì forward bảo thủ.
    pub fn dat_lai(&mut self) {
        self.relinquish();
        self.the_he_focus = None;
    }

    /// Xử lý một sự kiện nhập qua host, trả kết quả.
    ///
    /// Đây là entry point duy nhất. Một sự kiện tạo tối đa một lời gọi
    /// `host.thuc_thi` (bất biến: một sự kiện - một action logic). Xem
    /// [`KetQuaXuLy`] cho contract "tối đa một lần xuất hiện" và hướng dẫn
    /// adapter khi nào chuyển tiếp phím gốc.
    pub fn xu_ly<H: Host>(&mut self, host: &mut H, su_kien: &SuKienNhap) -> KetQuaXuLy {
        let boi_canh = host.boi_canh();
        // 1. Đồng bộ focus generation. Lần đầu adopt; nếu đổi → relinquish + forward.
        match self.the_he_focus {
            None => self.the_he_focus = Some(boi_canh.the_he_focus),
            Some(g) if g == boi_canh.the_he_focus => {}
            Some(_) => {
                // Focus regenerated: không xóa dựa suffix cũ.
                self.relinquish_voi_host(host);
                self.the_he_focus = Some(boi_canh.the_he_focus);
                return KetQuaXuLy::ChuyenTiep;
            }
        }
        // 2. Mất focus → relinquish + forward.
        if !boi_canh.dang_co_focus {
            self.relinquish_voi_host(host);
            return KetQuaXuLy::ChuyenTiep;
        }
        // 3. Context lệch → relinquish + forward.
        if boi_canh.context_id != self.context_id {
            self.relinquish_voi_host(host);
            return KetQuaXuLy::ChuyenTiep;
        }
        // 4. MatDongBo: phục hồi an toàn khi da_hien_thi rỗng (không suffix cũ).
        if matches!(self.trang_thai, TrangThaiPhien::MatDongBo) {
            if self.da_hien_thi.is_empty() {
                self.trang_thai = TrangThaiPhien::Rong;
            } else {
                // Vẫn có suffix cũ không khớp thực tế - không delete.
                self.relinquish_voi_host(host);
                return KetQuaXuLy::ChuyenTiep;
            }
        }
        // 5. Phân nhánh theo loại sự kiện.
        match su_kien {
            SuKienNhap::KyTu(c) => self.xu_ly_xay(host, &boi_canh, SuKienXay::KyTu(*c)),
            SuKienNhap::XoaLui => self.xu_ly_xay(host, &boi_canh, SuKienXay::XoaLui),
            SuKienNhap::RanhGioiTu(c) => self.xu_ly_ranh_gioi(host, *c),
            SuKienNhap::DiChuyenConTro | SuKienNhap::DatLai => self.relinquish_voi_host(host),
        }
    }

    /// Xử lý sự kiện xây composition (KyTu hoặc XoaLui).
    ///
    /// Route selection xảy ra khi composition mới (`duong == None`). Sau khi
    /// chọn, route bị khóa đến boundary/reset. VerifiedReplace verify
    /// surrounding; PlainComposition cập nhật client preedit (no decoration).
    fn xu_ly_xay<H: Host>(
        &mut self,
        host: &mut H,
        boi_canh: &BoiCanhNhap,
        su_kien: SuKienXay,
    ) -> KetQuaXuLy {
        // Giới hạn lịch sử: nếu quá dài, relinquish để chống input độc hại tạo
        // composition dài không thực tế khiến replay O(n) đắt. Composition đã
        // commit/preedit trong host; runtime chỉ ngừng theo dõi.
        //
        // Dùng relinquish() nội bộ (không gọi host.thuc_thi) — nếu dùng
        // relinquish_voi_host() sẽ gọi thuc_thi cho XoaSoanThao, rồi phím này
        // tiếp tục gọi thuc_thi nữa → hai thuc_thi cho một sự kiện (vi phạm
        // bất biến). Preedit stale trên client là edge case (256+ events);
        // phím kế CapNhatSoanThao sẽ overwrite, hoặc lifecycle clear.
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
        // Route selection khi composition mới (duong == None).
        if self.duong.is_none() {
            self.duong = self.chon_duong(boi_canh);
        }
        // Chọn action logic theo route.
        let hanh_dong = match self.duong {
            None => {
                // Passthrough: không route khả dụng (sensitive, hoặc không
                // surrounding + không preedit). Rollback Cadence, forward raw key.
                self.relinquish();
                return KetQuaXuLy::ChuyenTiep;
            }
            Some(DuongXuat::VerifiedReplace) => {
                // Verify-before-mutate: khi runtime đang sở hữu suffix
                // (da_hien_thi không rỗng), mọi action tiếp tục composition đều
                // phải verify surrounding. Khi da_hien_thi rỗng (composition mới),
                // phím đầu tiên an toàn không cần surrounding.
                if !self.da_hien_thi.is_empty() && !self.khop_surrounding(boi_canh) {
                    self.relinquish();
                    return KetQuaXuLy::ChuyenTiep;
                }
                let ke = KeHoachSua::tinh(&noi_dung_cu, &noi_dung_moi);
                if ke.la_rong() {
                    HanhDong::ChuyenTiep
                } else if ke.la_chen() {
                    HanhDong::Chen(ke.chen.clone())
                } else {
                    HanhDong::ThayThe(ke)
                }
            }
            Some(DuongXuat::PlainComposition) => {
                // PlainComposition: cập nhật client preedit (NO decoration).
                // Không verify surrounding — text nằm trong client composition,
                // chưa vào document. Không cần KeHoachSua (không diff document).
                if noi_dung_moi.is_empty() {
                    HanhDong::XoaSoanThao
                } else {
                    HanhDong::CapNhatSoanThao(noi_dung_moi.clone())
                }
            }
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

    /// Xử lý ranh giới từ: kết thúc composition, chèn ký tự ranh giới.
    ///
    /// VerifiedReplace: composition đã trong document, chỉ chèn boundary char.
    /// PlainComposition: commit composition + boundary char, clear preedit.
    /// None (composition rỗng): chèn boundary char (Chen, không Passthrough —
    /// boundary char như space/punctuation luôn cần vào document).
    fn xu_ly_ranh_gioi<H: Host>(&mut self, host: &mut H, ky_tu: char) -> KetQuaXuLy {
        let hanh_dong = match self.duong {
            Some(DuongXuat::PlainComposition) if !self.da_hien_thi.is_empty() => {
                // Commit composition text + boundary char, clear preedit.
                let mut text = self.da_hien_thi.clone();
                text.push(ky_tu);
                HanhDong::KetThucSoanThao(text)
            }
            // VerifiedReplace (composition đã commit), PlainComposition rỗng,
            // hoặc None (composition rỗng, không route): chỉ chèn boundary char.
            _ => HanhDong::Chen(ky_tu.to_string()),
        };
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

    /// Chọn đường output khi composition mới (`duong == None`).
    ///
    /// Thứ tự ưu tiên: sensitive → Passthrough; surrounding text hợp lệ
    /// (`van_ban_truoc_con_tro.is_some()`) → VerifiedReplace; preedit có
    /// → PlainComposition; else → Passthrough.
    ///
    /// Route selection dùng `van_ban_truoc_con_tro.is_some()` (trạng thái
    /// surrounding hiện tại), KHÔNG dùng `co_surrounding` (capability): nhiều
    /// app (LibreOffice, Konsole) có `sur_cap=1` nhưng `sur_valid=0` mọi phím —
    /// nếu chọn VerifiedReplace dựa capability, phím 2 bị chặn (verify fail)
    /// và app không gõ được. Dựa trạng thái hiện tại: sur_valid=0 →
    /// PlainComposition (dùng preedit) → app gõ được.
    ///
    /// Kate/Qt: sur_valid=0 phím đầu → PlainComposition (route lock). Phím 2
    /// sur_valid=1 nhưng route đã khóa → tiếp tục PlainComposition. Kate vẫn
    /// gõ được (text trong preedit, commit tại boundary).
    fn chon_duong(&self, boi_canh: &BoiCanhNhap) -> Option<DuongXuat> {
        if boi_canh.co_sensitive {
            return None;
        }
        if boi_canh.van_ban_truoc_con_tro.is_some() {
            return Some(DuongXuat::VerifiedReplace);
        }
        if boi_canh.co_preedit {
            return Some(DuongXuat::PlainComposition);
        }
        None
    }

    /// Kiểm tra surrounding text trước con trỏ kết thúc bằng `da_hien_thi`.
    ///
    /// Trả `true` khi host cung cấp surrounding (`Some`) và nó kết thúc bằng
    /// `da_hien_thi` - tức composition runtime theo dõi vẫn ở đúng vị trí trước
    /// con trỏ. Trả `false` khi surrounding `None` (không thể verify) hoặc khi
    /// surrounding không khớp (text đã thay đổi ngoài runtime).
    ///
    /// Chỉ dùng cho VerifiedReplace. PlainComposition không cần verify (text
    /// nằm trong client preedit, không trong document).
    fn khop_surrounding(&self, boi_canh: &BoiCanhNhap) -> bool {
        match &boi_canh.van_ban_truoc_con_tro {
            Some(vb) => vb.ends_with(&self.da_hien_thi),
            None => false,
        }
    }

    /// Relinquish composition qua host: clear client preedit nếu đang
    /// PlainComposition với text, rồi relinquish internal. Dùng cho
    /// DatLai/DiChuyenConTro/focus-out khi cần clear preedit trên client.
    fn relinquish_voi_host<H: Host>(&mut self, host: &mut H) -> KetQuaXuLy {
        if matches!(self.duong, Some(DuongXuat::PlainComposition)) && !self.da_hien_thi.is_empty() {
            // PlainComposition có text trong preedit — cần clear qua host.
            let ket_qua = host.thuc_thi(&HanhDong::XoaSoanThao);
            match ket_qua {
                KetQuaHost::DaPhat | KetQuaHost::KhongPhat => {
                    self.relinquish();
                    KetQuaXuLy::ChuyenTiep
                }
                KetQuaHost::KhongChac => {
                    self.relinquish();
                    self.trang_thai = TrangThaiPhien::MatDongBo;
                    KetQuaXuLy::MatDongBo
                }
            }
        } else {
            self.relinquish();
            KetQuaXuLy::ChuyenTiep
        }
    }

    /// Relinquish composition: đặt lại Cadence, xóa ownership, về Rong, clear route.
    fn relinquish(&mut self) {
        self.cadence.dat_lai();
        self.da_hien_thi.clear();
        self.lich_su.clear();
        self.trang_thai = TrangThaiPhien::Rong;
        self.duong = None;
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
                co_surrounding: true,
                co_preedit: false,
                co_sensitive: false,
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
                // Host đơn giản không track preedit; các action PlainComposition
                // là no-op (host này có co_preedit=false, không dùng route này).
                HanhDong::CapNhatSoanThao(_) | HanhDong::XoaSoanThao => {}
                HanhDong::KetThucSoanThao(s) => self.van_ban.push_str(s),
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
