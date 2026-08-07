# AGENTS.md

Hướng dẫn cho agent/maintainer làm việc trên CanType.

## Phong cách

Giữ đồng bộ với Cadence (`https://github.com/LumeWorks/Cadence`):

* Identifier domain tiếng Việt không dấu (`PhienNhap`, `them_ky_tu`).
* Comment và tài liệu tiếng Việt có dấu.
* Không wildcard import, không `unwrap()` trong production code, không
  `expect()` trừ bất biến nội bộ đã chứng minh và có giải thích.
* Commit message ngắn tiếng Việt không dấu.
* Không force-push, không amend/squash, không rewrite history.

## Lệnh kiểm tra (chạy trước khi commit)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo check --release --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo +1.92 check --all-targets --all-features   # MSRV
```

Feature gates (addon không kéo Slint, app không cần Fcitx5 dev):

```bash
cargo test --no-default-features
cargo check --no-default-features
cargo check --features app
PKG_CONFIG_PATH=/path/to/fcitx5-dev cargo check --no-default-features --features fcitx5
```

Build `--all-features` và `--features fcitx5` cần Fcitx5 dev headers
(`libfcitx5core-dev`, `libfcitx5utils-dev`, `libfcitx5config-dev`) hoặc
set `PKG_CONFIG_PATH`.

## Kiến trúc

* Đúng một Cargo package, một library crate + một binary `cantype`. Không
  workspace, không subcrate.
* Chỉ `src/cadence.rs` được phép `use cadence::...`. Các module khác không
  import kiểu Cadence.
* Feature `app` (default): GUI Slint + tray. Feature `fcitx5`: addon `.so`.
  `.so` không link Slint; app không cần Fcitx5 dev headers.
* Không thêm module/tầng thư mục nếu chưa có code thật cần đặt.

## Cadence

Pin theo full commit SHA trong `Cargo.toml`, không theo branch. Hiện pin
`v2026.1.0` (peeled `a5a586334208a4c084e062f1d77657b07ec3d580`). Tag
`v2026.0.1` trong spec không tồn tại; `v2026.1.0` là bản khớp spec. Xem
`docs/PHASE_1_RUNTIME.md` cho API Cadence đang dùng. Không commit
`path = "../Cadence"` hay `.cargo/config.toml` chứa đường dẫn local.

## MSRV

CanType MSRV = Rust 1.92 (do GUI Slint ~1.17). Lõi runtime thuần tương thích
1.85 (đồng bộ Cadence), nhưng package cần 1.92 vì feature `app` default.

## Phase 1

Phase 1 là runtime thuần Rust, độc lập nền tảng. Xem `docs/PHASE_1_RUNTIME.md`
cho ranh giới Phase 1 và tiêu chí bước sang Phase 2.

## Phase 2

Phase 2 tích hợp Fcitx5 (Linux) + GUI/tray Slint. Lifecycle đầy đủ
(activate/deactivate/reset/focus-in/focus-out/switch/destroy). Cấu hình
chung schema versioned. Developer install qua `scripts/`. Xem
`docs/PHASE_2_FCITX5.md`. Chưa: IBus, Windows TSF, macOS, packaging
system-wide, global hotkey, auto-repair.

## Phase 3

Phase 3 mở rộng tương thích Fcitx5 (KHÔNG packaging, KHÔNG preedit fallback,
KHÔNG uinput). Diagnostic `src/tuong_thich.rs` (feature `diag` + env
`CANTYPE_DEBUG`) log metadata KHÔNG chứa text user. Route B
(`ContextKeyReplace` = `forwardKey(Backspace)` + `commitString`) REJECTED —
cursor invalidation contract yếu (GTK4/Qt/XIM/LibreOffice không reset khi
click/undo/autocomplete). Test `tests/compatibility.rs` pin invariant. Xem
`docs/PHASE_3_COMPATIBILITY.md` + `docs/COMPATIBILITY.md`. Chưa runtime-verify:
Wayland, XIM, GTK3/4 text field harness riêng. Chuyển Phase 4.
