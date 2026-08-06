// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Biến Cadence thành API trung lập cho runtime.
//!
//! Đây là module duy nhất được phép `use cadence::...`. Mọi kiểu của Cadence
//! (`BoGo`, `PhienGo`, `KetQuaXuLy`, `BanChupSoan`, ...) đều bị giữ lại trong
//! boundary này; các module khác chỉ thấy [`PhienCadence`] và [`ChupBan`] trung
//! lập. Nếu API Cadence thay đổi, mục tiêu là chỉ cần sửa file này.

use cadence::{BoGo, CauHinh, KetQuaXuLy, PhienGo};

/// Kết quả thao tác từ Cadence, dạng trung lập (không lộ `KetQuaXuLy`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KetQuaCadence {
    /// Thao tác không thay đổi state (ví dụ vượt giới hạn, hoặc xóa khi rỗng).
    KhongDoi,
    /// State phiên đã thay đổi; snapshot mới lấy qua [`PhienCadence::ban_chup`].
    CapNhat,
}

/// Snapshot trung lập nền tảng, không lộ `BanChupSoan` của Cadence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChupBan {
    /// Nội dung đã render (Telex đã biến đổi).
    pub(crate) noi_dung: String,
}

/// Phiên Cadence được bọc trong boundary.
///
/// Không `Clone` vì `PhienGo` của Cadence không `Clone`. Khi cần quay lui
/// (host từ chối action), `phien.rs` dựng lại phiên qua [`PhienCadence::moi`]
/// rồi replay lịch sử sự kiện đã chấp nhận, thay vì clone.
pub(crate) struct PhienCadence {
    phien: PhienGo,
}

impl PhienCadence {
    /// Tạo phiên Cadence rỗng với cấu hình mặc định.
    ///
    /// `expect` ở đây là bất biến nội bộ đã chứng minh: `CauHinh::mac_dinh()`
    /// đặt giới hạn thao tác 128 (nằm trong `1..=4096`), và `BoGo::new` không
    /// kiểm tra range (chỉ `Ok`), nên không thể lỗi. `BoGo` chỉ dùng để tạo
    /// `PhienGo` ban đầu; không giữ lại vì runtime dựng lại phiên qua `moi()`
    /// khi cần quay lui state.
    pub(crate) fn moi() -> Self {
        let bo_go = BoGo::new(CauHinh::mac_dinh())
            .expect("cau hinh mac dinh dat gioi han 128 trong kho 1..=4096 va BoGo::new khong kiem tra range");
        let phien = bo_go.tao_phien();
        Self { phien }
    }

    /// Thêm ký tự theo chế độ tự động (Telex biến đổi khi áp dụng).
    pub(crate) fn them_ky_tu(&mut self, ky_tu: char) -> KetQuaCadence {
        chuyen_ket_qua(self.phien.them_ky_tu(ky_tu))
    }

    /// Xóa thao tác raw ngay trước con trỏ (backspace hoàn tác một thao tác).
    pub(crate) fn xoa_lui(&mut self) -> KetQuaCadence {
        chuyen_ket_qua(self.phien.xoa_lui())
    }

    /// Đặt lại phiên: xóa toàn bộ lịch sử, con trỏ và snapshot.
    pub(crate) fn dat_lai(&mut self) {
        self.phien.dat_lai();
    }

    /// Đọc snapshot trung lập hiện tại.
    pub(crate) fn ban_chup(&self) -> ChupBan {
        ChupBan {
            noi_dung: String::from(self.phien.ban_chup().noi_dung()),
        }
    }

    /// Trả `true` nếu phiên đang rỗng.
    pub(crate) fn dang_trong(&self) -> bool {
        self.phien.dang_trong()
    }
}

/// Chuyển `KetQuaXuLy` của Cadence sang [`KetQuaCadence`] trung lập.
///
/// `ChapNhan` không xuất hiện ở Phase 1: runtime không gọi `chap_nhan()` mà
/// commit dần qua `ThayThe` mỗi phím; relinquish chỉ `dat_lai()`. Nếu Cadence
/// thêm hành vi commit ngầm, hàm này tĩnh lọc và cần xem lại.
fn chuyen_ket_qua(ket_qua: KetQuaXuLy) -> KetQuaCadence {
    match ket_qua {
        KetQuaXuLy::KhongDoi => KetQuaCadence::KhongDoi,
        KetQuaXuLy::CapNhat => KetQuaCadence::CapNhat,
        KetQuaXuLy::ChapNhan { .. } => KetQuaCadence::CapNhat,
    }
}

#[cfg(test)]
mod tests {
    //! Regression test cho boundary `cadence.rs`: pin trực tiếp API surface của
    //! Cadence mà `PhienCadence` dùng, để một thay đổi API Cadence (xóa/đổi tên
    //! method, đổi `KetQuaXuLy`) bị bắt tại boundary sớm nhất, thay vì chỉ thấy
    //! qua test runtime. Các test end-to-end trong `tests/runtime.rs` vẫn là
    //! nguồn sự thật chính về hành vi Telex.

    use super::*;

    /// Tạo phiên, gõ "as", kiểm tra biến đổi Telex "as" → "á" qua boundary.
    #[test]
    fn phien_cadence_telex_as_thanh_a_sac() {
        let mut phien = PhienCadence::moi();
        assert_eq!(phien.them_ky_tu('a'), KetQuaCadence::CapNhat);
        assert_eq!(phien.them_ky_tu('s'), KetQuaCadence::CapNhat);
        assert!(!phien.dang_trong());
        assert_eq!(phien.ban_chup().noi_dung, "á");
    }

    /// Backspace hoàn tác một thao tác raw: "á" → "a" qua boundary.
    #[test]
    fn phien_cadence_backspace_hoan_tac_tone() {
        let mut phien = PhienCadence::moi();
        phien.them_ky_tu('a');
        phien.them_ky_tu('s');
        assert_eq!(phien.ban_chup().noi_dung, "á");
        assert_eq!(phien.xoa_lui(), KetQuaCadence::CapNhat);
        assert_eq!(phien.ban_chup().noi_dung, "a");
    }

    /// `dat_lai` relinquish: phiên rỗng, snapshot rỗng.
    #[test]
    fn phien_cadence_dat_lai_rong() {
        let mut phien = PhienCadence::moi();
        phien.them_ky_tu('a');
        phien.them_ky_tu('s');
        assert!(!phien.dang_trong());
        phien.dat_lai();
        assert!(phien.dang_trong());
        assert_eq!(phien.ban_chup().noi_dung, "");
    }

    /// Pin ánh xạ `KetQuaXuLy` của Cadence sang `KetQuaCadence`: cả ba nhánh,
    /// kể cả `ChapNhan` (commit ngầm nếu Cadence thêm) phải về `CapNhat` để
    /// runtime không nuốt state.
    #[test]
    fn chuyen_ket_qua_ba_nhanh() {
        assert_eq!(
            chuyen_ket_qua(KetQuaXuLy::KhongDoi),
            KetQuaCadence::KhongDoi
        );
        assert_eq!(chuyen_ket_qua(KetQuaXuLy::CapNhat), KetQuaCadence::CapNhat);
        assert_eq!(
            chuyen_ket_qua(KetQuaXuLy::ChapNhan {
                noi_dung: String::from("x")
            }),
            KetQuaCadence::CapNhat
        );
    }
}
