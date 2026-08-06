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
cargo check --release
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo +1.85 check --all-targets   # MSRV
```

## Kiến trúc

* Đúng một Cargo package và một library crate. Không workspace, không subcrate.
* Chỉ `src/cadence.rs` được phép `use cadence::...`. Các module khác không import
  kiểu Cadence.
* Không thêm module/tầng thư mục nếu chưa có code thật cần đặt.

## Cadence

Pin theo full commit SHA trong `Cargo.toml`, không theo branch. Xem
`docs/PHASE_1_RUNTIME.md` cho SHA hiện tại và API Cadence đang dùng. Không commit
`path = "../Cadence"` hay `.cargo/config.toml` chứa đường dẫn local.

## Phase 1

Phase 1 là runtime thuần Rust, độc lập nền tảng. Chưa tích hợp Fcitx5/IBus/
Wayland/Windows/GUI/uinput. Xem `docs/PHASE_1_RUNTIME.md` cho ranh giới Phase 1
và tiêu chí bước sang Phase 2.
