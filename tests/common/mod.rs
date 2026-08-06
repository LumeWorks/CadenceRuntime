// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Host mô phỏng dùng chung cho test runtime (§13).
//!
//! Mô phỏng editor thật: giữ văn bản và con trỏ, cập nhật text khi action được
//! áp dụng, và ghi lại lịch sử action để test kiểm chứng. Không chỉ ghi log -
//! sau mỗi action thành công, text trong host phải khớp rendered snapshot runtime
//! đã chấp nhận.
//!
//! Field là `pub` để test chỉnh trạng thái trực tiếp (focus, cursor, văn bản
//! ngoài runtime). Đây là test fixture, không phải public API của crate.

use cantype::{BoiCanhNhap, ContextId, HanhDong, Host, KetQuaHost};

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
    /// `true` nếu host cung cấp surrounding text; `false` cho terminal/game.
    pub cung_cap_surrounding: bool,
    /// Khi `KhongChac`, `true` để áp dụng action nhưng vẫn trả `KhongChac`
    /// (mô phỏng host nhận action nhưng runtime không biết). `false` để không
    /// áp dụng gì (mô phỏng host chưa nhận).
    pub khong_chac_ap_dung: bool,
}

impl HostMoPhong {
    /// Tạo host rỗng với context và focus mặc định.
    #[must_use]
    pub fn moi(context_id: ContextId) -> Self {
        Self {
            context_id,
            the_he_focus: 1,
            dang_co_focus: true,
            van_ban: String::new(),
            vi_tri_con_tro: 0,
            ket_qua_ke_tiep: KetQuaHost::DaPhat,
            lich_su_hanh_dong: Vec::new(),
            cung_cap_surrounding: true,
            khong_chac_ap_dung: false,
        }
    }

    /// Áp dụng action lên văn bản (chỉ khi DaPhat hoặc KhongChac có flag).
    fn ap_dung(&mut self, hanh_dong: &HanhDong) {
        match hanh_dong {
            HanhDong::Chen(s) => {
                self.van_ban.insert_str(self.vi_tri_con_tro, s);
                self.vi_tri_con_tro += s.len();
            }
            HanhDong::ThayThe(ke) => {
                let xoa = ke.xoa_truoc.byte_utf8;
                // Saturating_sub để không underflow khi xoa > vi_tri_con_tro;
                // runtime không nên gửi ThayThe vượt cursor, nhưng host phòng thủ.
                let bat_dau = self.vi_tri_con_tro.saturating_sub(xoa);
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
        let van_ban_truoc = if self.cung_cap_surrounding {
            Some(self.van_ban[..self.vi_tri_con_tro].to_string())
        } else {
            None
        };
        BoiCanhNhap {
            context_id: self.context_id,
            the_he_focus: self.the_he_focus,
            dang_co_focus: self.dang_co_focus,
            van_ban_truoc_con_tro: van_ban_truoc,
        }
    }

    fn thuc_thi(&mut self, hanh_dong: &HanhDong) -> KetQuaHost {
        let ket_qua = self.ket_qua_ke_tiep;
        self.lich_su_hanh_dong.push(hanh_dong.clone());
        match ket_qua {
            KetQuaHost::DaPhat => self.ap_dung(hanh_dong),
            KetQuaHost::KhongPhat => {}
            KetQuaHost::KhongChac => {
                if self.khong_chac_ap_dung {
                    self.ap_dung(hanh_dong);
                }
            }
        }
        ket_qua
    }
}
