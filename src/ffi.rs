// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Ranh giới FFI với C++ shim Fcitx5.
//!
//! Module duy nhất được phép dùng `unsafe`. Mọi `unsafe` block có giải thích
//! `// SAFETY:`. C ABI nhỏ, POD-only. Hàm Rust xuất dùng `catch_unwind`; panic
//! không vượt FFI. Callback C++ không truyền exception sang Rust (contract C++).
//!
//! Ownership: `cadence_phien_tao` trả `Box<PhienNhap>` thành raw pointer (leak
//! có chủ đích); `cadence_phien_giai_phong` tái lập và drop. Mỗi handle đúng
//! một lần (C++ đặt `nullptr` sau khi free). `cadence_xu_ly_phim` mượn
//! `&mut PhienNhap` + `&CadenceKeySnapshot` + `&CadenceHostBang` trong một lời
//! gọi; con trỏ C++ phải hợp lệ trong suốt lời gọi.

#![allow(unsafe_code)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::ContextId;
use crate::fcitx5::{
    AnhXaPhim, CadenceContextSnapshot, CadenceHostBang, CadenceKeySnapshot, CadenceXuLyKetQua,
    anh_xa_phim, chuyen_boi_canh,
};
use crate::host::{BoiCanhNhap, HanhDong, Host, KetQuaHost};
use crate::phien::{KetQuaXuLy, PhienNhap};

/// Host Fcitx5: triển khai [`Host`] qua bảng callback C++ (`CadenceHostBang`).
///
/// Mỗi lời gọi `boi_canh`/`thuc_thi` gọi callback C++ (unsafe, gói trong hàm
/// an toàn này). Không sở hữu `ic` hay callback; mượn trong thời gian sống của
/// `xu_ly_phim`.
pub(crate) struct FcitxHost<'a> {
    bang: &'a CadenceHostBang,
}

impl<'a> FcitxHost<'a> {
    /// Tạo host từ bảng callback. Trả `None` nếu bảng hoặc `ic` null.
    pub(crate) fn moi(bang: &'a CadenceHostBang) -> Option<Self> {
        if bang.ic.is_null() {
            return None;
        }
        Some(Self { bang })
    }

    /// Gọi callback `lay_boi_canh`, điền snapshot. Trả `false` nếu callback
    /// null hoặc lỗi.
    fn lay_boi_canh(&self, snapshot: &mut CadenceContextSnapshot) -> bool {
        let Some(cb) = self.bang.lay_boi_canh else {
            return false;
        };
        // `cb` là `extern "C" fn` (safe để gọi); `bang.ic` đã check non-null.
        // `snapshot` là &mut valid. Callback C++ không ném exception (contract).
        let ret = cb(self.bang.ic, snapshot);
        ret == 0
    }
}

impl<'a> Host for FcitxHost<'a> {
    fn boi_canh(&self) -> BoiCanhNhap {
        let mut snapshot = CadenceContextSnapshot {
            context_id: 0,
            focus_generation: 0,
            has_focus: 0,
            surrounding_valid: 0,
            cursor: 0,
            anchor: 0,
            text: crate::fcitx5::CadenceSlice {
                ptr: std::ptr::null(),
                len: 0,
            },
        };
        if !self.lay_boi_canh(&mut snapshot) {
            // Callback lỗi/null → context rỗng, surrounding None.
            return BoiCanhNhap {
                context_id: ContextId(0),
                the_he_focus: 0,
                dang_co_focus: false,
                van_ban_truoc_con_tro: None,
            };
        }
        // SAFETY: `snapshot.text.ptr` do C++ điền, hợp lệ trong thời gian lời
        // gọi (C++ borrow surrounding std::string). `len` byte. from_utf8 kiểm
        // tra ranh giới; nếu null/invalid → "".
        let text = unsafe { lay_chuoi_utf8(&snapshot.text) };
        chuyen_boi_canh(&snapshot, &text)
    }

    fn thuc_thi(&mut self, _hanh_dong: &HanhDong) -> KetQuaHost {
        // Commit 4: stub. Insert (Chen) và Replace (ThayThe) thật ở commit kế.
        // Passthrough: không phát lệnh mutation nào → KhongPhat.
        KetQuaHost::KhongPhat
    }
}

/// Đọc `CadenceSlice` thành `String` (validate UTF-8, copy). Trả `String` rỗng
/// nếu ptr null hoặc UTF-8 invalid.
///
/// # Safety
///
/// `slice.ptr` phải hợp lệ cho `slice.len` byte (hoặc null), không alias Rust
/// memory đang mượn mutable. C++ đảm bảo trong thời gian lời gọi FFI.
unsafe fn lay_chuoi_utf8(slice: &crate::fcitx5::CadenceSlice) -> String {
    if slice.ptr.is_null() || slice.len == 0 {
        return String::new();
    }
    // SAFETY: ptr hợp lệ len byte (contract C++). from_raw_parts tạo &[u8];
    // from_utf8 kiểm tra; nếu invalid trả "".
    let bytes = unsafe { std::slice::from_raw_parts(slice.ptr, slice.len) };
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => String::new(),
    }
}

