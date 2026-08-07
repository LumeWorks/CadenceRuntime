// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Logic GUI CanType: đọc/ghi cấu hình, bind vào cửa sổ Slint + tray, xử lý
//! callbacks.
//!
//! Không sở hữu Rust sessions của Fcitx5; GUI và addon chỉ dùng chung schema
//! cấu hình (`cau_hinh`). Khi user đổi setting, GUI ghi atomically; addon đọc
//! lại ở focus cycle/context tiếp theo (không realtime).

// Code Slint generate (`slint::include_modules!`) không có doc comments và dùng
// `unwrap()` nội bộ; allow ở module này (không ảnh hưởng phần còn lại của lib).
#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use crate::cau_hinh::{
    self, CauHinhCanType, ChinhSach, DangUnicode, KieuGo, KieuTelex, QuyTacDatDau,
};

slint::include_modules!();

/// Chạy GUI CanType: cửa sổ chính + tray icon. Đọc config, bind, xử lý
/// callbacks. Trả lỗi platform nếu Slint không khởi tạo được.
///
/// # Errors
/// Trả `slint::PlatformError` nếu backend Slint không khởi tạo được.
pub fn chay() -> Result<(), slint::PlatformError> {
    let cfg = doc_config();

    // Cửa sổ chính.
    let app = App::new()?;
    bind_config(&app, &cfg);

    // Thu thập trạng thái hệ thống (chỉ 1 lần khi GUI mở, không mỗi frame).
    let thong_tin = crate::he_thong::thu_thap();
    app.set_phien_lam_viec(thong_tin.phien_lam_viec.into());
    app.set_trang_thai_addon(thong_tin.trang_thai_addon.into());
    app.set_trang_thai_framework(thong_tin.trang_thai_framework.into());
    app.set_phien_ban_cantype(thong_tin.phien_ban_cantype.into());
    app.set_phien_ban_cadence(thong_tin.phien_ban_cadence.into());
    app.set_trang_thai_tong_quan(thong_tin.trang_thai_tong_quan.into());
    app.set_khoe_manh(thong_tin.trang_thai == crate::he_thong::TrangThaiTichHop::HoatDong);

    // Tray icon.
    let tray = TrayCanType::new()?;
    tray.set_dang_bat(cfg.dang_bat);
    tray.set_kieu_go(match cfg.kieu_go {
        KieuGo::Telex => "telex".into(),
        KieuGo::Vni => "vni".into(),
    });

    // --- Callbacks cửa sổ ---
    let app_luu = app.as_weak();
    let tray_luu = tray.as_weak();
    app.on_luu(move || {
        let app = app_luu.unwrap();
        let tray = tray_luu.unwrap();
        let cfg = lay_config_tu_ui(&app);
        if let Some(duong_dan) = cau_hinh::duong_dan_config()
            && let Err(e) = cau_hinh::ghi(&duong_dan, &cfg)
        {
            eprintln!("CanType: loi ghi config: {e}");
        }
        // Sync tray state với window.
        tray.set_dang_bat(cfg.dang_bat);
        tray.set_kieu_go(match cfg.kieu_go {
            KieuGo::Telex => "telex".into(),
            KieuGo::Vni => "vni".into(),
        });
    });

    let app_mac_dinh = app.as_weak();
    app.on_mac_dinh(move || {
        let app = app_mac_dinh.unwrap();
        let mac_dinh = CauHinhCanType::default();
        bind_config(&app, &mac_dinh);
    });

    let app_dong = app.as_weak();
    app.on_dong(move || {
        let app = app_dong.unwrap();
        let _ = app.window().hide();
        // Không quit: đóng cửa sổ chỉ ẩn xuống tray.
    });

    // --- Callbacks tray ---
    // Mở CanType (từ menu) → reset về tab Cơ bản + show + un-minimize.
    // (Slint không expose set_focus public; dùng show + set_minimized(false).)
    let app_mo = app.as_weak();
    tray.on_mo_cua_so(move || {
        let app = app_mo.unwrap();
        app.set_tab_hien_tai(0);
        let _ = app.window().show();
        app.window().set_minimized(false);
    });

    // Đổi tiếng Việt/Anh từ tray → update window + tray + ghi config.
    let app_tv = app.as_weak();
    let tray_tv = tray.as_weak();
    tray.on_dat_tieng_viet(move |bat| {
        let app = app_tv.unwrap();
        let tray = tray_tv.unwrap();
        let mut cfg = lay_config_tu_ui(&app);
        cfg.dang_bat = bat;
        app.set_dang_bat(cfg.dang_bat);
        tray.set_dang_bat(cfg.dang_bat);
        if let Some(duong_dan) = cau_hinh::duong_dan_config()
            && let Err(e) = cau_hinh::ghi(&duong_dan, &cfg)
        {
            eprintln!("CanType: loi ghi config: {e}");
        }
    });

    // Đổi kiểu gõ từ tray → update window + ghi config.
    let app_kg = app.as_weak();
    let tray_kg = tray.as_weak();
    tray.on_dat_kieu_go(move |kieu| {
        let app = app_kg.unwrap();
        let tray = tray_kg.unwrap();
        let mut cfg = lay_config_tu_ui(&app);
        cfg.kieu_go = if kieu.as_str() == "vni" {
            KieuGo::Vni
        } else {
            KieuGo::Telex
        };
        app.set_kieu_go(kieu.clone());
        tray.set_kieu_go(kieu);
        if let Some(duong_dan) = cau_hinh::duong_dan_config()
            && let Err(e) = cau_hinh::ghi(&duong_dan, &cfg)
        {
            eprintln!("CanType: loi ghi config: {e}");
        }
    });

    // Thoát GUI → quit event loop. Addon Fcitx5 vẫn tiếp tục chạy riêng.
    tray.on_thoat_giao_dien(move || {
        slint::quit_event_loop().ok();
    });

    // Kiểm tra lại trạng thái hệ thống (tab Hệ thống).
    let app_kt = app.as_weak();
    app.on_kiem_tra_lai(move || {
        let app = app_kt.unwrap();
        let thong_tin = crate::he_thong::thu_thap();
        app.set_phien_lam_viec(thong_tin.phien_lam_viec.into());
        app.set_trang_thai_addon(thong_tin.trang_thai_addon.into());
        app.set_trang_thai_framework(thong_tin.trang_thai_framework.into());
        app.set_trang_thai_tong_quan(thong_tin.trang_thai_tong_quan.into());
        app.set_khoe_manh(thong_tin.trang_thai == crate::he_thong::TrangThaiTichHop::HoatDong);
    });

    tray.show()?;
    app.run()
}

