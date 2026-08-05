# CadenceRuntime

CadenceRuntime là runtime thuần Rust, độc lập nền tảng cho lõi gõ tiếng Việt
[Cadence](https://github.com/LumeWorks/Cadence).

Runtime chịu trách nhiệm:

```text
nhận sự kiện nhập liệu
→ đưa vào Cadence
→ so sánh nội dung đã hiển thị với nội dung Cadence mới
→ tạo hành động sửa văn bản
→ thực thi qua host của nền tảng
```

## Phase 1

Phase 1 dựng **nền móng bất biến** của runtime:

* Phiên nhập độc lập theo context, không state global.
* Tích hợp Cadence qua một boundary duy nhất (`cadence.rs`).
* Kế hoạch sửa committed text an toàn Unicode (byte/UTF-16/codepoint).
* Host abstraction tối thiểu và host mô phỏng để test.
* Zero-preedit: mọi chữ đều đi qua committed insert hoặc committed replace,
  không bao giờ vẽ chữ tạm.
* Verify-before-mutate: không xóa dựa trên state cũ khi context/focus/surrounding
  text không khớp.
* State Cadence mới không được chấp nhận trước khi host áp dụng thành công.
* Host không chắc chắn làm phiên mất đồng bộ an toàn.

### Chưa làm được ở Phase 1

Phase 1 **chưa** triển khai và cố tình chưa làm:

* Fcitx5, IBus, Wayland, XIM, Windows TSF, macOS IMK, uinput, evdev.
* GUI, tray icon, CLI installer, đóng gói.
* Nhận diện ứng dụng, app profile, workaround riêng cho app.
* Async runtime, thread nền, IPC, D-Bus, telemetry.
* Test trên ứng dụng thật — Phase 1 chỉ test qua host mô phỏng.

Project **chưa sẵn sàng cho end user** ở Phase 1.

## Kiến trúc

CadenceRuntime là đúng **một Cargo package và một library crate**. Module:

* `cadence` — anti-corruption boundary, module duy nhất biết API Cadence.
* `sua` — kế hoạch sửa committed text (common-prefix diff an toàn Unicode).
* `host` — host abstraction (`Host`, `BoiCanhNhap`, `KetQuaHost`, `HanhDong`).
* `phien` — phiên nhập (`PhienNhap`, `SuKienNhap`, `TrangThaiPhien`).

## Cadence

CadenceRuntime pin Cadence theo full commit SHA, không theo branch. Xem
`Cargo.toml` và `docs/PHASE_1_RUNTIME.md` cho SHA đang pin.

## Chạy test

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo check --release
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

## Giấy phép

MPL-2.0. Xem [`LICENSE`](./LICENSE).

## Credit

Copyright (c) 2026 Lê Hùng Quang Minh.
