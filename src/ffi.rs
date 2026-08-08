// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Ranh giới FFI với C++ shim Fcitx5.
//!
//! Module duy nhất được phép dùng `unsafe`. Mọi `unsafe` block có giải thích
//! `// SAFETY:`. C ABI nhỏ, POD-only. Hàm Rust xuất dùng `catch_unwind`; panic
//! không vượt FFI. Callback C++ không truyền exception sang Rust (contract C++).
//!
//! Ownership: `cantype_phien_tao` trả `Box<PhienNhap>` thành raw pointer (leak
//! có chủ đích); `cantype_phien_giai_phong` tái lập và drop. Mỗi handle đúng
//! một lần (C++ đặt `nullptr` sau khi free). `cantype_xu_ly_phim` mượn
//! `&mut PhienNhap` + `&CanTypeKeySnapshot` + `&CanTypeHostBang` trong một lời
//! gọi; con trỏ C++ phải hợp lệ trong suốt lời gọi.

#![allow(unsafe_code)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::ContextId;
use crate::fcitx5::{
    AnhXaPhim, CanTypeContextSnapshot, CanTypeHostBang, CanTypeKeySnapshot, CanTypeXuLyKetQua,
    PhimRust, anh_xa_phim, chuyen_boi_canh,
};
use crate::host::{BoiCanhNhap, HanhDong, Host, KetQuaHost};
use crate::phien::{KetQuaXuLy, PhienNhap};

/// Host Fcitx5: triển khai [`Host`] qua bảng callback C++ (`CanTypeHostBang`).
///
/// Mỗi lời gọi `boi_canh`/`thuc_thi` gọi callback C++ (unsafe, gói trong hàm
/// an toàn này). Không sở hữu `ic` hay callback; mượn trong thời gian sống của
/// `xu_ly_phim`.
pub(crate) struct FcitxHost<'a> {
    bang: &'a CanTypeHostBang,
}

impl<'a> FcitxHost<'a> {
    /// Tạo host từ bảng callback. Trả `None` nếu bảng hoặc `ic` null.
    pub(crate) fn moi(bang: &'a CanTypeHostBang) -> Option<Self> {
        if bang.ic.is_null() {
            return None;
        }
        Some(Self { bang })
    }

    /// Gọi callback `lay_boi_canh`, điền snapshot. Trả `false` nếu callback
    /// null hoặc lỗi.
    fn lay_boi_canh(&self, snapshot: &mut CanTypeContextSnapshot) -> bool {
        let Some(cb) = self.bang.lay_boi_canh else {
            return false;
        };
        // `cb` là `extern "C" fn` (safe để gọi); `bang.ic` đã check non-null.
        // `snapshot` là &mut valid. Callback C++ không ném exception (contract).
        let ret = cb(self.bang.ic, snapshot);
        ret == 0
    }
}

/// Thông tin đầy đủ từ snapshot ngữ cảnh: [`BoiCanhNhap`] (cho runtime) +
/// metadata Phase 3 (frontend/program/capability cho diagnostic và phân loại).
///
/// `boi_canh()` chỉ dùng `boi_canh`; diagnostic (`#[cfg(feature = "diag")]`)
/// dùng thêm metadata. Cả hai đọc snapshot MỘT lần qua callback C++.
#[cfg(feature = "diag")]
struct ThongTinDayDu {
    /// Ngữ cảnh nhập (cho `Host::boi_canh`).
    boi_canh: BoiCanhNhap,
    /// `ic->frontend()` (String owned, copy ra từ callback).
    frontend: String,
    /// `ic->program()` (String owned).
    program: String,
    /// `ic->capabilityFlags().toInteger()` (bitmask CapabilityFlag).
    capability: u64,
    /// `true` nếu `surroundingText().isValid()`.
    surrounding_hop_le: bool,
    /// Offset con trỏ (ký tự).
    cursor: u32,
    /// Offset anchor (ký tự).
    anchor: u32,
    /// Độ dài surrounding text theo byte (KHÔNG phải giá trị text).
    surrounding_do_dai: usize,
}