/// Đọc config từ đường dẫn mặc định. Nếu lỗi, dùng mặc định an toàn.
fn doc_config() -> CauHinhCanType {
    let Some(duong_dan) = cau_hinh::duong_dan_config() else {
        return CauHinhCanType::default();
    };
    match cau_hinh::doc(&duong_dan) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("CanType: loi doc config ({e}), dung mac dinh");
            CauHinhCanType::default()
        }
    }
}

/// Bind `CauHinhCanType` vào properties cửa sổ Slint.
fn bind_config(app: &App, cfg: &CauHinhCanType) {
    app.set_dang_bat(cfg.dang_bat);
    app.set_kieu_go(match cfg.kieu_go {
        KieuGo::Telex => "telex".into(),
        KieuGo::Vni => "vni".into(),
    });
    app.set_khoi_dong_cung_he_thong(cfg.khoi_dong_cung_he_thong);
}

/// Lấy `CauHinhCanType` từ trạng thái UI hiện tại.
fn lay_config_tu_ui(app: &App) -> CauHinhCanType {
    let kieu_go = app.get_kieu_go();
    let kieu_go = if kieu_go.as_str() == "vni" {
        KieuGo::Vni
    } else {
        KieuGo::Telex
    };
    CauHinhCanType {
        phien_ban: cau_hinh::PHIEN_BAN_SCHEMA,
        dang_bat: app.get_dang_bat(),
        kieu_go,
        chinh_sach: ChinhSach::TuNhien,
        kieu_telex: KieuTelex::CanBang,
        quy_tac_dat_dau: QuyTacDatDau::HienDai,
        dang_unicode: DangUnicode::Nfc,
        khoi_dong_cung_he_thong: app.get_khoi_dong_cung_he_thong(),
    }
}
