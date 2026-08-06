// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Stress test deterministic (§19): hàng chục nghìn sự kiện, không panic, không
//! state leak giữa phiên, action history đúng thứ tự, delete không vượt ownership.
//! Không dùng sleep, không flaky.

mod common;

use cadence_runtime::{ContextId, HanhDong, KetQuaHost, PhienNhap, SuKienNhap};
use common::HostMoPhong;

/// Số sự kiện mỗi phiên stress.
const SO_SU_KIEN: usize = 40_000;

/// Pool ký tự Telex có nghĩa (tạo composition tiếng Việt và raw xen kẽ).
const POOL: &[char] = &['t', 'i', 'e', 'n', 'g', 's', 'a', 'w', 'f', 'd', 'o', 'u'];

#[test]
fn stress_khong_panic_khong_leak_khong_vuot_ownership() {
    let mut phien_a = PhienNhap::moi(ContextId(1));
    let mut host_a = HostMoPhong::moi(ContextId(1));

    let mut tong_action = 0usize;
    let mut tong_thaythe = 0usize;
    let mut tong_chen = 0usize;
    let mut tong_xoa_byte = 0usize;

    for i in 0..SO_SU_KIEN {
        // Capture da_hien_thi trước khi xử lý để kiểm ownership của ThayThe.
        let da_hien_thi_truoc = phien_a.da_hien_thi().to_string();
        let da_hien_thi_len = da_hien_thi_truoc.len();

        // Xen kẽ focus change định kỳ.
        if i % 50 == 49 {
            host_a.the_he_focus += 1;
        }

        let su_kien = match i % 7 {
            0..=4 => SuKienNhap::KyTu(POOL[i % POOL.len()]),
            5 => SuKienNhap::XoaLui,
            _ => SuKienNhap::DatLai,
        };
        phien_a.xu_ly(&mut host_a, &su_kien);

        // Kiểm action mới (nếu có): ThayThe không vượt ownership.
        let so_action = host_a.lich_su_hanh_dong.len();
        if so_action > tong_action {
            // Tất cả action phát sinh từ sự kiện này (tối đa 1).
            let action_moi = &host_a.lich_su_hanh_dong[so_action - 1];
            match action_moi {
                HanhDong::Chen(_) => tong_chen += 1,
                HanhDong::ThayThe(ke) => {
                    tong_thaythe += 1;
                    tong_xoa_byte += ke.xoa_truoc.byte_utf8;
                    assert!(
                        ke.xoa_truoc.byte_utf8 <= da_hien_thi_len,
                        "ThayThe xoa {} vuot da_hien_thi {} tai su kien {i}",
                        ke.xoa_truoc.byte_utf8,
                        da_hien_thi_len
                    );
                }
                HanhDong::ChuyenTiep => {}
            }
            tong_action = so_action;
        }

        // Bất biến đồng bộ: khi DaApDung và dang sở hữu suffix, host text kết
        // thúc bằng da_hien_thi.
        if !phien_a.da_hien_thi().is_empty() {
            assert!(
                host_a.van_ban.ends_with(phien_a.da_hien_thi()),
                "host ({:?}) khong ket thuc bang da_hien_thi ({:?}) tai su kien {i}",
                host_a.van_ban,
                phien_a.da_hien_thi()
            );
        }
    }

    // Action history đúng thứ tự: không giảm.
    assert_eq!(host_a.lich_su_hanh_dong.len(), tong_action);
    // Tối đa một action mỗi sự kiện.
    assert!(tong_action <= SO_SU_KIEN);
    // Có cả Chen lẫn ThayThe (pool tạo composition tiếng Việt).
    assert!(tong_chen > 0, "phai co Chen");
    assert!(tong_thaythe > 0, "phai co ThayThe");
    // Mỗi ThayThe xóa ít nhất 1 byte (nhánh ThayThe chỉ khi xoa_truoc không
    // rỗng) - delete count nhất quán, không âm.
    assert!(
        tong_xoa_byte >= tong_thaythe,
        "tong_xoa_byte ({tong_xoa_byte}) phai >= so ThayThe ({tong_thaythe})"
    );

    // --- Không state leak giữa phiên ---
    // Chạy phien_b độc lập; phien_a không bị thay đổi.
    let da_hien_thi_a = phien_a.da_hien_thi().to_string();
    let van_ban_a = host_a.van_ban.clone();

    let mut phien_b = PhienNhap::moi(ContextId(2));
    let mut host_b = HostMoPhong::moi(ContextId(2));
    for i in 0..SO_SU_KIEN {
        let su_kien = match i % 5 {
            0..=3 => SuKienNhap::KyTu(POOL[(i + 3) % POOL.len()]),
            _ => SuKienNhap::XoaLui,
        };
        phien_b.xu_ly(&mut host_b, &su_kien);
    }

    // phien_a không bị phien_b ảnh hưởng.
    assert_eq!(phien_a.da_hien_thi(), da_hien_thi_a);
    assert_eq!(host_a.van_ban, van_ban_a);
    // phien_b có state riêng, không rỗng (đã gõ).
    assert!(!host_b.van_ban.is_empty());
}

#[test]
fn stress_khong_chac_phuc_hoi_an_toan() {
    // Xen kẽ KhongChac định kỳ; sau mỗi KhongChac, runtime phải phục hồi an toàn
    // và không delete dựa state cũ.
    let mut phien = PhienNhap::moi(ContextId(1));
    let mut host = HostMoPhong::moi(ContextId(1));

    for i in 0..SO_SU_KIEN {
        // Cứ 100 sự kiện, ép host trả KhongChac một lần.
        host.ket_qua_ke_tiep = if i % 100 == 50 {
            KetQuaHost::KhongChac
        } else {
            KetQuaHost::DaPhat
        };
        let su_kien = match i % 6 {
            0..=4 => SuKienNhap::KyTu(POOL[i % POOL.len()]),
            _ => SuKienNhap::XoaLui,
        };
        let ket_qua = phien.xu_ly(&mut host, &su_kien);

        if matches!(ket_qua, cadence_runtime::KetQuaXuLy::MatDongBo) {
            // Sau KhongChac: da_hien_thi rỗng, không sở hữu suffix cũ.
            assert_eq!(phien.da_hien_thi(), "");
            assert!(phien.dang_mat_dong_bo());
        }
        // Không panic sau 40k sự kiện xen kẽ KhongChac.
    }
}
