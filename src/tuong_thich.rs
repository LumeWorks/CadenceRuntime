// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Tương thích frontend Phase 3 — phân loại frontend Fcitx5 và diagnostic
//! logging KHÔNG chứa text user.
//!
//! Phase 3 khảo sát các frontend Fcitx5 (dbus, wayland, wayland_v2, xim, ibus,
//! fcitx4) để lập capability matrix và quyết định route. Module này chứa:
//!
//! * [`Frontend`] — phân loại frontend từ `ic->frontend()` (const char*).
//! * [`ChanDoan`] — bản trace metadata KHÔNG chứa text user (chỉ surrounding
//!   length, suffix match boolean, capability flags, cursor/anchor offset).
//! * [`ghi_chan_doan`] — ghi trace ra stderr, chỉ compile khi feature `diag`
//!   bật, và chỉ chạy khi env `CANTYPE_DEBUG` set. Production build (không
//!   feature `diag`) không có code diagnostic → zero cost trên hot path.
//!
//! Bất biến quyền riêng tư (§12, §56): KHÔNG bao giờ log raw user input,
//! surrounding text, clipboard, document text, password, hay full typed word.
//! Chỉ log surrounding **length**, suffix match **boolean**, delete/insert
//! **length**, capability flags, cursor/anchor offset (không phải giá trị text).
//!
//! Module luôn compile (thuần Rust) để test classification chạy không cần
//! Fcitx5 dev. `ghi_chan_doan` chỉ được `ffi` gọi khi feature `diag` VÀ
//! `fcitx5` đều bật; khi chỉ `diag` (không `fcitx5`) hoặc không `diag`, các
//! kiểu trace không được dùng → `allow(dead_code)`.

#![cfg_attr(not(all(feature = "diag", feature = "fcitx5")), allow(dead_code))]

#[cfg(feature = "diag")]
use std::sync::OnceLock;

/// Phân loại frontend Fcitx5, từ `ic->frontend()` (const char*).
///
/// Giá trị thật (Fcitx 5.1.12, src/frontend/): `"dbus"` (GTK/Qt/Electron qua
/// fcitx5-gtk/fcitx5-qt IM module), `"wayland"` (text-input-v1), `"wayland_v2"`
/// (text-input-v2), `"xim"` (X11 raw XIM, no surrounding), `"ibus"` (IBus
/// protocol), `"fcitx4"` (legacy). Không đoán theo app name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Frontend {
    /// D-Bus frontend — GTK/Qt/Electron qua fcitx5-gtk/fcitx5-qt IM module.
    Dbus,
    /// Wayland text-input v1.
    Wayland,
    /// Wayland text-input v2.
    WaylandV2,
    /// XIM — X11 raw, không surrounding text.
    Xim,
    /// IBus protocol frontend.
    Ibus,
    /// Legacy fcitx4 D-Bus frontend.
    Fcitx4,
    /// Frontend không nhận diện được.
    Khac,
}

impl Frontend {
    /// Tên ngắn gọn cho trace (không chứa thông tin nhạy cảm).
    #[must_use]
    pub(crate) fn ten(self) -> &'static str {
        match self {
            Self::Dbus => "dbus",
            Self::Wayland => "wayland",
            Self::WaylandV2 => "wayland_v2",
            Self::Xim => "xim",
            Self::Ibus => "ibus",
            Self::Fcitx4 => "fcitx4",
            Self::Khac => "khac",
        }
    }
}

/// Phân loại frontend từ chuỗi `ic->frontend()`. Trailing NUL không được tính.
///
/// Trả [`Frontend::Khac`] cho chuỗi rỗng hoặc không nhận diện. Không panic,
/// không đoán theo app name.
#[must_use]
pub(crate) fn phan_loai(frontend: &str) -> Frontend {
    match frontend {
        "dbus" => Frontend::Dbus,
        "wayland" => Frontend::Wayland,
        "wayland_v2" => Frontend::WaylandV2,
        "xim" => Frontend::Xim,
        "ibus" => Frontend::Ibus,
        "fcitx4" => Frontend::Fcitx4,
        _ => Frontend::Khac,
    }
}

