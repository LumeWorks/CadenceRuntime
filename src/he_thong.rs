// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Trạng thái tích hợp Fcitx5 — thu thập thông tin khi GUI mở hoặc user bấm
//! "Kiểm tra lại".
//!
//! Không chạy process mỗi frame. Không chạy lệnh trên mỗi key. Dùng
//! `std::process::Command` với argument riêng (không shell string ghép từ
//! input user). Kết quả là enum/model rõ.

use std::path::PathBuf;
use std::process::Command;

/// Trạng thái tích hợp addon CanType với Fcitx5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrangThaiTichHop {
    /// Addon đã nạp, input method đang hoạt động.
    HoatDong,
    /// Fcitx5 chạy nhưng addon chưa nạp.
    ChuaNap,
    /// Addon thiếu (file .so hoặc metadata không tìm thấy).
    ThieuAddon,
    /// Fcitx5 không tìm thấy (chưa cài hoặc chưa chạy).
    KhongTimThayFcitx,
    /// Fcitx5 chạy nhưng cần restart để nạp addon mới.
    CanKhoiDongLai,
    /// Không xác định được trạng thái.
    KhongXacDinh,
}

/// Thông tin hệ thống thu thập được. Tất cả field là `String`/`Option` để GUI
/// hiển thị trực tiếp, không log raw pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThongTinHeThong {
    /// Phiên làm việc (desktop + session type).
    pub phien_lam_viec: String,
    /// Framework nhập liệu (Fcitx5/IBus/không).
    pub framework_nhap_lieu: String,
    /// Trạng thái addon CanType.
    pub trang_thai_addon: String,
    /// Trạng thái input method.
    pub trang_thai_im: String,
    /// Chế độ cài đặt (Development/System).
    pub che_do_cai_dat: String,
    /// Phiên bản CanType (từ Cargo.toml).
    pub phien_ban_cantype: String,
    /// Phiên bản Cadence.
    pub phien_ban_cadence: String,
    /// SHA Cadence (short).
    pub sha_cadence: String,
    /// Phiên bản Fcitx5.
    pub phien_ban_fcitx: Option<String>,
    /// Đường dẫn .so addon (nếu tìm thấy).
    pub duong_dan_addon: Option<String>,
    /// FCITX_ADDON_DIRS (nếu set).
    pub fcitx_addon_dirs: Option<String>,
    /// Trạng thái tích hợp tổng quát.
    pub trang_thai: TrangThaiTichHop,
    /// Mô tả trạng thái tổng quát bằng tiếng người.
    pub trang_thai_tong_quan: String,
}

/// Phiên bản CanType (từ Cargo.toml tại build time).
pub const PHIEN_BAN_CANTYPE: &str = env!("CARGO_PKG_VERSION");

/// Phiên bản Cadence (hardcode từ Cargo.toml rev pin, tránh parse git).
pub const PHIEN_BAN_CADENCE: &str = "v2026.1.0";

/// Short SHA Cadence (peeled commit, 7 ký tự đầu).
pub const SHA_CADENCE: &str = "a5a5863";

/// Thu thập thông tin hệ thống. Chỉ gọi khi GUI mở hoặc user bấm "Kiểm tra lại".
///
/// Hàm này chạy `fcitx5 --version` và kiểm tra file addon — không chạy trên
/// mỗi key, không chạy mỗi frame.
#[must_use]
pub fn thu_thap() -> ThongTinHeThong {
    let phien_lam_viec = doc_phien_lam_viec();
    let phien_ban_fcitx = doc_phien_ban_fcitx();
    let duong_dan_addon = tim_duong_dan_addon();
    let fcitx_addon_dirs = std::env::var("FCITX_ADDON_DIRS").ok();
    let trang_thai = tinh_trang_thai(&phien_ban_fcitx, &duong_dan_addon, &fcitx_addon_dirs);
    let trang_thai_tong_quan = mo_ta_trang_thai(trang_thai);

    ThongTinHeThong {
        phien_lam_viec,
        framework_nhap_lieu: "Fcitx5".to_string(),
        trang_thai_addon: mo_ta_trang_thai_addon(trang_thai),
        trang_thai_im: mo_ta_trang_thai_im(trang_thai),
        che_do_cai_dat: xac_dinh_che_do_cai_dat(&fcitx_addon_dirs),
        phien_ban_cantype: PHIEN_BAN_CANTYPE.to_string(),
        phien_ban_cadence: PHIEN_BAN_CADENCE.to_string(),
        sha_cadence: SHA_CADENCE.to_string(),
        phien_ban_fcitx,
        duong_dan_addon,
        fcitx_addon_dirs,
        trang_thai,
        trang_thai_tong_quan,
    }
}

/// Đọc phiên làm việc: desktop + session type từ biến môi trường.
fn doc_phien_lam_viec() -> String {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "Không xác định".into());
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "không xác định".into());
    format!("{desktop} · {session}")
}

