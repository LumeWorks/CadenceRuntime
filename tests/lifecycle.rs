// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Test lifecycle context Fcitx5 (§4 Phase 2).
//!
//! Các test này kiểm chứng contract lifecycle ở tầng runtime `PhienNhap`, mô
//! phỏng lifecycle event của adapter C++ qua `PhienNhap::dat_lai()` (FFI
//! `cadence_phien_dat_lai` gọi đúng phương thức này cho activate/deactivate/
//! reset/focus-out) và bump `the_he_focus` cho focus-in/activate. Bất biến:
//! focus-out/deactivate/reset/switch/destroy không giữ ownership suffix cũ, và
//! phím đầu sau lifecycle compose tươi (không bị "ăn").
//!
//! Hai test lifecycle C++-only (restart Fcitx load/unload không crash, context
//! destroy free đúng một lần ở tầng C++) không kiểm chứng được ở tầng Rust; ta
//! đảm bảo bằng code review (factory static cục bộ, không global state, per-IC
//! property, destructor free đúng một lần) và smoke test Fcitx5 thật.

mod common;

use cadence_runtime::{ContextId, KetQuaXuLy, PhienNhap, SuKienNhap};
use common::HostMoPhong;

fn go_chuoi(phien: &mut PhienNhap, host: &mut HostMoPhong, s: &str) {
    for c in s.chars() {
        phien.xu_ly(host, &SuKienNhap::KyTu(c));
    }
}

/// 1. focus-out xóa ownership: sau `dat_lai` (focus-out), da_hien_thi rỗng;
///    phím kế compose tươi, không xóa text đã commit.
#[test]
fn focus_out_xoa_ownership_compose_tuoi() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // Focus-out: adapter gọi cadence_phien_dat_lai.
    phien.dat_lai();
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_rong());

    // Focus-in: focus_generation tăng.
    host.the_he_focus += 1;
    // Phím đầu sau focus cycle compose tươi (Chen), không bị forward.
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(host.van_ban, "áa");
    assert_eq!(phien.da_hien_thi(), "a");
}

/// 2. focus-in tạo generation mới: session adopt generation mới, composition
///    cũ không được dùng. "as" sau focus cycle vẫn ra "á".
#[test]
fn focus_in_tao_generation_moi_compose_lai() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // Focus cycle: out + in (bump gen).
    phien.dat_lai();
    host.the_he_focus += 1;

    // Gõ lại "as" → "á" (composition tươi, không dùng suffix cũ).
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "áá");
    assert_eq!(phien.da_hien_thi(), "á");
}

/// 3. deactivate xóa ownership: `dat_lai` (deactivate) relinquish; phím kế fresh.
#[test]
fn deactivate_xoa_ownership() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(phien.da_hien_thi(), "á");

    phien.dat_lai();
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_rong());

    // Phím kế (cùng focus gen, vì deactivate rồi activate lại cùng gen) compose.
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(phien.da_hien_thi(), "d");
}

/// 4. reset khi vẫn focus: `dat_lai` (reset) relinquish + xóa focus adoption;
///    phím kế compose tươi (không forward). "as" sau reset ra "á".
#[test]
fn reset_khi_van_focus_compose_tuoi() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // Reset (ic vẫn focus, không bump gen từ phía host mô phỏng).
    phien.dat_lai();
    assert!(phien.dang_rong());

    // Phím đầu sau reset phải COMPOSE, không bị forward (không "ăn" phím).
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq, KetQuaXuLy::DaApDung, "phim dau sau reset phai compose");
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(host.van_ban, "áá");
    assert_eq!(phien.da_hien_thi(), "á");
}

/// 5. đổi input method: deactivate (dat_lai) + activate (bump gen); phím kế
///    compose tươi, không gửi delete dựa suffix cũ.
#[test]
fn doi_input_method_relinquish_va_compose_tuoi() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // Switch IM: deactivate relinquish, activate bump gen.
    phien.dat_lai();
    host.the_he_focus += 1;

    let so_action_truoc = host.lich_su_hanh_dong.len();
    // Phím kế: Chen tươi, KHÔNG có ThayThe xóa "á".
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    // Chỉ thêm 1 action Chen, không có delete.
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc + 1);
    assert_eq!(host.van_ban, "ád");
    assert_eq!(phien.da_hien_thi(), "d");
}