/// Bitmask [`CapabilityFlag`] quan tâm Phase 3 (Fcitx 5.1.12,
/// fcitx-utils/capabilityflags.h).
pub(crate) mod co_cap {
    /// `SurroundingText = (1 << 6)` — app báo hỗ trợ surrounding text.
    pub(crate) const SURROUNDING: u64 = 1 << 6;
    /// `Password = (1 << 3)` — trường password.
    pub(crate) const PASSWORD: u64 = 1 << 3;
    /// `Terminal = (1ULL << 32)` — terminal emulator.
    pub(crate) const TERMINAL: u64 = 1 << 32;
    /// `Sensitive = (1ULL << 36)` — trường nhạy cảm.
    pub(crate) const SENSITIVE: u64 = 1 << 36;
    /// `Preedit = (1 << 1)` — app hỗ trợ preedit (CanType không dùng, chỉ trace).
    pub(crate) const PREDIT: u64 = 1 << 1;
}

/// Bản trace diagnostic metadata, KHÔNG chứa text user.
///
/// Chỉ chứa: context id, frontend, program (tên binary app — không phải text
/// user gõ), capability flags, surrounding validity/length, cursor/anchor
/// offset (số, không phải giá trị text), route, action, outcome. Theo §56.
#[derive(Debug, Clone)]
pub(crate) struct ChanDoan {
    /// Context id.
    pub context_id: u64,
    /// Frontend đã phân loại.
    pub frontend: Frontend,
    /// Tên binary app (`ic->program()`). Metadata, không phải text user.
    pub program: String,
    /// `true` nếu capability có SurroundingText.
    pub co_surrounding: bool,
    /// `true` nếu surrounding text hợp lệ (`isValid()`).
    pub surrounding_hop_le: bool,
    /// `true` nếu có focus.
    pub co_focus: bool,
    /// Thế hệ focus.
    pub focus_generation: u64,
    /// `true` nếu capability báo Password.
    pub co_password: bool,
    /// `true` nếu capability báo Terminal.
    pub co_terminal: bool,
    /// `true` nếu capability báo Sensitive.
    pub co_sensitive: bool,
    /// `true` nếu capability báo Preedit (CanType không dùng preedit, chỉ trace).
    pub co_preedit: bool,
    /// Độ dài surrounding text theo byte (KHÔNG phải giá trị text).
    pub surrounding_do_dai: usize,
    /// Offset con trỏ (ký tự, không phải giá trị text).
    pub cursor: u32,
    /// Offset anchor (ký tự). `cursor != anchor` → có selection.
    pub anchor: u32,
    /// Route đã chọn: `"native"`, `"passthrough"`, `"mat_dong_bo"`.
    pub route: &'static str,
    /// Hành động: `"chen"`, `"thay_the"`, `"chuyen_tiep"`, `"dat_lai"`, ...
    pub hanh_dong: &'static str,
    /// Kết quả: `"da_ap_dung"`, `"chuyen_tiep"`, `"mat_dong_bo"`.
    pub ket_qua: &'static str,
    /// Độ dài suffix runtime sở hữu (byte) tại lúc trace.
    pub da_hien_thi_do_dai: usize,
    /// `true` nếu surrounding kết thúc bằng `da_hien_thi` (suffix khớp).
    pub suffix_khop: bool,
}

impl ChanDoan {
    /// Trả `true` nếu cursor ≠ anchor (có selection).
    #[must_use]
    pub(crate) fn co_selection(&self) -> bool {
        self.cursor != self.anchor
    }
}

/// Đọc env `CANTYPE_DEBUG` một lần (OnceLock). `true` nếu set (bất kỳ giá trị
/// non-empty). Diagnostic chỉ chạy khi cả feature `diag` bật AND env set.
#[cfg(feature = "diag")]
fn chan_doan_bat() -> bool {
    static BAT: OnceLock<bool> = OnceLock::new();
    *BAT.get_or_init(|| std::env::var_os("CANTYPE_DEBUG").is_some())
}