impl<'a> FcitxHost<'a> {
    /// Đọc snapshot ngữ cảnh đầy đủ qua callback C++, trả `None` nếu callback
    /// lỗi/null. Đọc `text`/`frontend`/`program` thành String owned ngay trong
    /// scope callback (ptr chỉ valid trong lời gọi).
    #[cfg(feature = "diag")]
    fn doc_thong_tin_day_du(&self) -> Option<ThongTinDayDu> {
        let mut snapshot = CanTypeContextSnapshot {
            context_id: 0,
            focus_generation: 0,
            has_focus: 0,
            surrounding_valid: 0,
            cursor: 0,
            anchor: 0,
            text: crate::fcitx5::CanTypeSlice {
                ptr: std::ptr::null(),
                len: 0,
            },
            frontend: crate::fcitx5::CanTypeSlice {
                ptr: std::ptr::null(),
                len: 0,
            },
            program: crate::fcitx5::CanTypeSlice {
                ptr: std::ptr::null(),
                len: 0,
            },
            capability: 0,
        };
        if !self.lay_boi_canh(&mut snapshot) {
            return None;
        }
        // SAFETY: `snapshot.{text,frontend,program}.ptr` do C++ điền, hợp lệ
        // trong thời gian lời gọi (C++ borrow std::string/const char* sống
        // cùng InputContext). `len` byte. `lay_chuoi_utf8` copy ra String owned
        // ngay; sau khi trả, ptr không còn dùng. from_utf8 kiểm tra ranh giới;
        // nếu null/invalid → "".
        let text = unsafe { lay_chuoi_utf8(&snapshot.text) };
        let frontend = unsafe { lay_chuoi_utf8(&snapshot.frontend) };
        let program = unsafe { lay_chuoi_utf8(&snapshot.program) };
        let boi_canh = chuyen_boi_canh(&snapshot, &text);
        Some(ThongTinDayDu {
            boi_canh,
            frontend,
            program,
            capability: snapshot.capability,
            surrounding_hop_le: snapshot.surrounding_valid != 0,
            cursor: snapshot.cursor,
            anchor: snapshot.anchor,
            surrounding_do_dai: text.len(),
        })
    }
}

impl<'a> Host for FcitxHost<'a> {
    fn boi_canh(&self) -> BoiCanhNhap {
        let mut snapshot = CanTypeContextSnapshot {
            context_id: 0,
            focus_generation: 0,
            has_focus: 0,
            surrounding_valid: 0,
            cursor: 0,
            anchor: 0,
            text: crate::fcitx5::CanTypeSlice {
                ptr: std::ptr::null(),
                len: 0,
            },
            frontend: crate::fcitx5::CanTypeSlice {
                ptr: std::ptr::null(),
                len: 0,
            },
            program: crate::fcitx5::CanTypeSlice {
                ptr: std::ptr::null(),
                len: 0,
            },
            capability: 0,
        };
        if !self.lay_boi_canh(&mut snapshot) {
            // Callback lỗi/null → context rỗng, surrounding None.
            return BoiCanhNhap {
                context_id: ContextId(0),
                the_he_focus: 0,
                dang_co_focus: false,
                van_ban_truoc_con_tro: None,
                co_surrounding: false,
                co_preedit: false,
                co_sensitive: false,
            };
        }
        // SAFETY: `snapshot.text.ptr` do C++ điền, hợp lệ trong thời gian lời
        // gọi (C++ borrow surrounding std::string). `len` byte. from_utf8 kiểm
        // tra ranh giới; nếu null/invalid → "".
        let text = unsafe { lay_chuoi_utf8(&snapshot.text) };
        chuyen_boi_canh(&snapshot, &text)
    }

    fn thuc_thi(&mut self, hanh_dong: &HanhDong) -> KetQuaHost {
        match hanh_dong {
            HanhDong::Chen(s) => {
                let Some(cb) = self.bang.chen else {
                    return KetQuaHost::KhongPhat;
                };
                // `cb` là `extern "C" fn` (safe để gọi). `bang.ic` non-null.
                // Chuỗi UTF-8 truyền qua (ptr, len); C++ copy vào std::string.
                let ret = cb(self.bang.ic, s.as_ptr(), s.len());
                chuyen_ket_qua_host(ret)
            }
            HanhDong::ThayThe(ke) => {
                let Some(cb) = self.bang.thay_the else {
                    return KetQuaHost::KhongPhat;
                };
                let xoa_ky_tu = u32::try_from(ke.xoa_truoc.ky_tu_unicode).unwrap_or(u32::MAX);
                // `ke.chen` là String; truyền (ptr, len). C++ delete rồi commit.
                let ret = cb(self.bang.ic, xoa_ky_tu, ke.chen.as_ptr(), ke.chen.len());
                chuyen_ket_qua_host(ret)
            }
            HanhDong::CapNhatSoanThao(s) => {
                let Some(cb) = self.bang.cap_nhat_soan_thao else {
                    return KetQuaHost::KhongPhat;
                };
                let ret = cb(self.bang.ic, s.as_ptr(), s.len());
                chuyen_ket_qua_host(ret)
            }
            HanhDong::KetThucSoanThao(s) => {
                let Some(cb) = self.bang.ket_thuc_soan_thao else {
                    return KetQuaHost::KhongPhat;
                };
                let ret = cb(self.bang.ic, s.as_ptr(), s.len());
                chuyen_ket_qua_host(ret)
            }
            HanhDong::XoaSoanThao => {
                let Some(cb) = self.bang.xoa_soan_thao else {
                    return KetQuaHost::KhongPhat;
                };
                let ret = cb(self.bang.ic);
                chuyen_ket_qua_host(ret)
            }
            HanhDong::ChuyenTiep => KetQuaHost::KhongPhat,
        }
    }
}

