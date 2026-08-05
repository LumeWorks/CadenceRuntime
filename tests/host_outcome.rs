// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Test outcome host: DaApDung, KhongApDung, KhongChac (§18).

mod common;

use cadence_runtime::{ContextId, HanhDong, KetQuaHost, PhienNhap, SuKienNhap};
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
    // "as" → "á". Sau đó host trả KhongApDung cho 'd'.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));
    go_chuoi(&mut phien, &mut host, "as");
    assert_eq!(phien.da_hien_thi(), "á");

    host.ket_qua_ke_tiep = KetQuaHost::KhongApDung;
    let so_action_truoc = host.lich_su_hanh_dong.len();
    let ket_qua = phien.xu_ly(&mut host, &SuKienNhap::KyTu('d'));

    // Sự kiện không bị nuốt: trả ChuyenTiep (adapter chuyển tiếp phím gốc).
    assert_eq!(ket_qua, cadence_runtime::KetQuaXuLy::ChuyenTiep);
    // State mới KHÔNG được chấp nhận: da_hien_thi vẫn "á".
    assert_eq!(phien.da_hien_thi(), "á");
    // Host không thay đổi văn bản (KhongApDung không áp dụng).
    assert_eq!(host.van_ban, "á");
    // Không có destructive retry: đúng một action thử, không retry tự động.
    assert_eq!(
        host.lich_su_hanh_dong.len(),
        so_action_truoc + 1,
        "khong duoc retry destructive"
    );

    // State cũ vẫn hợp lệ: tiếp tục gõ 'f' (DaApDung) → "à" (Cadence asf → à).
    host.ket_qua_ke_tiep = KetQuaHost::DaApDung;
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
    assert_eq!(ket_qua, cadence_runtime::KetQuaXuLy::MatDongBo);
    // Phiên mất đồng bộ, da_hien_thi rỗng (không sở hữu suffix cũ).
    assert!(phien.dang_mat_dong_bo());
    assert_eq!(phien.da_hien_thi(), "");
    // Host giữ nguyên "á" (KhongChac không áp dụng, text đã commit không mất).

    // Sự kiện kế tiếp KHÔNG delete dựa state cũ: da_hien_thi rỗng nên chỉ Chen.
    host.ket_qua_ke_tiep = KetQuaHost::DaApDung;
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
