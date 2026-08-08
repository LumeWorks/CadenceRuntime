// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Test Phase 3C PlainComposition — route thứ hai cho frontend không surrounding
//! text nhưng có client preedit (CapabilityFlag::Preedit).
//!
//! Mô phỏng LibreOffice Writer, Google Chrome, VS Code, Konsole: `sur_valid=0`
//! (hoặc `sur_cap=0`) nhưng `preedit=1`. Phase 3 cũ passthrough toàn bộ →
//! Vietnamese UNSUPPORTED. Phase 3C dùng PlainComposition: cập nhật client
//! preedit (NO decoration) thay vì commit trung gian, commit tại boundary.
//!
//! Bất biến Phase 3C:
//! * Zero visible composition decoration — `CapNhatSoanThao` gửi text với
//!   `TextFormatFlag::NoFlag` (không underline, highlight, bold, italic).
//! * Không commit text trung gian vào document — text chỉ vào document tại
//!   boundary (space, punctuation) qua `KetThucSoanThao`.
//! * Route lock — một composition giữ route từ khi bắt đầu đến boundary/reset.
//! * Sensitive/password context → Passthrough (không preedit).

mod common;

use cantype::{ContextId, HanhDong, KetQuaXuLy, PhienNhap, SuKienNhap};
use common::HostMoPhong;

/// Gõ một chuỗi ký tự vào phiên qua host.
fn go_chuoi(phien: &mut PhienNhap, host: &mut HostMoPhong, s: &str) {
    for c in s.chars() {
        phien.xu_ly(host, &SuKienNhap::KyTu(c));
    }
}

/// Tạo host mô phỏng frontend không surrounding nhưng có preedit (LibreOffice,
/// Chrome, VS Code, Konsole).
fn host_preedit(context_id: ContextId) -> HostMoPhong {
    let mut host = HostMoPhong::moi(context_id);
    host.cung_cap_surrounding = false;
    host.co_surrounding = false;
    host.co_preedit = true;
    host
}

/// Tạo host mô phỏng frontend có surrounding capability nhưng sur_valid=0
/// tạm thời (Kate/Qt phím đầu), có preedit.
fn host_sur_cap_no_sur(context_id: ContextId) -> HostMoPhong {
    let mut host = HostMoPhong::moi(context_id);
    host.cung_cap_surrounding = false; // sur_valid=0 tạm thời
    host.co_surrounding = true; // nhưng có capability
    host.co_preedit = true;
    host
}

// ---------------------------------------------------------------------------
// Route selection (STEP 5).
// ---------------------------------------------------------------------------

/// Frontend không surrounding + có preedit → PlainComposition (KHÔNG Passthrough).
/// Phím đầu → `CapNhatSoanThao` (cập nhật client preedit, NO decoration).
#[test]
fn khong_surrounding_co_preedit_chon_plain_composition() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    // Text nằm trong preedit, KHÔNG trong document.
    assert_eq!(host.van_ban, "");
    assert_eq!(host.preedit, "a");
    assert_eq!(phien.da_hien_thi(), "a");

    // Action phải là CapNhatSoanThao (KHÔNG Chen, KHÔNG ThayThe).
    match &host.lich_su_hanh_dong[0] {
        HanhDong::CapNhatSoanThao(s) => assert_eq!(s, "a"),
        other => panic!("phai la CapNhatSoanThao, duoc {other:?}"),
    }
}

/// Frontend có surrounding capability → VerifiedReplace (ưu tiên hơn
/// PlainComposition). `co_preedit=true` nhưng `co_surrounding=true` →
/// VerifiedReplace.
#[test]
fn co_surrounding_co_preedit_chon_verified_replace() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    host.co_preedit = true; // Có preedit, nhưng surrounding cũng có

    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    // VerifiedReplace: text vào document, KHÔNG vào preedit.
    assert_eq!(host.van_ban, "a");
    assert_eq!(host.preedit, "");
}

/// Kate/Qt: có surrounding capability (`co_surrounding=true`) nhưng `sur_valid=0`
/// ở phím đầu (chưa có text để report). Route selection dùng trạng thái hiện tại
/// (van_ban_truoc_con_tro=None) → PlainComposition (không VerifiedReplace).
/// Phím đầu CapNhatSoanThao, text trong preedit.
#[test]
fn kate_sur_cap_sur_valid_0_chon_plain_composition() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_sur_cap_no_sur(ContextId(1));

    // Phím đầu 'a': sur_valid=0, van_ban_truoc_con_tro=None → PlainComposition.
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    // Text trong preedit, KHÔNG trong document.
    assert_eq!(host.preedit, "a");
    assert_eq!(host.van_ban, "");
    assert_eq!(phien.da_hien_thi(), "a");
}

