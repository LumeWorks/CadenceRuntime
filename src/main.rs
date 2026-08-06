// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Binary `cantype` — GUI + tray CanType.
//!
//! Chỉ build khi feature `app` bật (Slint dep). Mở cửa sổ chính qua
//! [`ung_dung::chay`]. Tray icon + trạng thái V/E được thêm ở commit sau.
//!
//! Lõi runtime (addon Fcitx5) là library crate riêng (`cantype` lib), không
//! phụ thuộc GUI. Binary này không sở hữu Rust sessions của Fcitx5; GUI và
//! addon chỉ dùng chung schema cấu hình.

#![cfg(feature = "app")]
#![cfg_attr(not(feature = "app"), allow(dead_code))]

fn main() -> Result<(), slint::PlatformError> {
    cantype::ung_dung::chay()
}
