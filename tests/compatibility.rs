// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Test tương thích Phase 3 — chứng minh invariant Route A (NativeReplace) và
//! containment khi frontend không cung cấp surrounding text hợp lệ.
//!
//! Khác `tests/focus.rs` (test runtime core), file này mô phỏng contract
//! frontend thật đã khám phá qua source Fcitx 5.1.12 + fcitx5-gtk/fcitx5-qt:
//!
//! * frontend không surrounding (LibreOffice GTK3 `sur_valid=0`, XIM no-op):
//!   runtime phải passthrough an toàn, KHÔNG destructive NativeReplace.
//! * mouse click di chuyển cursor khi surrounding None (GTK4/Qt generic):
//!   runtime không nhận reset/invalidation → KHÔNG được delete dựa suffix cũ.
//! * partial dispatch (forwardKey Backspace + commit fail): mô phỏng Route B
//!   hypothetical — runtime không rollback text, không replay mù.
//! * two contexts độc lập qua contract khác nhau (A có surrounding, B không).
//!
//! Test ở tầng runtime thuần (PhienNhap + Host mô phỏng), không cần Fcitx5
//! dev. Route B (forwardKey) KHÔNG được implement production (decision gate
//! REJECTED), nên test partial dispatch dùng fault injection host mô phỏng
//! để pin invariant containment nếu route tương lai được xét lại.
//!
//! Xem `docs/PHASE_3_COMPATIBILITY.md` §Route B research cho bằng chứng
//! cursor invalidation contract yếu trên GTK4/Qt/LibreOffice.

mod common;

use cantype::{ContextId, KetQuaXuLy, PhienNhap, SuKienNhap};
use common::HostMoPhong;

/// Gõ một chuỗi ký tự vào phiên qua host.
fn go_chuoi(phien: &mut PhienNhap, host: &mut HostMoPhong, s: &str) {
    for c in s.chars() {
        phien.xu_ly(host, &SuKienNhap::KyTu(c));
    }
}

// ---------------------------------------------------------------------------
// Frontend không surrounding (LibreOffice GTK3, XIM).
// ---------------------------------------------------------------------------

/// LibreOffice GTK3: capability có SurroundingText nhưng `sur_valid=0` cho
/// mọi key. Runtime không NativeReplace → phím đầu Chen thuần, phím kế bị
/// chặn (relinquish + ChuyểnTiep), không delete mù.
#[test]
fn frontend_khong_surrounding_phim_dau_chen_khong_delete_mù() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    host.cung_cap_surrounding = false;

    // Phím đầu "a": da_hien_thi rỗng → Chen thuần OK (không cần surrounding).
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(host.van_ban, "a");
    assert_eq!(phien.da_hien_thi(), "a");

    // Phím "s" kế: da_hien_thi="a" không rỗng, surrounding None → không
    // verify được → relinquish + ChuyểnTiep, KHÔNG ThayThe, không delete "a".
    let so_action_truoc = host.lich_su_hanh_dong.len();
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    // Không có action host mới (relinquish là internal, không thuc_thi).
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc);
    // Text "a" không bị xóa.
    assert_eq!(host.van_ban, "a");
    // Runtime đã relinquish ownership.
    assert_eq!(phien.da_hien_thi(), "");
}

/// Frontend không surrounding: Backspace khi đang sở hữu suffix → relinquish +
/// ChuyểnTiep, KHÔNG xóa lùi trong Cadence (không có gì verify vị trí cursor).
#[test]
fn frontend_khong_surrounding_backspace_khong_xoa_lui_mù() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    host.cung_cap_surrounding = false;

    // Phím đầu Chen (OK). Phím kế bị chặn nên da_hien_thi rỗng.
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('s')); // ChuyểnTiep, relinquish

    // Backspace: da_hien_thi rỗng, Cadence rỗng → Cadence KhongDoi → ChuyểnTiep.
    let kq = phien.xu_ly(&mut host, &SuKienNhap::XoaLui);
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "a");
}

// ---------------------------------------------------------------------------
// Mouse click di chuyển cursor khi surrounding None (GTK4/Qt generic).
// ---------------------------------------------------------------------------

