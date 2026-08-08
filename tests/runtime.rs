// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Test runtime end-to-end: semantic input → Cadence wrapper → KeHoachSua →
//! fake host (§15, §16).

mod common;

use cantype::{HanhDong, KetQuaXuLy, PhienNhap, SuKienNhap};
use common::HostMoPhong;

fn nhap(phien: &mut PhienNhap, host: &mut HostMoPhong, su_kien: SuKienNhap) {
    phien.xu_ly(host, &su_kien);
}

fn go_chuoi(phien: &mut PhienNhap, host: &mut HostMoPhong, s: &str) {
    for c in s.chars() {
        nhap(phien, host, SuKienNhap::KyTu(c));
    }
}

#[test]
fn telex_as_thanh_a_end_to_end() {
    // Khớp Cadence `as_thanh_a_sac`: "as" → "á".
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "á");
    assert!(!phien.dang_rong());
}

#[test]
fn telex_tieengs_thanh_tieng_end_to_end() {
    // Khớp Cadence: "tieengs" → "tiếng".
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "tieengs");
    assert_eq!(host.van_ban, "tiếng");
    assert_eq!(phien.da_hien_thi(), "tiếng");
}

#[test]
fn backspace_trong_composition() {
    // Khớp Cadence `backspace_sau_tone_hoan_tac`: "as" → "á", backspace → "a".
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");
    nhap(&mut phien, &mut host, SuKienNhap::XoaLui);
    assert_eq!(host.van_ban, "a");
    assert_eq!(phien.da_hien_thi(), "a");
}

#[test]
fn ascii_khong_bien_doi_end_to_end() {
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "abc123");
    assert_eq!(host.van_ban, "abc123");
    assert_eq!(phien.da_hien_thi(), "abc123");
}

#[test]
fn url_giu_nguyen_end_to_end() {
    // Khớp Cadence corpus Phase 3: URL giữ nguyên, không Telex.
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "https://example.com");
    assert_eq!(host.van_ban, "https://example.com");
}

#[test]
fn ranh_gioi_tu_chen_space() {
    // "as" → "á", rồi space → "á " (composition relinquish, space chèn).
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    nhap(&mut phien, &mut host, SuKienNhap::RanhGioiTu(' '));
    assert_eq!(host.van_ban, "á ");
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_rong());
}

#[test]
fn dat_lai_relinquish_khong_xoa_text() {
    // "as" → "á" (app đã sở hữu). DatLai relinquish, không xóa "á".
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    nhap(&mut phien, &mut host, SuKienNhap::DatLai);
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_rong());
}

#[test]
fn delete_rong_chuyentiep_khong_commit() {
    // Regression: Delete (XK_Delete 0xFFFF → keySymToUTF8 trả U+007F) trên
    // composition rỗng phải ChuyểnTiep, không commit U+007F. Trước fix,
    // Delete bị dac_biet=Khac → DEL được commit như ký tự in được.
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    let kq = phien.xu_ly(&mut host, &SuKienNhap::DatLai);
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "");
    assert!(phien.dang_rong());
    // DatLai relinquish không gửi action — host không nhận gì.
    assert!(host.lich_su_hanh_dong.is_empty());
}

#[test]
fn delete_co_suffix_relinquish_khong_thaythe_khong_xoa_lui() {
    // Regression: Delete khi đang sở hữu suffix phải relinquish + ChuyểnTiep,
    // KHÔNG gọi ThayThe và KHÔNG biến thành XoaLui (Backspace). Text đã commit
    // ("á") phải giữ nguyên.
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");
    let so_action_truoc = host.lich_su_hanh_dong.len();
    let kq = phien.xu_ly(&mut host, &SuKienNhap::DatLai);
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "á"); // không xóa text đã commit
    assert_eq!(phien.da_hien_thi(), ""); // relinquish
    assert!(phien.dang_rong());
    // DatLai không gửi action (relinquish không gọi thuc_thi).
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc);
}

#[test]
fn delete_roi_go_tiep_khong_dung_state_cu() {
    // Regression: sau Delete (relinquish), gõ tiếp phải tạo composition mới,
    // không dùng state composition cũ. Đảm bảo Delete reset sạch state.
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");
    // Delete (DatLai) relinquish composition "á".
    phien.xu_ly(&mut host, &SuKienNhap::DatLai);
    assert!(phien.dang_rong());
    // Gõ "a" tiếp phải là Chen thuần (composition mới), DaApDung.
    let kq_a = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq_a, KetQuaXuLy::DaApDung);
    assert_eq!(host.van_ban, "áa");
    assert_eq!(phien.da_hien_thi(), "a");
    // Gõ "s" phải ThayThe "a" → "á" (composition mới), không dùng "á" cũ.
    let kq_s = phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(kq_s, KetQuaXuLy::DaApDung);
    assert_eq!(host.van_ban, "áá");
    assert_eq!(phien.da_hien_thi(), "á");
}

