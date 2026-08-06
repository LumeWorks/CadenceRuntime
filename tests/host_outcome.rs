// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Test outcome host: DaPhat, KhongPhat, KhongChac (§18).

mod common;

use cantype::{ContextId, HanhDong, KetQuaHost, KetQuaXuLy, PhienNhap, SuKienNhap};
use common::HostMoPhong;

fn go_chuoi(phien: &mut PhienNhap, host: &mut HostMoPhong, s: &str) {
    for c in s.chars() {
        phien.xu_ly(host, &SuKienNhap::KyTu(c));
    }
}

#[test]
fn da_ap_dung_state_tien_va_text_khop_rendered() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    // State đã tiến: da_hien_thi khớp rendered Cadence "á".
    assert_eq!(phien.da_hien_thi(), "á");
    // Text host khớp rendered snapshot runtime đã chấp nhận.
    assert_eq!(host.van_ban, "á");
    assert!(host.van_ban.ends_with(phien.da_hien_thi()));
    // Có đúng một action được gửi cho mỗi phím (Chen hoặc ThayThe).
    assert_eq!(host.lich_su_hanh_dong.len(), 2);
}

#[test]
fn khong_ap_dung_khong_chap_nhan_state_moi() {
    // "as" → "á". Sau đó host trả KhongPhat cho 'd'.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(phien.da_hien_thi(), "á");

    host.ket_qua_ke_tiep = KetQuaHost::KhongPhat;
    let so_action_truoc = host.lich_su_hanh_dong.len();
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));

    // Sự kiện không bị nuốt: trả ChuyenTiep (adapter chuyển tiếp phím gốc).
    assert_eq!(ket_qua, cantype::KetQuaXuLy::ChuyenTiep);
    // State mới KHÔNG được chấp nhận: da_hien_thi vẫn "á".
    assert_eq!(phien.da_hien_thi(), "á");
    // Host không thay đổi văn bản (KhongPhat không phát).
    assert_eq!(host.van_ban, "á");
    // Không có destructive retry: đúng một action thử, không retry tự động.
    assert_eq!(
        host.lich_su_hanh_dong.len(),
        so_action_truoc + 1,
        "khong duoc retry destructive"
    );

    // State cũ vẫn hợp lệ: tiếp tục gõ 'f' (DaPhat) → "à" (Cadence asf → à).
    host.ket_qua_ke_tiep = KetQuaHost::DaPhat;
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('f'));
    assert_eq!(host.van_ban, "à");
    assert_eq!(phien.da_hien_thi(), "à");
}

#[test]
fn khong_chac_phien_mat_dong_bo_an_toan() {
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    host.ket_qua_ke_tiep = KetQuaHost::KhongChac;
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));

    // Trả MatDongBo.
    assert_eq!(ket_qua, cantype::KetQuaXuLy::MatDongBo);
    // Phiên mất đồng bộ, da_hien_thi rỗng (không sở hữu suffix cũ).
    assert!(phien.dang_mat_dong_bo());
    assert_eq!(phien.da_hien_thi(), "");
    // Host giữ nguyên "á" (KhongChac không áp dụng, text đã commit không mất).

    // Sự kiện kế tiếp KHÔNG delete dựa state cũ: da_hien_thi rỗng nên chỉ Chen.
    host.ket_qua_ke_tiep = KetQuaHost::DaPhat;
    let so_action_truoc = host.lich_su_hanh_dong.len();
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));

    // Action kế tiếp phải là Chen (không ThayThe xóa "á" cũ).
    let action_moi = &host.lich_su_hanh_dong[so_action_truoc];
    assert!(
        matches!(action_moi, HanhDong::Chen(_)),
        "sau KhongChac phai Chen, khong delete, duoc {action_moi:?}"
    );
    // Văn bản cũ "á" không bị xóa, "a" chèn thêm.
    assert_eq!(host.van_ban, "áa");
    // Runtime đã phục hồi (Rong/DangGo), không còn MatDongBo.
    assert!(!phien.dang_mat_dong_bo());
}

#[test]
fn khong_chac_context_khac_van_hoat_dong_binh_thuong() {
    // Context A gặp KhongChac; context B vẫn gõ bình thường.
    let mut phien_a = PhienNhap::moi(ContextId(1));
    let mut host_a = HostMoPhong::moi(ContextId(1));
    let mut phien_b = PhienNhap::moi(ContextId(2));
    let mut host_b = HostMoPhong::moi(ContextId(2));

    go_chuoi(&mut phien_a, &mut host_a, "as");
    host_a.ket_qua_ke_tiep = KetQuaHost::KhongChac;
    phien_a.xu_ly(&mut host_a, &SuKienNhap::KyTu('d'));
    assert!(phien_a.dang_mat_dong_bo());

    // Context B gõ bình thường, không bị A ảnh hưởng.
    go_chuoi(&mut phien_b, &mut host_b, "as");
    assert_eq!(host_b.van_ban, "á");
    assert_eq!(phien_b.da_hien_thi(), "á");
    assert!(!phien_b.dang_mat_dong_bo());
}