/// Đọc `CanTypeSlice` thành `String` (validate UTF-8, copy). Trả `String` rỗng
/// nếu ptr null hoặc UTF-8 invalid.
///
/// # Safety
///
/// `slice.ptr` phải hợp lệ cho `slice.len` byte (hoặc null), không alias Rust
/// memory đang mượn mutable. C++ đảm bảo trong thời gian lời gọi FFI.
unsafe fn lay_chuoi_utf8(slice: &crate::fcitx5::CanTypeSlice) -> String {
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

/// Chuyển [`CanTypeKeySnapshot`] (C ABI, có con trỏ) sang [`PhimRust`] (thuần
/// Rust, không con trỏ). Đọc `utf8` slice một lần rồi trích `char` đầu tiên.
///
/// # Safety
///
/// `key.utf8.ptr` phải hợp lệ cho `key.utf8.len` byte (hoặc null) trong suốt
/// lời gọi. C++ đảm bảo (std::string sống trong scope `keyEvent`).
unsafe fn doc_phim(key: &CanTypeKeySnapshot) -> PhimRust {
    let utf8 = unsafe { lay_chuoi_utf8(&key.utf8) };
    let ky_tu = utf8.chars().next();
    PhimRust {
        is_release: key.is_release != 0,
        is_cursor_move: key.is_cursor_move != 0,
        is_modifier: key.is_modifier != 0,
        has_modifier: key.has_modifier != 0,
        dac_biet: key.dac_biet,
        ky_tu,
    }
}

/// Ánh xả [`KetQuaXuLy`] sang [`CanTypeXuLyKetQua`] (C ABI).
fn chuyen_ket_qua_xu_ly(kq: KetQuaXuLy) -> CanTypeXuLyKetQua {
    match kq {
        KetQuaXuLy::DaApDung => CanTypeXuLyKetQua::DaApDung,
        KetQuaXuLy::ChuyenTiep => CanTypeXuLyKetQua::ChuyenTiep,
        KetQuaXuLy::MatDongBo => CanTypeXuLyKetQua::MatDongBo,
    }
}

/// Ánh xạ giá trị trả về của callback C++ sang [`KetQuaHost`].
/// 0 = DaPhat, 1 = KhongPhat, 2 = KhongChac. Khác → KhongChac (an toàn).
fn chuyen_ket_qua_host(ret: i32) -> KetQuaHost {
    match ret {
        0 => KetQuaHost::DaPhat,
        1 => KetQuaHost::KhongPhat,
        _ => KetQuaHost::KhongChac,
    }
}

/// Entry factory do Fcitx loader `dlsym`. Định nghĩa trong Rust (`#[no_mangle]`)
/// để lọt vào dynsym export của cdylib Rust; gọi `cantype_native_factory` của
/// C++ trả static `AddonFactory*`.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn fcitx_addon_factory_instance() -> *mut core::ffi::c_void {
    // SAFETY: hàm C++ thuần trả static pointer, không panic, không đọc state Rust.
    unsafe { cantype_native_factory() }
}

#[cfg(feature = "fcitx5")]
unsafe extern "C" {
    fn cantype_native_factory() -> *mut core::ffi::c_void;
}

/// Tạo `PhienNhap` mới cho `context_id`, trả opaque handle.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn cantype_phien_tao(context_id: u64) -> *mut core::ffi::c_void {
    let res = catch_unwind(|| {
        let phien = PhienNhap::moi(ContextId(context_id));
        Box::into_raw(Box::new(phien)) as *mut core::ffi::c_void
    });
    res.unwrap_or(std::ptr::null_mut())
}

/// Giải phóng `PhienNhap` handle. An toàn với NULL. Mỗi handle đúng một lần.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn cantype_phien_giai_phong(phien: *mut core::ffi::c_void) {
    if phien.is_null() {
        return;
    }
    // SAFETY: `phien` do `cantype_phien_tao` trả (Box::into_raw), non-null ở đây.
    // Tái lập Box và drop. Caller (C++) phải đặt nullptr sau — không gọi 2 lần.
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(phien as *mut PhienNhap));
    }));
}

/// Đặt lại phiên (relinquish): dùng cho reset/deactivate/focus-out.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn cantype_phien_dat_lai(phien: *mut core::ffi::c_void) {
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

