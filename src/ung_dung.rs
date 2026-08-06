// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Logic GUI CanType: đọc/ghi cấu hình, bind vào cửa sổ Slint, xử lý callbacks.
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

/// Chạy GUI CanType. Đọc config, mở cửa sổ, bind callbacks. Trả lỗi platform
/// nếu Slint không khởi tạo được.
///
/// # Errors
/// Trả `slint::PlatformError` nếu backend Slint không khởi tạo được.
pub fn chay() -> Result<(), slint::PlatformError> {
    let app = App::new()?;

    // Đọc config, bind vào properties.
    let cfg = doc_config();
    bind_config(&app, &cfg);

    // Callbacks.
    let app_luu = app.as_weak();
    app.on_luu(move || {
        let app = app_luu.unwrap();
        let cfg = lay_config_tu_ui(&app);
        if let Some(duong_dan) = cau_hinh::duong_dan_config()
            && let Err(e) = cau_hinh::ghi(&duong_dan, &cfg)
        {
            eprintln!("CanType: loi ghi config: {e}");
        }
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
        slint::quit_event_loop().ok();
    });

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