/// Mouse click di chuyển cursor trong cùng widget, frontend không surrounding
/// → runtime KHÔNG nhận reset/invalidation. Phím kế tiếp KHÔNG được delete
/// dựa suffix cũ (vì cursor đã lệch, runtime không biết).
///
/// Mô phỏng: host gõ "as" → "á" (Route A, có surrounding). Sau đó click làm
/// cursor lệch: host set surrounding None (mô phỏng app không báo). Phím kế
/// phải relinquish + ChuyểnTiep, không ThayThe ở vị trí cũ.
#[test]
fn click_di_chuyen_cursor_khong_surrounding_khong_delete_cu() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    // Gõ "as" → "á" (Route A hoạt động, surrounding có).
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "á");

    // Mouse click: cursor lệch, app không báo surrounding (GTK4/Qt generic).
    // Mô phỏng: host ngừng cung cấp surrounding. Runtime giữ da_hien_thi="á".
    host.cung_cap_surrounding = false;

    // Phím "s" kế: da_hien_thi="á" không rỗng, surrounding None → relinquish +
    // ChuyểnTiep. KHÔNG ThayThe ở vị trí cũ (corruption risk chặn).
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    // "á" không bị xóa/sửa.
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "");
}

/// Click sau khi gõ rồi gõ tiếp: composition mới tươi, không dùng state cũ.
#[test]
fn click_roi_go_tiep_composition_moi_tuoi() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // Click → surrounding None (cursor lệch). Phím "s" bị relinquish +
    // ChuyểnTiep (không ThayThe ở vị trí cũ). Runtime mất ownership.
    host.cung_cap_surrounding = false;
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(phien.da_hien_thi(), "");

    // App đã nhận "s" raw (do adapter forward phím gốc). Mô phỏng cursor ở
    // vị trí mới, text "ás". Surrounding lại có (app cập nhật).
    host.cung_cap_surrounding = true;
    host.van_ban = "ás".to_string();
    host.vi_tri_con_tro = 3; // byte offset cuối "ás" (á=2 byte, s=1 byte)

    // Phím đầu sau click: da_hien_thi rỗng → Chen thuần "a", KHÔNG delete "ás".
    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));
    assert_eq!(kq, KetQuaXuLy::DaApDung);
    assert_eq!(host.van_ban, "ása");
    assert_eq!(phien.da_hien_thi(), "a");
}

// ---------------------------------------------------------------------------
// Selection (cursor ≠ anchor) khi surrounding có.
// ---------------------------------------------------------------------------

/// Selection (cursor ≠ anchor) phát hiện qua surrounding mismatch:
/// surrounding không kết thúc bằng da_hien_thi → relinquish + ChuyểnTiep.
#[test]
fn selection_khi_surrounding_lech_relinquish() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // User select text "á" (cursor=0, anchor=1) — surrounding trước cursor
    // là "" không kết thúc "á" → mismatch.
    host.vi_tri_con_tro = 0;

    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "");
}

// ---------------------------------------------------------------------------
// Partial dispatch — mô phỏng Route B (forwardKey + commit) fail giữa chừng.
// Route B KHÔNG implement production (REJECTED), test pin containment.
// ---------------------------------------------------------------------------

/// Mô phỏng Route B partial: host đã phát một Backspace nhưng commit fail
/// (KetQuaHost::KhongChac). Runtime phải MatDongBo, KHÔNG rollback text,
/// KHÔNG replay phím đã xử lý.
#[test]
fn partial_dispatch_khong_chac_mat_dong_bo_khong_rollback() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // Phím kế: host KhongChac (mô phỏng forward Backspace gửi, commit fail).
    host.ket_qua_ke_tiep = cantype::KetQuaHost::KhongChac;
    host.khong_chac_ap_dung = false; // không áp dụng → text giữ nguyên

    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::MatDongBo);
    // Runtime MatDongBo: không delete, không replay.
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_mat_dong_bo());
}

/// Partial dispatch: host áp dụng một phần (KhongChac + áp dụng). Runtime
/// không delete text cũ dựa state đã lệch.
#[test]
fn partial_dispatch_host_ap_dung_runtime_khong_delete_cu() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // Host KhongChac + áp dụng (mô phỏng host nhận một phần action).
    host.ket_qua_ke_tiep = cantype::KetQuaHost::KhongChac;
    host.khong_chac_ap_dung = true;

    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::MatDongBo);
    // Runtime không xóa dựa state cũ (dù host có áp dụng).
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_mat_dong_bo());
}