/// Xử lý một phím. Trả [`CanTypeXuLyKetQua`]. Panic không vượt FFI.
#[cfg(feature = "fcitx5")]
#[unsafe(no_mangle)]
pub extern "C" fn cantype_xu_ly_phim(
    phien: *mut core::ffi::c_void,
    key: *const CanTypeKeySnapshot,
    bang: *const CanTypeHostBang,
) -> CanTypeXuLyKetQua {
    let res = catch_unwind(AssertUnwindSafe(|| xu_ly_phim_noi_bo(phien, key, bang)));
    res.unwrap_or(CanTypeXuLyKetQua::ChuyenTiep)
}

/// Xử lý phím nội bộ (không catch_unwind; wrapper đã catch).
fn xu_ly_phim_noi_bo(
    phien: *mut core::ffi::c_void,
    key: *const CanTypeKeySnapshot,
    bang: *const CanTypeHostBang,
) -> CanTypeXuLyKetQua {
    if phien.is_null() || key.is_null() || bang.is_null() {
        return CanTypeXuLyKetQua::ChuyenTiep;
    }
    // SAFETY: `phien` hợp lệ (Box::into_raw) — mượn &mut cho lời gọi này.
    // `key`, `bang` hợp lệ (C++ borrow) cho lời gọi. Không alias nhau.
    let phien: &mut PhienNhap = unsafe { &mut *(phien as *mut PhienNhap) };
    let key_ref: &CanTypeKeySnapshot = unsafe { &*key };
    let bang_ref: &CanTypeHostBang = unsafe { &*bang };

    // SAFETY: `key_ref.utf8.ptr` hợp lệ trong scope `keyEvent` (C++ borrow
    // std::string). `doc_phim` đọc slice một lần, copy ra `PhimRust`.
    let phim = unsafe { doc_phim(key_ref) };
    match anh_xa_phim(&phim) {
        AnhXaPhim::BoQua => CanTypeXuLyKetQua::ChuyenTiep,
        AnhXaPhim::SuKien(su_kien) => {
            let Some(mut host) = FcitxHost::moi(bang_ref) else {
                // Bang null → không xử lý, passthrough (không nuốt phím).
                return CanTypeXuLyKetQua::ChuyenTiep;
            };
            let kq = phien.xu_ly(&mut host, &su_kien);
            // Phase 3 diagnostic: trace metadata KHÔNG chứa text user. Chỉ
            // compile khi feature `diag` bật, và chỉ chạy khi env
            // `CANTYPE_DEBUG` set (`ghi_chan_doan` check OnceLock). Production
            // build không có `diag` → block không tồn tại → zero cost.
            #[cfg(feature = "diag")]
            {
                if let Some(t) = host.doc_thong_tin_day_du() {
                    let da_hien_thi = phien.da_hien_thi();
                    let suffix_khop = t
                        .boi_canh
                        .van_ban_truoc_con_tro
                        .as_ref()
                        .map(|v| v.ends_with(da_hien_thi))
                        .unwrap_or(false);
                    let (action, outcome) = match kq {
                        KetQuaXuLy::DaApDung => ("da_xu_ly", "da_ap_dung"),
                        KetQuaXuLy::ChuyenTiep => ("passthrough", "chuyen_tiep"),
                        KetQuaXuLy::MatDongBo => ("reset", "mat_dong_bo"),
                    };
                    let route = phien.duong_hien_tai();
                    let cd = crate::tuong_thich::ChanDoan {
                        context_id: t.boi_canh.context_id.0,
                        frontend: crate::tuong_thich::phan_loai(&t.frontend),
                        program: t.program,
                        co_surrounding: (t.capability & crate::tuong_thich::co_cap::SURROUNDING)
                            != 0,
                        surrounding_hop_le: t.surrounding_hop_le,
                        co_focus: t.boi_canh.dang_co_focus,
                        focus_generation: t.boi_canh.the_he_focus,
                        co_password: (t.capability & crate::tuong_thich::co_cap::PASSWORD) != 0,
                        co_terminal: (t.capability & crate::tuong_thich::co_cap::TERMINAL) != 0,
                        co_sensitive: (t.capability & crate::tuong_thich::co_cap::SENSITIVE) != 0,
                        co_preedit: (t.capability & crate::tuong_thich::co_cap::PREDIT) != 0,
                        surrounding_do_dai: t.surrounding_do_dai,
                        cursor: t.cursor,
                        anchor: t.anchor,
                        route,
                        hanh_dong: action,
                        ket_qua: outcome,
                        da_hien_thi_do_dai: da_hien_thi.len(),
                        suffix_khop,
                    };
                    crate::tuong_thich::ghi_chan_doan(&cd);
                }
            }
            chuyen_ket_qua_xu_ly(kq)
        }
    }
}
