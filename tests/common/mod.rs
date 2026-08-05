// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Host mô phỏng dùng chung cho test runtime (§13).
//!
//! Mô phỏng editor thật: giữ văn bản và con trỏ, cập nhật text khi action được
//! áp dụng, và ghi lại lịch sử action để test kiểm chứng. Không chỉ ghi log -
//! sau mỗi action thành công, text trong host phải khớp rendered snapshot runtime
//! đã chấp nhận.

use cadence_runtime::{BoiCanhNhap, ContextId, HanhDong, Host, KetQuaHost};

/// Host mô phỏng editor cho test.
pub struct HostMoPhong {
    /// Context của host.
    pub context_id: ContextId,
    /// Thế hệ focus hiện tại.
    pub the_he_focus: u64,
    /// `true` nếu đang có focus.
    pub dang_co_focus: bool,
    /// Văn bản đầy đủ trong editor.
    pub van_ban: String,
    /// Vị trí con trỏ theo byte UTF-8.
    pub vi_tri_con_tro: usize,
    /// Kết quả `thuc_thi` kế tiếp sẽ trả.
    pub ket_qua_ke_tiep: KetQuaHost,
    /// Lịch sử action đã gửi (kể cả khi không áp dụng).
    pub lich_su_hanh_dong: Vec<HanhDong>,
}

impl HostMoPhong {
    /// Tạo host rỗng với context và focus mặc định.
    pub fn moi(context_id: ContextId) -> Self {
        Self {
            context_id,
            the_he_focus: 1,
            dang_co_focus: true,
            van_ban: String::new(),
            vi_tri_con_tro: 0,
            ket_qua_ke_tiep: KetQuaHost::DaApDung,
            lich_su_hanh_dong: Vec::new(),
        }
    }

    /// Áp dụng action lên văn bản (chỉ khi DaApDung).
    fn ap_dung(&mut self, hanh_dong: &HanhDong) {
        match hanh_dong {
            HanhDong::Chen(s) => {
                self.van_ban.insert_str(self.vi_tri_con_tro, s);
                self.vi_tri_con_tro += s.len();
            }
            HanhDong::ThayThe(ke) => {
                let xoa = ke.xoa_truoc.byte_utf8;
                let bat_dau = self.vi_tri_con_tro - xoa;
                self.van_ban
                    .replace_range(bat_dau..self.vi_tri_con_tro, &ke.chen);
                self.vi_tri_con_tro = bat_dau + ke.chen.len();
            }
            HanhDong::ChuyenTiep => {}
        }
    }
}

impl Host for HostMoPhong {
    fn boi_canh(&self) -> BoiCanhNhap {
        let van_ban_truoc = self.van_ban[..self.vi_tri_con_tro].to_string();
        BoiCanhNhap {
            context_id: self.context_id,
            the_he_focus: self.the_he_focus,
            dang_co_focus: self.dang_co_focus,
            van_ban_truoc_con_tro: Some(van_ban_truoc),
        }
    }

    fn thuc_thi(&mut self, hanh_dong: &HanhDong) -> KetQuaHost {
        let ket_qua = self.ket_qua_ke_tiep;
        self.lich_su_hanh_dong.push(hanh_dong.clone());
        if matches!(ket_qua, KetQuaHost::DaApDung) {
            self.ap_dung(hanh_dong);
        }
        ket_qua
    }
}