// ---------------------------------------------------------------------------
// Two contexts với contract frontend khác nhau.
// ---------------------------------------------------------------------------

/// Context A (Qt, surrounding có) gõ được; context B (LibreOffice, không
/// surrounding) passthrough. A không bị B ảnh hưởng.
#[test]
fn hai_context_contract_khac_doc_lap() {
    let mut phien_a = PhienNhap::moi(ContextId(1));
    let mut phien_b = PhienNhap::moi(ContextId(2));
    let mut host_a = HostMoPhong::moi(ContextId(1));
    let mut host_b = HostMoPhong::moi(ContextId(2));
    host_b.cung_cap_surrounding = false; // B không surrounding

    // A gõ "as" → "á" (Route A).
    go_chuoi(&mut phien_a, &mut host_a, "as");
    assert_eq!(host_a.van_ban, "á");
    assert_eq!(phien_a.da_hien_thi(), "á");

    // B gõ "as" → "a" (phím đầu OK, "s" bị chặn passthrough).
    let kq_b0 = phien_b.xu_ly(&mut host_b, &SuKienNhap::KyTu('a'));
    assert_eq!(kq_b0, KetQuaXuLy::DaApDung);
    let kq_b1 = phien_b.xu_ly(&mut host_b, &SuKienNhap::KyTu('s'));
    assert_eq!(kq_b1, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host_b.van_ban, "a");

    // A vẫn "á", không bị B ảnh hưởng.
    assert_eq!(host_a.van_ban, "á");
    assert_eq!(phien_a.da_hien_thi(), "á");
}

// ---------------------------------------------------------------------------
// Boundary: shortcut, Delete, arrow, Escape không phá composition.
// ---------------------------------------------------------------------------

/// Shortcut (Ctrl+S) passthrough, không vào Cadence, không破坏 composition.
/// (Runtime nhận KyTu chỉ khi không có modifier — adapter lọc trước.)
#[test]
fn shortcut_khong_vao_cadence_khong_pha_composition() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    go_chuoi(&mut phien, &mut host, "a");
    assert_eq!(host.van_ban, "a");

    // Adapter (fcitx5.rs) lọc Ctrl+S ra BoQua trước khi đến PhienNhap.
    // Mô phỏng: không gọi xu_ly cho Ctrl+S. Composition vẫn "a".
    assert_eq!(phien.da_hien_thi(), "a");

    // Phím thường kế tiếp vẫn compose.
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('s'));
    assert_eq!(host.van_ban, "á");
}

/// Delete (forward) relinquish + ChuyểnTiep, không biến thành Backspace.
#[test]
fn delete_relinquish_khong_bien_thanh_backspace() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    let kq = phien.xu_ly(&mut host, &SuKienNhap::DatLai); // Delete → DatLai
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(host.van_ban, "á"); // không xóa
    assert_eq!(phien.da_hien_thi(), "");
}

/// Arrow/Escape relinquish + ChuyểnTiep.
#[test]
fn arrow_escape_relinquish_passthrough() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    let kq = phien.xu_ly(&mut host, &SuKienNhap::DiChuyenConTro);
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    assert_eq!(phien.da_hien_thi(), "");

    // Gõ lại sau arrow: composition mới.
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "áá");
}

// ---------------------------------------------------------------------------
// Undo/redo khi surrounding có: phát hiện qua mismatch.
// ---------------------------------------------------------------------------

/// App undo làm text thay đổi: surrounding không kết thúc da_hien_thi →
/// relinquish + ChuyểnTiep, không delete mù.
#[test]
fn undo_thay_text_surrounding_lech_relinquish() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // App undo → text "á" thành "as" (hoặc rỗng). Mô phỏng surrounding lệch:
    // surrounding trước cursor không kết thúc "á".
    host.van_ban = "as".to_string();
    host.vi_tri_con_tro = 2; // cursor cuối "as"

    let kq = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));
    assert_eq!(kq, KetQuaXuLy::ChuyenTiep);
    // Không delete "as" (surrounding "as" không kết thúc "á").
    assert_eq!(host.van_ban, "as");
    assert_eq!(phien.da_hien_thi(), "");
}