/// Kate/Qt: phím đầu sur_valid=0 (PlainComposition, route locked), phím 2
/// sur_valid=1 nhưng route đã khóa → tiếp tục PlainComposition. Kate vẫn
/// gõ được "as" → "á" qua preedit.
#[test]
fn kate_phim_dau_plain_phim_2_sur_valid_van_plain() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_sur_cap_no_sur(ContextId(1));

    // Phím 1 'a': sur_valid=0 → PlainComposition.
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(host.preedit, "a");
    assert_eq!(host.van_ban, "");

    // Phím 2: Kate report surrounding hợp lệ (sur_valid=1).
    host.cung_cap_surrounding = true;
    // van_ban="", cursor cuối → surrounding trước cursor = "" (Some, không None).
    // Nhưng route đã khóa PlainComposition → vẫn CapNhatSoanThao.
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(host.preedit, "á");
    assert_eq!(host.van_ban, ""); // vẫn preedit, chưa commit
    assert_eq!(phien.da_hien_thi(), "á");
}

/// Sensitive context → Passthrough (ưu tiên cao nhất, không preedit).
/// `co_sensitive=true` + `co_preedit=true` → Passthrough, không PlainComposition.
#[test]
fn sensitive_context_passthrough_khong_preedit() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    host.co_sensitive = true;
    host.co_preedit = true;

    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "");
    assert_eq!(host.preedit, "");
    assert_eq!(phien.da_hien_thi(), "");
}

/// Password context → Passthrough (co_sensitive bao gồm password).
#[test]
fn password_context_passthrough_khong_preedit() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    host.co_sensitive = true; // password flag → co_sensitive
    host.co_preedit = true;

    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('p'));
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.preedit, "");
    assert_eq!(host.van_ban, "");
}

/// Route lock: một composition giữ route đến boundary. Gõ "as" trên host có
/// preedit → cả hai phím dùng PlainComposition (CapNhatSoanThao).
#[test]
fn route_lock_plain_composition_giu_den_boundary() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    // Phím 1: "a" → CapNhatSoanThao("a")
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(host.preedit, "a");
    assert!(matches!(
        &host.lich_su_hanh_dong[0],
        HanhDong::CapNhatSoanThao(_)
    ));

    // Phím 2: "s" → CapNhatSoanThao("á") (Telex biến đổi "as" → "á")
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(host.preedit, "á");
    assert_eq!(host.van_ban, ""); // vẫn chưa commit
    assert!(matches!(
        &host.lich_su_hanh_dong[1],
        HanhDong::CapNhatSoanThao(_)
    ));
}

// ---------------------------------------------------------------------------
// PlainComposition telex state từng key (STEP 6).
// ---------------------------------------------------------------------------

/// Telex "as" → "á": từng phím cập nhật preedit, không commit document.
#[test]
fn plain_composition_telex_as_thanh_a_sac() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    // 'a' → preedit "a"
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(host.preedit, "a");
    assert_eq!(host.van_ban, "");

    // 's' → preedit "á" (Telex sắc)
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(host.preedit, "á");
    assert_eq!(host.van_ban, ""); // chưa commit
    assert_eq!(phien.da_hien_thi(), "á");
}

/// Telex "tieengs" → "tiếng": từng phím cập nhật preedit, commit tại boundary.
#[test]
fn plain_composition_telex_tieengs_thanh_tieng() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "tieengs");
    // Preedit có "tiếng", document rỗng.
    assert_eq!(host.preedit, "tiếng");
    assert_eq!(host.van_ban, "");
    assert_eq!(phien.da_hien_thi(), "tiếng");

    // Boundary (space): commit "tiếng " vào document, clear preedit.
    phien.xu_ly(&mut host, &SuKienNhap::RanhGioiTu(' '));
    assert_eq!(host.van_ban, "tiếng ");
    assert_eq!(host.preedit, "");
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_rong());
}

/// Telex "af" → "à": Telex huyền, từng phím cập nhật preedit.
#[test]
fn plain_composition_telex_af_thanh_a_huyen() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "af");
    // Cadence Telex (cân bằng): "a" + "f" → "à" (huyền)
    assert_eq!(host.preedit, "à");
    assert_eq!(host.van_ban, "");
}

/// Backspace trong PlainComposition: "á" → backspace → "a" → preedit "a".
#[test]
fn plain_composition_backspace_hoan_tac() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");

    // Backspace → Cadence undo → "a"
    phien.xu_ly(&mut host, &SuKienNhap::XoaLui);
    assert_eq!(host.preedit, "a");
    assert_eq!(host.van_ban, ""); // vẫn preedit, không commit
    assert_eq!(phien.da_hien_thi(), "a");
}

