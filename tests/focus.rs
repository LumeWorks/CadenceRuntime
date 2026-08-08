// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Test bảo vệ focus, surrounding mismatch và độc lập context (§17).

mod common;

use cantype::{ContextId, HanhDong, KetQuaXuLy, PhienNhap, SuKienNhap};
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
    assert_eq!(ket_qua, cantype::KetQuaXuLy::ChuyenTiep);
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
    assert_eq!(ket_qua, cantype::KetQuaXuLy::ChuyenTiep);
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
    assert_eq!(ket_qua, cantype::KetQuaXuLy::ChuyenTiep);
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
    assert_eq!(ket_qua, cantype::KetQuaXuLy::ChuyenTiep);
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
    assert_eq!(ket_qua, cantype::KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "");
}

#[test]
fn cursor_di_chuyen_phat_hien_qua_surrounding_lech() {
    // Phase 1 verify cursor gián tiếp qua van_ban_truoc_con_tro: không có
    // trường cursor riêng. Khi cursor di chuyển, surrounding text thay đổi
    // và runtime phát hiện mismatch (không chỉ qua sự kiện DiChuyenConTro).
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");
    assert_eq!(host.vi_tri_con_tro, 2); // cursor sau "á"

    // Cursor nhảy về đầu (position 0) — surrounding becomes "".
    host.vi_tri_con_tro = 0;
    // Gõ 'f' → Cadence "áf" → "à" (ThayThe: xóa "á", chèn "à").
    // Surrounding "" không kết thúc bằng "á" → relinquish + ChuyenTiep.
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('f'));
    assert_eq!(ket_qua, KetQuaXuLy::ChuyenTiep);
    // Văn bản "á" không bị xóa.
    assert_eq!(host.van_ban, "á");
    // Runtime relinquish: không sở hữu suffix cũ.
    assert_eq!(phien.da_hien_thi(), "");
}

// --- Blocker 1: surrounding=None + không preedit → Passthrough ---
//
// Phase 3C: khi host không cung cấp surrounding text VÀ không hỗ trợ preedit,
// runtime không có route khả dụng (VerifiedReplace cần surrounding, PlainComposition
// cần preedit) → Passthrough: forward mọi phím raw, KHÔNG Chen, KHÔNG ThayThe,
// KHÔNG preedit. Đây là thay đổi so với Phase 3, nơi phím đầu Chen thuần được
// phép khi da_hien_thi rỗng. Phase 3C bỏ hành vi đó: không route → passthrough.

#[test]
fn surrounding_none_khong_preedit_passthrough_tat_ca_phim() {
    // surrounding=None + co_preedit=false → Passthrough. Mọi phím ChuyểnTiep,
    // không Chen, không ThayThe, không preedit. Văn bản host rỗng.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    host.cung_cap_surrounding = false;
    host.co_surrounding = false;
    host.co_preedit = false;

    // 'a' → Passthrough (không route khả dụng).
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(ket_qua, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "");
    assert_eq!(phien.da_hien_thi(), "");

    // 's' → cũng Passthrough. Không ThayThe, không Chen.
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(ket_qua, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "");
    assert_eq!(phien.da_hien_thi(), "");

    // Không có action nào trong history.
    assert!(host.lich_su_hanh_dong.is_empty());
}

#[test]
fn surrounding_none_khong_preedit_khong_so_huu_suffix() {
    // surrounding=None + co_preedit=false: runtime không bao giờ sở hữu suffix.
    // Mọi phím passthrough → da_hien_thi luôn rỗng → không bao giờ delete mù.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    host.cung_cap_surrounding = false;
    host.co_surrounding = false;
    host.co_preedit = false;

    phien.xu_ly(&mut host, &SuKienNhap::KyTu('a')); // ChuyểnTiep
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('b')); // ChuyểnTiep

    // Runtime không sở hữu suffix.
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_rong());

    // Backspace: Cadence rỗng → KhongDoi → ChuyểnTiep.
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::XoaLui);
    assert_eq!(ket_qua, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "");
}

#[test]
fn surrounding_lech_chen_khi_dang_so_huu_suffix_bi_chan() {
    // da_hien_thi không rỗng + surrounding mismatch + Chen → relinquish +
    // ChuyenTiep. Không chèn "z" vào "abc" (sai vị trí).
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "tie");
    assert_eq!(phien.da_hien_thi(), "tie");
    assert_eq!(host.van_ban, "tie");

    // App sửa text ngoài runtime: "tie" → "abc", cursor ở cuối.
    host.van_ban = "abc".to_string();
    host.vi_tri_con_tro = 3;

    // Gõ 'z' → Cadence "tiez" (la_chen: chèn "z"). Nhưng surrounding "abc"
    // không kết thúc bằng "tie" → relinquish + ChuyenTiep.
    let so_action_truoc = host.lich_su_hanh_dong.len();
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('z'));
    assert_eq!(ket_qua, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc);
    // Văn bản "abc" không bị chèn "z" vào.
    assert_eq!(host.van_ban, "abc");
    // Runtime relinquish: không sở hữu suffix cũ.
    assert_eq!(phien.da_hien_thi(), "");
}

#[test]
fn surrounding_none_khong_preedit_passthrough_khong_nuot_phim() {
    // surrounding=None + co_preedit=false → Passthrough. Sự kiện không bị nuốt:
    // runtime forward raw key (ChuyểnTiep), không Chen/ThayThe.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    host.cung_cap_surrounding = false;
    host.co_surrounding = false;
    host.co_preedit = false;

    // 'a' → Passthrough.
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(ket_qua, KetQuaXuLy::ChuyenTiep);
    assert_eq!(phien.da_hien_thi(), "");

    // 's' → cũng Passthrough. Không ThayThe, không Chen, không nuốt.
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(ket_qua, KetQuaXuLy::ChuyenTiep);
    assert_eq!(phien.da_hien_thi(), "");

    // 'd' → Passthrough tiếp.
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(ket_qua, KetQuaXuLy::ChuyenTiep);
    assert_eq!(phien.da_hien_thi(), "");
}

#[test]
fn surrounding_none_context_khac_khong_anh_huong() {
    // Context A: không surrounding + không preedit → Passthrough. Context B:
    // có surrounding → VerifiedReplace (Chen/ThayThe). A không bị B ảnh hưởng.
    let mut phien_a = PhienNhap::moi(ContextId(1));
    let mut host_a = HostMoPhong::moi(ContextId(1));
    host_a.cung_cap_surrounding = false;
    host_a.co_surrounding = false;
    host_a.co_preedit = false;

    let mut phien_b = PhienNhap::moi(ContextId(2));
    let mut host_b = HostMoPhong::moi(ContextId(2));
    // host_b.cung_cap_surrounding = true (mặc định)

    // A: 'a' → Passthrough, 's' → Passthrough.
    phien_a.xu_ly(&mut host_a, &SuKienNhap::KyTu('a'));
    phien_a.xu_ly(&mut host_a, &SuKienNhap::KyTu('s'));
    assert_eq!(host_a.van_ban, "");
    assert_eq!(phien_a.da_hien_thi(), "");

    // B: 'a' → Chen, 's' → ThayThe (surrounding khớp).
    phien_b.xu_ly(&mut host_b, &SuKienNhap::KyTu('a'));
    phien_b.xu_ly(&mut host_b, &SuKienNhap::KyTu('s'));
    assert_eq!(host_b.van_ban, "á");
    assert_eq!(phien_b.da_hien_thi(), "á");

    // A không bị B ảnh hưởng.
    assert_eq!(host_a.van_ban, "");
    assert_eq!(phien_a.da_hien_thi(), "");
}