#[test]
fn khong_chac_khong_replay_mou_phim_da_xu_ly() {
    // Sau KhongChac, runtime không replay mù các phím đã xử lý: chỉ có tối đa
    // một action kế tiếp (Chen), không gửi hàng loạt ThayThe/Chen bù.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "tieengs");
    let so_action_truoc_khongchac = host.lich_su_hanh_dong.len();

    host.ket_qua_ke_tiep = KetQuaHost::KhongChac;
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('x'));
    // Một sự kiện KhongChac → đúng một action thử (không replay 7 phím cũ).
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc_khongchac + 1);
    assert!(phien.dang_mat_dong_bo());
    assert_eq!(phien.da_hien_thi(), "");
}

// --- Contract: mỗi input event xuất hiện tối đa một lần trong ứng dụng ---

#[test]
fn khong_chac_tra_mat_dong_bo_khong_chuyen_tiep() {
    // KhongChac → MatDongBo (KHÔNG phải ChuyenTiep).
    // Adapter không được forward phím gốc — host có thể đã áp dụng một phần.
    // Sự kiện xuất hiện tối đa một lần (0 hoặc 1), chuyển tiếp sẽ tạo bản sao.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");

    host.ket_qua_ke_tiep = KetQuaHost::KhongChac;
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));

    // MatDongBo, không phải ChuyenTiep — adapter phải biết không forward.
    assert_eq!(ket_qua, KetQuaXuLy::MatDongBo);
    assert_ne!(ket_qua, KetQuaXuLy::ChuyenTiep);
}

#[test]
fn khong_ap_dung_dung_mot_action_khong_retry_destructive() {
    // KhongPhat → ChuyenTiep (adapter forward), đúng MỘT thuc_thi call
    // (không retry ThayThe). Sự kiện chưa xuất hiện qua action → forward
    // đúng một lần qua phím gốc = tổng đúng một lần.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");

    host.ket_qua_ke_tiep = KetQuaHost::KhongPhat;
    let so_action_truoc = host.lich_su_hanh_dong.len();
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('f'));

    // ChuyenTiep: adapter chuyển tiếp phím gốc.
    assert_eq!(ket_qua, KetQuaXuLy::ChuyenTiep);
    // Đúng một thuc_thi call — không retry destructive.
    assert_eq!(host.lich_su_hanh_dong.len(), so_action_truoc + 1);
}

#[test]
fn da_ap_dung_khong_tra_chuyen_tiep() {
    // DaPhat → DaApDung (KHÔNG phải ChuyenTiep).
    // Adapter không được forward phím gốc — text đã xuất hiện đúng một lần.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));

    assert_eq!(ket_qua, KetQuaXuLy::DaApDung);
    assert_ne!(ket_qua, KetQuaXuLy::ChuyenTiep);
}

// --- Fault injection: KhongChac có thể áp dụng hoặc không ---

#[test]
fn khong_chac_host_ap_dung_thaythe_runtime_khong_delete_cu() {
    // Host áp dụng ThayThe nhưng trả KhongChac — runtime không biết host đã
    // áp dụng. Sự kiện kế tiếp phải Chen (không ThayThe xóa text cũ).
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    // 'f' → Cadence "áf" → "à" (ThayThe: xóa "á", chèn "à").
    host.ket_qua_ke_tiep = KetQuaHost::KhongChac;
    host.khong_chac_ap_dung = true; // Host áp dụng nhưng trả KhongChac.
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('f'));

    // Runtime trả MatDongBo (không biết host đã áp dụng).
    assert_eq!(ket_qua, KetQuaXuLy::MatDongBo);
    // Runtime reset: da_hien_thi rỗng.
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_mat_dong_bo());
    // Host đã áp dụng ThayThe: "á" → "à".
    assert_eq!(host.van_ban, "à");

    // Sự kiện kế tiếp phải Chen (không ThayThe xóa "à" cũ).
    host.ket_qua_ke_tiep = KetQuaHost::DaPhat;
    host.khong_chac_ap_dung = false;
    let so_action = host.lich_su_hanh_dong.len();
    phien.xu_ly(&mut host, &SuKienNhap::KyTu('a'));

    let action_moi = &host.lich_su_hanh_dong[so_action];
    assert!(
        matches!(action_moi, HanhDong::Chen(_)),
        "sau KhongChac phai Chen, khong ThayThe, duoc {action_moi:?}"
    );
    // Text cũ "à" không bị xóa, "a" chèn thêm.
    assert_eq!(host.van_ban, "àa");
}

#[test]
fn khong_chac_khong_ap_dung_text_khong_doi() {
    // KhongChac với khong_chac_ap_dung = false → host không áp dụng.
    // Text host không đổi, runtime reset an toàn.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(host.van_ban, "á");

    host.ket_qua_ke_tiep = KetQuaHost::KhongChac;
    host.khong_chac_ap_dung = false; // Host không áp dụng.
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('f'));

    assert_eq!(ket_qua, KetQuaXuLy::MatDongBo);
    // Host text không đổi (không áp dụng action).
    assert_eq!(host.van_ban, "á");
    assert_eq!(phien.da_hien_thi(), "");
    assert!(phien.dang_mat_dong_bo());
}
