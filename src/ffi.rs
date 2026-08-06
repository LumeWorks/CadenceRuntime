// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Ranh giới FFI với C++ shim Fcitx5.
//!
//! Module duy nhất được phép dùng `unsafe`. Mọi `unsafe` block có giải thích
//! `// SAFETY:`. C ABI nhỏ, POD-only: không truyền `String`/`Vec`/Rust enum
//! không `repr`/reference/trait object qua ABI. Hàm Rust xuất dùng
//! `catch_unwind`; panic không vượt FFI. Header C ABI là
//! `native/fcitx5_ffi.h`.
//!
//! Phase 2 commit này chỉ dựng factory entry. Toàn bộ hàm `cadence_*` (phien,
//! xu_ly_phim) và unsafe isolation đầy đủ wire ở commit FFI boundary.

#![allow(unsafe_code)]

/// Entry factory do Fcitx loader `dlsym`. Định nghĩa trong Rust (`#[no_mangle]`)
/// để попад vào version script export của cdylib Rust (Rust cdylib chỉ export
/// symbol `#[no_mangle]` Rust). Gọi `cadence_native_factory` của C++ trả static
/// `fcitx::AddonFactory*`. Không dùng `FCITX_ADDON_FACTORY` (C++) vì linker
/// localize symbol C++ không trong version script.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn fcitx_addon_factory_instance() -> *mut core::ffi::c_void {
    // SAFETY: `cadence_native_factory` là hàm C (`extern "C"`) xuất bởi C++
    // shim, trả static `AddonFactory*`. Gọi nó không đọc state Rust, không có
    // điều kiện tiên quyết. Function-local static trong C++ init thread-safe.
    // Không catch_unwind ở đây: hàm C++ thuần trả pointer, không panic.
    unsafe { cadence_native_factory() }
}

#[cfg(feature = "fcitx5")]
unsafe extern "C" {
    /// Factory do C++ `cadence_native_factory` xuất. Trả `AddonFactory*`.
    fn cadence_native_factory() -> *mut core::ffi::c_void;
}