/// Đọc phiên bản Fcitx5 qua `fcitx5 --version`. Trả `None` nếu không tìm thấy.
fn doc_phien_ban_fcitx() -> Option<String> {
    let output = Command::new("fcitx5").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `fcitx5 --version` in "fcitx5 version 5.1.12" hoặc tương tự.
    let line = stdout.lines().next()?;
    Some(line.trim().to_string())
}

/// Tìm đường dẫn .so addon CanType. Kiểm tra FCITX_ADDON_DIRS và đường dẫn
/// mặc định.
fn tim_duong_dan_addon() -> Option<String> {
    // FCITX_ADDON_DIRS (developer mode).
    if let Ok(dirs) = std::env::var("FCITX_ADDON_DIRS") {
        for dir in dirs.split(':') {
            let p = PathBuf::from(dir).join("libcantype.so");
            if p.exists() {
                return Some(p.display().to_string());
            }
        }
    }
    // Đường dẫn mặc định user.
    let home = std::env::var("HOME").ok()?;
    let user_dir = PathBuf::from(&home).join(".local/lib/fcitx5/libcantype.so");
    if user_dir.exists() {
        return Some(user_dir.display().to_string());
    }
    // Đường dẫn system (đoán theo architecture).
    let sys_dir = PathBuf::from("/usr/lib/x86_64-linux-gnu/fcitx5/libcantype.so");
    if sys_dir.exists() {
        return Some(sys_dir.display().to_string());
    }
    None
}

/// Tính trạng thái tích hợp từ thông tin có được.
fn tinh_trang_thai(
    phien_ban_fcitx: &Option<String>,
    duong_dan_addon: &Option<String>,
    _fcitx_addon_dirs: &Option<String>,
) -> TrangThaiTichHop {
    if phien_ban_fcitx.is_none() {
        return TrangThaiTichHop::KhongTimThayFcitx;
    }
    if duong_dan_addon.is_none() {
        return TrangThaiTichHop::ThieuAddon;
    }
    // Không thể biết chắc addon đã nạp hay chưa từ Rust (không có API Fcitx5
    // từ GUI process). Dùng `fcitx5-remote` để kiểm tra IM active, nhưng đó là
    // heuristic. Phase 2: nếu .so tồn tại + Fcitx5 chạy → giả định HoatDong.
    TrangThaiTichHop::HoatDong
}

/// Mô tả trạng thái tổng quát bằng tiếng người.
fn mo_ta_trang_thai(t: TrangThaiTichHop) -> String {
    match t {
        TrangThaiTichHop::HoatDong => "CanType đang hoạt động bình thường".to_string(),
        TrangThaiTichHop::ChuaNap => "Addon chưa nạp — restart Fcitx5".to_string(),
        TrangThaiTichHop::ThieuAddon => "Thiếu addon libcantype.so".to_string(),
        TrangThaiTichHop::KhongTimThayFcitx => "Không tìm thấy Fcitx5".to_string(),
        TrangThaiTichHop::CanKhoiDongLai => "Cần restart Fcitx5 để nạp addon".to_string(),
        TrangThaiTichHop::KhongXacDinh => "Không xác định được trạng thái".to_string(),
    }
}

/// Mô tả trạng thái addon.
fn mo_ta_trang_thai_addon(t: TrangThaiTichHop) -> String {
    match t {
        TrangThaiTichHop::HoatDong | TrangThaiTichHop::ChuaNap => "Đã nạp".to_string(),
        TrangThaiTichHop::ThieuAddon => "Thiếu".to_string(),
        TrangThaiTichHop::KhongTimThayFcitx => "Không xác định".to_string(),
        TrangThaiTichHop::CanKhoiDongLai => "Cần reload".to_string(),
        TrangThaiTichHop::KhongXacDinh => "Không xác định".to_string(),
    }
}

/// Mô tả trạng thái IM.
fn mo_ta_trang_thai_im(t: TrangThaiTichHop) -> String {
    match t {
        TrangThaiTichHop::HoatDong => "Đang hoạt động".to_string(),
        TrangThaiTichHop::ChuaNap => "Chưa nạp".to_string(),
        TrangThaiTichHop::ThieuAddon => "Không khả dụng".to_string(),
        TrangThaiTichHop::KhongTimThayFcitx => "Không tìm thấy Fcitx5".to_string(),
        TrangThaiTichHop::CanKhoiDongLai => "Cần restart".to_string(),
        TrangThaiTichHop::KhongXacDinh => "Không xác định".to_string(),
    }
}

/// Xác định chế độ cài đặt: Development nếu FCITX_ADDON_DIRS set, ngược lại
/// System.
fn xac_dinh_che_do_cai_dat(fcitx_addon_dirs: &Option<String>) -> String {
    if fcitx_addon_dirs.is_some() {
        "Development".to_string()
    } else {
        "System".to_string()
    }
}