/// Ánh xạ [`KetQuaXuLy`] sang [`CadenceXuLyKetQua`] (C ABI).
fn chuyen_ket_qua_xu_ly(kq: KetQuaXuLy) -> CadenceXuLyKetQua {
    match kq {
        KetQuaXuLy::DaApDung => CadenceXuLyKetQua::DaApDung,
        KetQuaXuLy::ChuyenTiep => CadenceXuLyKetQua::ChuyenTiep,
        KetQuaXuLy::MatDongBo => CadenceXuLyKetQua::MatDongBo,
    }
}

/// Entry factory do Fcitx loader `dlsym`. Định nghĩa trong Rust (`#[no_mangle]`)
/// để попад vào version script export của cdylib Rust; gọi `cadence_native_
/// factory` của C++ trả static `AddonFactory*`.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn fcitx_addon_factory_instance() -> *mut core::ffi::c_void {
    // SAFETY: hàm C++ thuần trả static pointer, không panic, không đọc state Rust.
    unsafe { cadence_native_factory() }
}

#[cfg(feature = "fcitx5")]
unsafe extern "C" {
    fn cadence_native_factory() -> *mut core::ffi::c_void;
}

/// Tạo `PhienNhap` mới cho `context_id`, trả opaque handle.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn cadence_phien_tao(context_id: u64) -> *mut core::ffi::c_void {
    let res = catch_unwind(|| {
        let phien = PhienNhap::moi(ContextId(context_id));
        Box::into_raw(Box::new(phien)) as *mut core::ffi::c_void
    });
    res.unwrap_or(std::ptr::null_mut())
}

/// Giải phóng `PhienNhap` handle. An toàn với NULL. Mỗi handle đúng một lần.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn cadence_phien_giai_phong(phien: *mut core::ffi::c_void) {
    if phien.is_null() {
        return;
    }
    // SAFETY: `phien` do `cadence_phien_tao` trả (Box::into_raw), non-null ở đây.
    // Tái lập Box và drop. Caller (C++) phải đặt nullptr sau — không gọi 2 lần.
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(phien as *mut PhienNhap));
    }));
}

/// Đặt lại phiên (relinquish): dùng cho reset/deactivate/focus-out.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn cadence_phien_dat_lai(phien: *mut core::ffi::c_void) {
    if phien.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        dat_lai_noi_bo(phien);
    }));
}

/// Đặt lại phiên nội bộ (không catch_unwind; gọi từ wrapper có catch).
unsafe fn dat_lai_noi_bo(phien: *mut core::ffi::c_void) {
    // SAFETY: `phien` hợp lệ (Box::into_raw). Mượn &mut, gọi `dat_lai()`
    // relinquish trực tiếp (không cần Host).
    let phien: &mut PhienNhap = unsafe { &mut *(phien as *mut PhienNhap) };
    phien.dat_lai();
}

/// Xử lý một phím. Trả [`CadenceXuLyKetQua`]. Panic không vượt FFI.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn cadence_xu_ly_phim(
    phien: *mut core::ffi::c_void,
    key: *const CadenceKeySnapshot,
    bang: *const CadenceHostBang,
) -> CadenceXuLyKetQua {
    let res = catch_unwind(AssertUnwindSafe(|| xu_ly_phim_noi_bo(phien, key, bang)));
    res.unwrap_or(CadenceXuLyKetQua::ChuyenTiep)
}

/// Xử lý phím nội bộ (không catch_unwind; wrapper đã catch).
fn xu_ly_phim_noi_bo(
    phien: *mut core::ffi::c_void,
    key: *const CadenceKeySnapshot,
    bang: *const CadenceHostBang,
) -> CadenceXuLyKetQua {
    if phien.is_null() || key.is_null() || bang.is_null() {
        return CadenceXuLyKetQua::ChuyenTiep;
    }
    // SAFETY: `phien` hợp lệ (Box::into_raw) — mượn &mut cho lời gọi này.
    // `key`, `bang` hợp lệ (C++ borrow) cho lời gọi. Không alias nhau.
    let phien: &mut PhienNhap = unsafe { &mut *(phien as *mut PhienNhap) };
    let key_ref: &CadenceKeySnapshot = unsafe { &*key };
    let bang_ref: &CadenceHostBang = unsafe { &*bang };

    match anh_xa_phim(key_ref) {
        AnhXaPhim::BoQua => CadenceXuLyKetQua::ChuyenTiep,
        AnhXaPhim::SuKien(su_kien) => {
            let Some(mut host) = FcitxHost::moi(bang_ref) else {
                // Bang null → không xử lý, passthrough (không nuốt phím).
                return CadenceXuLyKetQua::ChuyenTiep;
            };
            let kq = phien.xu_ly(&mut host, &su_kien);
            chuyen_ket_qua_xu_ly(kq)
        }
    }
}