/// Backspace đến empty → XoaSoanThao (clear preedit).
#[test]
fn plain_composition_backspace_den_empty_xoa_soan_thao() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    // 'a' → preedit "a"
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(host.preedit, "a");

    // Backspace → Cadence rỗng → XoaSoanThao
    let kq = phien.xu_ly(&mut host, &SuKienNhap::XoaLui);
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(host.preedit, "");
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_rong());

    // Action cuối phải là XoaSoanThao.
    match host.lich_su_hanh_dong.last() {
        Some(HanhDong::XoaSoanThao) => {}
        Some(other) => panic!("phai la XoaSoanThao, duoc {other:?}"),
        None => panic!("khong co action cuoi"),
    }
}

/// Boundary commit: KetThucSoanThao commit composition + boundary char.
#[test]
fn plain_composition_boundary_commit_ket_thuc_soan_thao() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");

    // Space (boundary): commit "á " vào document, clear preedit.
    phien.xu_ly(&mut host, &SuKienNhap::RanhGioiTu(' '));
    assert_eq!(host.van_ban, "á ");
    assert_eq!(host.preedit, "");
    assert_eq!(phien.da_hien_thi(), "");

    // Action phải là KetThucSoanThao.
    match host.lich_su_hanh_dong.last() {
        Some(HanhDong::KetThucSoanThao(s)) => assert_eq!(s, "á "),
        Some(other) => panic!("phai la KetThucSoanThao, duoc {other:?}"),
        None => panic!("khong co action cuoi"),
    }
}

/// Boundary khi composition rỗng: Chen boundary char (không KetThucSoanThao).
#[test]
fn plain_composition_boundary_rong_chi_chen() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    // Space khi chưa gõ gì → Chen space (composition rỗng).
    phien.xu_ly(&mut host, &SuKienNhap::RanhGioiTu(' '));
    assert_eq!(host.van_ban, " ");
    assert_eq!(host.preedit, "");

    match host.lich_su_hanh_dong.last() {
        Some(HanhDong::Chen(s)) => assert_eq!(s, " "),
        Some(other) => panic!("phai la Chen, duoc {other:?}"),
        None => panic!("khong co action cuoi"),
    }
}

/// Escape/arrow trong PlainComposition → relinquish + clear preedit (XoaSoanThao).
#[test]
fn plain_composition_escape_clear_preedit() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");

    // Escape (DatLai) → relinquish + clear preedit.
    let kq = phien.xu_ly(&mut host, &SuKienNhap::DatLai);
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.preedit, "");
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_rong());

    // Action cuối phải là XoaSoanThao (clear preedit).
    match host.lich_su_hanh_dong.last() {
        Some(HanhDong::XoaSoanThao) => {}
        Some(other) => panic!("phai la XoaSoanThao, duoc {other:?}"),
        None => panic!("khong co action cuoi"),
    }
}

/// Arrow trong PlainComposition → relinquish + clear preedit.
#[test]
fn plain_composition_arrow_clear_preedit() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");

    let kq = phien.xu_ly(&mut host, &SuKienNhap::DiChuyenConTro);
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.preedit, "");
    assert_eq!(phien.da_hien_thi(), "");
}

/// Focus change trong PlainComposition → relinquish + clear preedit.
#[test]
fn plain_composition_focus_change_clear_preedit() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");

    // Focus generation tăng → relinquish + clear preedit.
    host.the_he_focus += 1;
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.preedit, "");
    assert_eq!(phien.da_hien_thi(), "");
}

/// Mất focus trong PlainComposition → relinquish + clear preedit.
#[test]
fn plain_composition_mat_focus_clear_preedit() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");

    host.dang_co_focus = false;
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.preedit, "");
}

/// Sau boundary, gõ tiếp → composition mới, route mới (PlainComposition).
#[test]
fn plain_composition_sau_boundary_composition_moi() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    phien.xu_ly(&mut host, &SuKienNhap::RanhGioiTu(' '));
    assert_eq!(host.van_ban, "á ");
    assert!(phien.dang_rong());

    // Gõ tiếp "as" → composition mới, PlainComposition.
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");
    assert_eq!(host.van_ban, "á "); // document không đổi
}

/// Một sự kiện tối đa một action host (bất biến giữ nguyên).
#[test]
fn plain_composition_mot_su_kien_mot_action() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    let so_su_kien = 5;
    for c in "tieen".chars() {
        phien.xu_ly(&mut host, &SuKienNhap::KyTu(c));
    }
    assert!(
        host.lich_su_hanh_dong.len() <= so_su_kien,
        "lich su action ({}) khong duoc vuot so su kien ({})",
        host.lich_su_hanh_dong.len(),
        so_su_kien
    );
}

