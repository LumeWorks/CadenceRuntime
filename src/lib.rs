// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! CadenceRuntime - runtime thuần Rust, độc lập nền tảng cho lõi gõ tiếng Việt
//! Cadence.
//!
//! Phase 1 dựng nền móng xử lý phiên nhập, tích hợp Cadence, tính kế hoạch sửa
//! committed text, bảo vệ đồng bộ context và mô phỏng host. Xem
//! [`docs/PHASE_1_RUNTIME.md`] cho thiết kế đầy đủ.
//!
//! [`docs/PHASE_1_RUNTIME.md`]: https://github.com/LumeWorks/CadenceRuntime

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub(crate) mod cadence;
/// Adapter Fcitx5 — logic thuần Rust (không unsafe), luôn compile. Kiểu
/// `#[repr(C)]` + key mapping + surrounding extraction. Test chạy không cần
/// Fcitx5 dev.
pub(crate) mod fcitx5;
pub mod host;
pub(crate) mod phien;
pub(crate) mod sua;
// FFI boundary với C++ shim Fcitx5. Module duy nhất dùng `unsafe` (allow trong
// module). Chỉ compile khi feature `fcitx5` bật.
#[cfg(feature = "fcitx5")]
pub(crate) mod ffi;

pub use host::{BoiCanhNhap, ContextId, HanhDong, Host, KetQuaHost};
pub use phien::{KetQuaXuLy, PhienNhap, SuKienNhap};
pub use sua::{DoDaiVanBan, KeHoachSua};
