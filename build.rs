// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Build script: khi feature `fcitx5` bật, compile C++ shim (`native/fcitx5.cpp`)
//! bằng `cc`, tìm Fcitx5Core qua `pkg-config`, và link vào cdylib.
//!
//! Khi feature tắt, build.rs không làm gì — lõi Rust thuần, không cần Fcitx5
//! dev. Entry point chính vẫn là `cargo build` / `cargo test`. Factory symbol
//! `fcitx_addon_factory_instance` (xuất bằng `#[no_mangle]` trong `src/ffi.rs`)
//! là symbol duy nhất loader Fcitx `dlsym`; nó gọi `cantype_native_factory` của
//! C++ trả static `AddonFactory*`. Xem `docs/PHASE_2_FCITX5.md`.

fn main() {
    #[cfg(feature = "app")]
    {
        build_app();
    }
    #[cfg(feature = "fcitx5")]
    {
        build_fcitx5();
    }
}

/// Compile `ui/cantype.slint` thành Rust module qua `slint-build`. Module được
/// nhúng vào binary `cantype` qua `slint::include_modules!()` trong `main.rs`.
/// Chỉ chạy khi feature `app` bật.
#[cfg(feature = "app")]
fn build_app() {
    slint_build::compile("ui/cantype.slint")
        .expect("Khong the compile ui/cantype.slint. Kiem tra Slint syntax va file ton tai.");
    println!("cargo:rerun-if-changed=ui/cantype.slint");
}

/// Compile C++ shim và link Fcitx5Core. Chỉ compile khi feature `fcitx5` bật
/// (để `cc`/`pkg-config` optional dependency không cần khi feature tắt).
#[cfg(feature = "fcitx5")]
fn build_fcitx5() {
    // pkg-config tìm Fcitx5Core (header + lib).
    let pk = pkg_config::Config::new()
        .atleast_version("5.0.14")
        .probe("Fcitx5Core")
        .expect(
            "Khong tim thay Fcitx5Core qua pkg-config. Cai libfcitx5core-dev \
             va libfcitx5utils-dev, hoac dat PKG_CONFIG_PATH. Build voi feature \
             `fcitx5` can Fcitx5 dev headers.",
        );

    println!("cargo:rerun-if-changed=native/fcitx5.cpp");
    println!("cargo:rerun-if-changed=native/fcitx5_ffi.h");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .file("native/fcitx5.cpp")
        .include("native")
        .warnings(true)
        .extra_warnings(true)
        .flag("-std=c++17")
        .flag("-Wall")
        .flag("-Wextra")
        .flag("-Wpedantic")
        .flag("-Werror");

    // Fcitx5 headers dùng `-isystem` (system include) để warning trong header
    // Fcitx không làm fail build với -Werror; chỉ code trong `native/` chịu
    // -Wall/-Wextra/-Wpedantic/-Werror.
    for inc in &pk.include_paths {
        build.flag(format!("-isystem{}", inc.display()));
    }

    // Compile C++ thành static archive `cantype_native` (link vào cdylib).
    // cc::compile phát `cargo:rustc-link-lib=static=cantype_native`.
    build.compile("cantype_native");

    // Link Fcitx5Core và dependency (Fcitx5Utils/Fcitx5Config từ Requires).
    for lib in &pk.libs {
        println!("cargo:rustc-link-lib={}", lib);
    }
    for lib in &pk.frameworks {
        println!("cargo:rustc-link-lib=framework={}", lib);
    }
    for path in &pk.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
}