/// Zero visible decoration: CapNhatSoanThao không gửi formatting flags.
/// Host mô phỏng chỉ track text, nhưng test pin rằng action là CapNhatSoanThao
/// (không phải ThayThe hay Chen — không commit document trung gian).
#[test]
fn plain_composition_khong_commit_trung_gian_vao_document() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "tieengs");
    // Document rỗng — text chỉ trong preedit.
    assert_eq!(host.van_ban, "");
    // Preedit có "tiếng".
    assert_eq!(host.preedit, "tiếng");

    // Mọi action là CapNhatSoanThao (không Chen, không ThayThe).
    for hd in &host.lich_su_hanh_dong {
        match hd {
            HanhDong::CapNhatSoanThao(_) => {}
            other => panic!("phai la CapNhatSoanThao, duoc {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Fault injection (STEP 12).
// ---------------------------------------------------------------------------

/// KhongPhat trong PlainComposition → rollback Cadence, ChuyenTiep (forward).
#[test]
fn plain_composition_khong_phat_rollback_chuyen_tiep() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");

    // Host trả KhongPhat cho phím kế.
    host.ket_qua_ke_tiep = cantype::KetQuaHost::KhongPhat;
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    // State cũ giữ nguyên: preedit vẫn "á".
    assert_eq!(host.preedit, "á");
    assert_eq!(phien.da_hien_thi(), "á");
}

/// KhongChac trong PlainComposition → MatDongBo, clear preedit (nếu host áp dụng).
#[test]
fn plain_composition_khong_chac_mat_dong_bo() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");

    host.ket_qua_ke_tiep = cantype::KetQuaHost::KhongChac;
    host.khong_chac_ap_dung = false; // host không áp dụng
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::MatDongBo);
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_mat_dong_bo());
}

/// KhongChac khi host áp dụng CapNhatSoanThao → host có preedit, runtime MatDongBo.
#[test]
fn plain_composition_khong_chac_host_ap_dung() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");

    host.ket_qua_ke_tiep = cantype::KetQuaHost::KhongChac;
    host.khong_chac_ap_dung = true; // host áp dụng action
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::MatDongBo);
    // Runtime reset (MatDongBo), không sở hữu suffix.
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_mat_dong_bo());
}

/// KhongChac khi KetThucSoanThao (boundary) — host commit một phần.
#[test]
fn plain_composition_khong_chac_tai_boundary() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.preedit, "á");

    host.ket_qua_ke_tiep = cantype::KetQuaHost::KhongChac;
    host.khong_chac_ap_dung = false;
    let kq = phien.xu_ly(&mut host, &SuKienNhap::RanhGioiTu(' '));
    assert_eq!(kq, KetQuaXuLy::MatDongBo);
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_mat_dong_bo());
}

/// Hai context độc lập: A dùng PlainComposition, B dùng VerifiedReplace.
#[test]
fn hai_context_a_plain_b_verified_doc_lap() {
    let mut phien_a = PhienNhap::moi(ContextId(1));
    let mut phien_b = PhienNhap::moi(ContextId(2));
    let mut host_a = host_preedit(ContextId(1)); // không surrounding, có preedit
    let mut host_b = HostMoPhong::moi(ContextId(2)); // có surrounding

    // A: PlainComposition — preedit "á", document rỗng.
    go_chuoi(&mut phien_a, &mut host_a, "as");
    assert_eq!(host_a.preedit, "á");
    assert_eq!(host_a.van_ban, "");

    // B: VerifiedReplace — document "á", preedit rỗng.
    go_chuoi(&mut phien_b, &mut host_b, "as");
    assert_eq!(host_b.van_ban, "á");
    assert_eq!(host_b.preedit, "");

    // A không bị B ảnh hưởng.
    assert_eq!(host_a.preedit, "á");
    assert_eq!(phien_a.da_hien_thi(), "á");
}

/// PlainComposition: gõ "vowsi" → "với" (acceptance corpus).
#[test]
fn plain_composition_vowsi_thanh_voi() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = host_preedit(ContextId(1));

    go_chuoi(&mut phien, &mut host, "vowsi");
    assert_eq!(host.preedit, "với");
    assert_eq!(host.van_ban, "");

    // Commit tại boundary.
    phien.xu_ly(&mut host, &SuKienNhap::RanhGioiTu(' '));
    assert_eq!(host.van_ban, "với ");
    assert_eq!(host.preedit, "");
}
