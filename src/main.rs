// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Binary `cantype` — GUI + tray CanType.
//!
//! Chỉ build khi feature `app` bật (Slint dep). Mở cửa sổ chính từ
//! `ui/cantype.slint`. Tab, settings, tray và binding cấu hình được thêm ở
//! commit sau.
//!
//! Lõi runtime (addon Fcitx5) là library crate riêng (`cantype` lib), không
//! phụ thuộc GUI. Binary này không sở hữu Rust sessions của Fcitx5; GUI và
//! addon chỉ dùng chung schema cấu hình.

#![cfg(feature = "app")]
#![cfg_attr(not(feature = "app"), allow(dead_code))]

// Code Slint generate (`slint::include_modules!`) không có doc comments và dùng
// `unwrap()` nội bộ; lints `missing_docs = warn` + `unwrap_used = deny` của
// package sẽ bắt. Allow ở binary crate (không ảnh hưởng library crate
// `cantype`).
#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    App::new()?.run()
}
