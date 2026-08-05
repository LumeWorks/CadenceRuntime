// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Test bảo vệ focus, surrounding mismatch và độc lập context (§17).

mod common;

use cadence_runtime::{ContextId, HanhDong, PhienNhap, SuKienNhap};
use common::HostMoPhong;

fn go_chuoi(phien: &mut PhienNhap, host: &mut HostMoPhong, s: &str) {
    for c in s.chars() {
        phien.xu_ly(host, &SuKienNhap::KyTu(c));
    }
}

#[test]
fn hai_context_doc_lap_khong_chia_se_state() {
    // Context A gõ "as" → "á"; context B gõ "t" → "t". A không bị B ảnh hưởng.
    let mut phien_a = PhienNhap::moi(ContextId(1));
    let mut phien_b = PhienNhap::moi(ContextId(2));
    let mut host_a = HostMoPhong::moi(ContextId(1));
    let mut host_b = HostMoPhong::moi(ContextId(2));

    go_chuoi(&mut phien_a, &mut host_a, "as");
    assert_eq!(host_a.van_ban, "á");
    assert_eq!(phien_a.da_hien_thi(), "á");

    go_chuoi(&mut phien_b, &mut host_b, "t");
    assert_eq!(host_b.van_ban, "t");
    assert_eq!(phien_b.da_hien_thi(), "t");

    // A vẫn "á" sau khi B gõ.
    assert_eq!(host_a.van_ban, "á");
    assert_eq!(phien_a.da_hien_thi(), "á");

    // Tiếp tục gõ ở A: "á" + "s" (thêm sắc? Cadence: "ás" hay "as"?).
    phien_a.xu_ly(&mut host_a, &SuKienNhap::KyTu('s'));
    // A vẫn tách rời B.
    assert_eq!(host_b.van_ban, "t");
    assert_eq!(phien_b.da_hien_thi(), "t");
}

#[test]
fn focus_thay_doi_khong_xoa_dua_tren_suffix_cu() {
    // Đang gõ "as" → "á". Focus generation tăng. Phím kế tiếp không được xóa "á".
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    host.the_he_focus += 1;
    let so_action_truoc = host.lich_su_hanh_dong.len();
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));

    // Sự kiện sau focus change phải forward, không gửi ThayThe xóa "á".
    assert_eq!(ket_qua, cadence_runtime::KetQuaXuLy::ChuyenTiep);
    // Không có action mới (relinquish + forward, không gọi thuc_thi).
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc);
    // Văn bản host không bị xóa.
    assert_eq!(host.van_ban, "á");
    // Runtime đã relinquish: da_hien_thi rỗng, không sở hữu suffix cũ.
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_rong());
}

#[test]
fn mat_focus_khong_xoa_dua_tren_suffix_cu() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    host.dang_co_focus = false;
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(ket_qua, cadence_runtime::KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "");
}

#[test]
fn surrounding_lech_khong_gui_delete() {
    // Runtime nghĩ app có "tie" (da_hien_thi). App thực tế có "abc".
    // Phím kế tiếp không được gửi ThayThe xóa "tie".
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "tie");
    assert_eq!(phien.da_hien_thi(), "tie");
    assert_eq!(host.van_ban, "tie");

    // Ứng dụng tự sửa văn bản ngoài runtime.
    host.van_ban = "abc".to_string();
    host.vi_tri_con_tro = 3;
    let so_action_truoc = host.lich_su_hanh_dong.len();
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('e'));

    // Surrounding "abc" không kết thúc bằng "tie" → forward, không delete.
    assert_eq!(ket_qua, cadence_runtime::KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc);
    // Văn bản app không bị runtime thay đổi.
    assert_eq!(host.van_ban, "abc");
    // Runtime relinquish: không còn sở hữu suffix "tie".
    assert_eq!(phien.da_hien_thi(), "");
}

#[test]
fn surrounding_khop_van_cho_phep_thaythe() {
    // Khi surrounding kết thúc đúng da_hien_thi, ThayThe vẫn gửi bình thường.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");
    // Host surrounding = "á" (khớp da_hien_thi). Gõ thêm 'f' → "à" (huyền).
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('f'));
    // Phải có ThayThe (không bị block).
    let co_thaythe = host
        .lich_su_hanh_dong
        .iter()
        .any(|hd| matches!(hd, HanhDong::ThayThe(_)));
    assert!(co_thaythe, "phai co ThayThe khi surrounding khop");
}

#[test]
fn con_tro_nhay_giua_van_ban_relinquish() {
    // Runtime theo dõi suffix cuối. Cursor nhảy sang giữa → relinquish, không xóa.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // Cursor nhảy về giữa "á" (byte 0).
    host.vi_tri_con_tro = 0;
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::DiChuyenConTro);
    assert_eq!(ket_qua, cadence_runtime::KetQuaXuLy::ChuyenTiep);
    assert_eq!(phien.da_hien_thi(), "");
    // Văn bản không bị xóa.
    assert_eq!(host.van_ban, "á");
}

#[test]
fn context_lech_khong_xoa() {
    // Host trả context_id khác phiên → relinquish + forward, không xóa.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    host.context_id = ContextId(99);
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(ket_qua, cadence_runtime::KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "");
}