/// 6. context destroy / idempotent: `dat_lai` gọi nhiều lần an toàn (không
///    double-free, không panic). Đây là nền tảng cho C++ destructor free đúng
///    một lần và helper lifecycle idempotent.
#[test]
fn dat_lai_idempotent_an_toan_nhieu_lan() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");

    // Gọi dat_lai nhiều lần (mô phỏng focus-out rồi deactivate).
    phien.dat_lai();
    phien.dat_lai();
    phien.dat_lai();
    assert!(phien.dang_rong());
    assert_eq!(phien.da_hien_thi(), "");

    // Session vẫn dùng được sau nhiều reset.
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(phien.da_hien_thi(), "a");
}

/// 7. focus-out rồi deactivate: hai relinquish liên tiếp không double-free, không
///    bump vô hạn (chỉ relinquish no-op lần hai). Phím kế compose tươi.
#[test]
fn focus_out_roi_deactivate_idempotent() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // Focus-out rồi deactivate (cả hai gọi dat_lai).
    phien.dat_lai();
    phien.dat_lai();
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_rong());

    // Phím kế compose tươi.
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(host.van_ban, "áa");
}

/// 8. hai context đổi focus liên tục: A focus, B focus, A lại focus; A compose
///    tươi, không lẫn state với B.
#[test]
fn hai_context_doi_focus_lien_tuc() {
    let mut phien_a = PhienNhap::moi(ContextId(1));
    let mut phien_b = PhienNhap::moi(ContextId(2));
    let mut host_a = HostMoPhong::moi(ContextId(1));
    let mut host_b = HostMoPhong::moi(ContextId(2));

    // A gõ "as" → "á".
    go_chuoi(&mut phien_a, &mut host_a, "as");
    assert_eq!(host_a.van_ban, "á");

    // A mất focus (focus-out), B nhận focus (focus-in).
    phien_a.dat_lai();
    host_a.the_he_focus += 1;
    go_chuoi(&mut phien_b, &mut host_b, "t");
    assert_eq!(host_b.van_ban, "t");

    // B mất focus, A nhận focus lại.
    phien_b.dat_lai();
    host_b.the_he_focus += 1;

    // A compose tươi: 'd' → Chen, không lẫn state B.
    let kq = phien_a.xu_ly(&mut host_a, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(host_a.van_ban, "ád");
    assert_eq!(phien_a.da_hien_thi(), "d");
    // B vẫn "t", không bị A ảnh hưởng.
    assert_eq!(host_b.van_ban, "t");
}

/// 9. reset không commit không delete: `dat_lai` không gọi `thuc_thi` (không
///    thêm action vào host), chỉ xóa ownership runtime.
#[test]
fn reset_khong_commit_khong_delete() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    let so_action_truoc = host.lich_su_hanh_dong.len();

    phien.dat_lai();

    // dat_lai không gửi action nào tới host.
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc);
    // Văn bản "á" không bị xóa.
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "");
}

/// 10. sự kiện passthrough sau reset: sau `dat_lai`, `DiChuyenConTro` và
///     `DatLai` (qua xu_ly) → ChuyenTiep (forward), không tạo text.
#[test]
fn passthrough_sau_reset() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    phien.dat_lai();

    let so_action_truoc = host.lich_su_hanh_dong.len();
    let kq = phien.xu_ly(&mut host, &SuKienNhap::DiChuyenConTro);
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc);

    let kq = phien.xu_ly(&mut host, &SuKienNhap::DatLai);
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "á");
}

/// 12. session cũ không được dùng sau focus cycle mới: sau focus cycle,
///     da_hien_thi cũ ("á") không còn, và phím kế không delete dựa nó.
#[test]
fn session_cu_khong_dung_sau_focus_cycle_moi() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(phien.da_hien_thi(), "á");

    // Focus cycle mới.
    phien.dat_lai();
    host.the_he_focus += 1;

    // Ứng dụng tự sửa text ngoài runtime (xóa "á", chèn "xyz").
    host.van_ban = "xyz".to_string();
    host.vi_tri_con_tro = 3;

    // Phím kế: phải compose tươi (Chen 'd' vào sau "xyz"), KHÔNG delete dựa
    // suffix "á" cũ (đã relinquish).
    let so_action_truoc = host.lich_su_hanh_dong.len();
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc + 1);
    assert_eq!(host.van_ban, "xyzd");
    assert_eq!(phien.da_hien_thi(), "d");
}