/// Ghi bản trace diagnostic ra stderr. Chỉ compile khi feature `diag` bật.
///
/// Format một dòng KV (§56). KHÔNG log text user — chỉ surrounding length,
/// suffix match boolean, capability flags, cursor/anchor offset. Khi env
/// `CANTYPE_DEBUG` không set, no-op (một lần đọc OnceLock, sau đó early
/// return).
#[cfg(feature = "diag")]
pub(crate) fn ghi_chan_doan(cd: &ChanDoan) {
    if !chan_doan_bat() {
        return;
    }
    eprintln!(
        "cantype diag ctx={ctx} frontend={frontend}{program} focus_gen={gen} \
         focus={focus} sur_cap={sur_cap} sur_valid={sur_valid} sur_len={sur_len} \
         cursor={cursor} anchor={anchor} sel={sel} \
         pw={pw} term={term} sens={sens} preedit={preedit} \
         route={route} action={action} outcome={outcome} \
         suffix_len={suffix_len} suffix_match={suffix_match}",
        ctx = cd.context_id,
        frontend = cd.frontend.ten(),
        program = if cd.program.is_empty() {
            String::new()
        } else {
            format!(" program={}", cd.program)
        },
        gen = cd.focus_generation,
        focus = u8::from(cd.co_focus),
        sur_cap = u8::from(cd.co_surrounding),
        sur_valid = u8::from(cd.surrounding_hop_le),
        sur_len = cd.surrounding_do_dai,
        cursor = cd.cursor,
        anchor = cd.anchor,
        sel = u8::from(cd.co_selection()),
        pw = u8::from(cd.co_password),
        term = u8::from(cd.co_terminal),
        sens = u8::from(cd.co_sensitive),
        preedit = u8::from(cd.co_preedit),
        route = cd.route,
        action = cd.hanh_dong,
        outcome = cd.ket_qua,
        suffix_len = cd.da_hien_thi_do_dai,
        suffix_match = u8::from(cd.suffix_khop),
    );
}

/// No-op khi feature `diag` tắt (production). Zero cost: hàm rỗng inline.
#[cfg(not(feature = "diag"))]
#[inline]
pub(crate) fn ghi_chan_doan(_cd: &ChanDoan) {}

#[cfg(test)]
mod tests {
    //! Test phân loại frontend (thuần Rust, không cần Fcitx5 dev).

    use super::*;

    #[test]
    fn phan_loai_dbus() {
        assert_eq!(phan_loai("dbus"), Frontend::Dbus);
    }

    #[test]
    fn phan_loai_wayland_v2() {
        assert_eq!(phan_loai("wayland_v2"), Frontend::WaylandV2);
    }

    #[test]
    fn phan_loai_xim() {
        assert_eq!(phan_loai("xim"), Frontend::Xim);
    }

    #[test]
    fn phan_loai_khac_cho_chuoi_la() {
        assert_eq!(phan_loai("fcitx5-qt"), Frontend::Khac);
        assert_eq!(phan_loai(""), Frontend::Khac);
        assert_eq!(phan_loai("unknown"), Frontend::Khac);
    }

    #[test]
    fn co_cap_bitmask_dung() {
        // Pin giá trị bitmask theo Fcitx 5.1.12 header (regression nếu đổi).
        assert_eq!(co_cap::SURROUNDING, 1 << 6);
        assert_eq!(co_cap::PASSWORD, 1 << 3);
        assert_eq!(co_cap::TERMINAL, 1 << 32);
        assert_eq!(co_cap::SENSITIVE, 1 << 36);
        assert_eq!(co_cap::PREDIT, 1 << 1);
    }

    #[test]
    fn ten_frontend_ngan_gon() {
        assert_eq!(Frontend::Dbus.ten(), "dbus");
        assert_eq!(Frontend::WaylandV2.ten(), "wayland_v2");
        assert_eq!(Frontend::Khac.ten(), "khac");
    }

    #[test]
    fn co_selection_khi_cursor_khac_anchor() {
        let cd = ChanDoan {
            context_id: 1,
            frontend: Frontend::Dbus,
            program: String::new(),
            co_surrounding: true,
            surrounding_hop_le: true,
            co_focus: true,
            focus_generation: 1,
            co_password: false,
            co_terminal: false,
            co_sensitive: false,
            co_preedit: false,
            surrounding_do_dai: 5,
            cursor: 3,
            anchor: 1,
            route: "native",
            hanh_dong: "thay_the",
            ket_qua: "da_ap_dung",
            da_hien_thi_do_dai: 2,
            suffix_khop: true,
        };
        assert!(cd.co_selection());
    }

    #[test]
    fn khong_selection_khi_cursor_bang_anchor() {
        let cd = ChanDoan {
            context_id: 1,
            frontend: Frontend::Dbus,
            program: String::new(),
            co_surrounding: true,
            surrounding_hop_le: true,
            co_focus: true,
            focus_generation: 1,
            co_password: false,
            co_terminal: false,
            co_sensitive: false,
            co_preedit: false,
            surrounding_do_dai: 5,
            cursor: 3,
            anchor: 3,
            route: "native",
            hanh_dong: "thay_the",
            ket_qua: "da_ap_dung",
            da_hien_thi_do_dai: 2,
            suffix_khop: true,
        };
        assert!(!cd.co_selection());
    }
}