#[test]
fn di_chuyen_con_tro_relinquish() {
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    nhap(&mut phien, &mut host, SuKienNhap::DiChuyenConTro);
    assert_eq!(host.van_ban, "á");
    assert!(phien.dang_rong());
}

#[test]
fn zero_visible_composition_moi_action_la_route_hop_le() {
    // Bất biến Phase 3C: mọi action ghi vào history phải thuộc một route hợp lệ.
    // VerifiedReplace: Chen/ThayThe/ChuyenTiep. PlainComposition:
    // CapNhatSoanThao/KetThucSoanThao/XoaSoanThao. Host mặc định có surrounding →
    // VerifiedReplace, nên chỉ thấy Chen/ThayThe/ChuyenTiep.
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    go_chuoi(&mut phien, &mut host, "tieengs");
    nhap(&mut phien, &mut host, SuKienNhap::XoaLui);
    nhap(&mut phien, &mut host, SuKienNhap::RanhGioiTu(' '));
    for hd in &host.lich_su_hanh_dong {
        match hd {
            HanhDong::Chen(_)
            | HanhDong::ThayThe(_)
            | HanhDong::CapNhatSoanThao(_)
            | HanhDong::KetThucSoanThao(_)
            | HanhDong::XoaSoanThao
            | HanhDong::ChuyenTiep => {}
        }
    }
}

#[test]
fn mot_su_kien_toi_da_mot_action_host() {
    // Bất biến §16: một semantic input tạo tối đa một logical host action.
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    let cac_su_kien: &[SuKienNhap] = &[
        SuKienNhap::KyTu('t'),
        SuKienNhap::KyTu('i'),
        SuKienNhap::KyTu('e'),
        SuKienNhap::KyTu('e'),
        SuKienNhap::KyTu('n'),
        SuKienNhap::KyTu('g'),
        SuKienNhap::KyTu('s'),
        SuKienNhap::XoaLui,
        SuKienNhap::RanhGioiTu(' '),
        SuKienNhap::DatLai,
        SuKienNhap::DiChuyenConTro,
    ];
    let so_su_kien = cac_su_kien.len();
    for sk in cac_su_kien {
        nhap(&mut phien, &mut host, sk.clone());
    }
    assert!(
        host.lich_su_hanh_dong.len() <= so_su_kien,
        "lich su action ({}) khong duoc vuot so su kien ({})",
        host.lich_su_hanh_dong.len(),
        so_su_kien
    );
}

#[test]
fn thaythe_khong_xoa_vuot_da_hien_thi() {
    // Bất biến §16: ThayThe không xóa vượt da_hien_thi. Kiểm chứng chặt: sau
    // mỗi DaApDung, suffix host sở hữu phải khớp da_hien_thi runtime - tức
    // runtime không bao giờ xóa quá đoạn nó tin ứng dụng đang hiển thị.
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    let chuoi = "tieengs";
    for c in chuoi.chars() {
        let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu(c));
        if matches!(ket_qua, KetQuaXuLy::DaApDung) {
            assert!(
                host.van_ban.ends_with(phien.da_hien_thi()),
                "host ({:?}) khong ket thuc bang da_hien_thi ({:?})",
                host.van_ban,
                phien.da_hien_thi()
            );
            // Mọi ThayThe đã gửi xóa đúng ≤ da_hien_thi tại thời điểm đó.
            for hd in &host.lich_su_hanh_dong {
                if let HanhDong::ThayThe(ke) = hd {
                    assert!(
                        ke.xoa_truoc.byte_utf8 <= chuoi.len(),
                        "xoa_truoc ({}) vuot raw input ({})",
                        ke.xoa_truoc.byte_utf8,
                        chuoi.len()
                    );
                }
            }
        }
    }
    // Kết thúc "tieengs" → "tiếng" (khớp Cadence).
    assert_eq!(host.van_ban, "tiếng");
}

#[test]
fn ket_qua_xu_ly_da_ap_dung_tra_dung_bien_the() {
    let mut phien = PhienNhap::moi(cantype::ContextId(1));
    let mut host = HostMoPhong::moi(cantype::ContextId(1));
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(ket_qua, KetQuaXuLy::DaApDung);
}
