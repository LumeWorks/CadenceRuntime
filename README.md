# CanType

CanType là bộ gõ tiếng Việt cho Linux và Windows, dùng chung lõi
[Cadence](https://github.com/LumeWorks/Cadence).

```text
CanType
Vietnamese typing for Linux and Windows
Powered by Cadence
```

Trên Linux, người dùng chỉ nhìn thấy một ứng dụng `cantype`. Addon Fcitx5
(`libcantype.so`) là nội tạng để Fcitx5 nạp; không có executable riêng cho tray
hay GUI.

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

## Phase 2

Phase 2 tích hợp Fcitx5 (Linux) và thêm GUI/tray `cantype`:

* Addon Fcitx5 zero-preedit, lifecycle đầy đủ (activate/deactivate/reset/
  focus-in/focus-out/switch/destroy/restart).
* GUI Slint + tray riêng (trạng thái V/E), cấu hình chung với addon.
* Tách feature sạch: `app` (GUI) và `fcitx5` (addon) không kéo dependency của
  nhau — `.so` không link Slint, app không cần Fcitx5 dev headers.

Chưa làm ở Phase 2: IBus, Windows TSF, macOS, uinput, packaging system-wide,
global hotkey, auto-repair. Xem `docs/PHASE_2_FCITX5.md`.

## Kiến trúc

CanType là đúng **một Cargo package và một library crate**. Module chính:

* `cadence` - anti-corruption boundary, module duy nhất biết API Cadence.
* `sua` - kế hoạch sửa committed text (common-prefix diff an toàn Unicode).
* `host` - host abstraction (`Host`, `BoiCanhNhap`, `KetQuaHost`, `HanhDong`).
* `phien` - phiên nhập (`PhienNhap`, `SuKienNhap`, `TrangThaiPhien`).
* `fcitx5` - adapter Fcitx5 (key mapping, surrounding, ABI types).
* `ffi` - ranh giới FFI với C++ shim (module duy nhất dùng `unsafe`).

## Cadence

CanType pin Cadence theo **full commit SHA** của `v2026.1.0`
(`a5a586334208a4c084e062f1d77657b07ec3d580`), không theo branch, vì Cadence đang
phát triển song song. Spec Phase 2 ghi `v2026.0.1` nhưng tag đó không tồn tại
trên `LumeWorks/Cadence` (chỉ có `v0.1.0` và `v2026.1.0`); `v2026.1.0` là bản phát
hành công khai đầu tiên khớp mọi tham chiếu (Telex + VNI + code/chat preservation
+ NFC/NFD). Xem `Cargo.toml` cho dependency. Chỉ `src/cadence.rs` được phép
import Cadence.

## MSRV

Rust 1.92 (do GUI Slint ~1.17). Lõi runtime thuần tương thích 1.85 (đồng bộ
Cadence), nhưng package cần 1.92 vì feature `app` default.

## Tài liệu thiết kế

Xem [`docs/PHASE_1_RUNTIME.md`](./docs/PHASE_1_RUNTIME.md) cho Phase 1 và
[`docs/PHASE_2_FCITX5.md`](./docs/PHASE_2_FCITX5.md) cho Phase 2: mục tiêu,
non-goals, sơ đồ luồng dữ liệu, trách nhiệm module, bất biến zero-preedit, host
outcome semantics, verify-before-mutate, lifecycle Fcitx5 và tiêu chí audit.

## Chạy kiểm tra

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo check --release
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

Build feature `fcitx5` cần Fcitx5 dev headers (`libfcitx5core-dev`,
`libfcitx5utils-dev`) hoặc set `PKG_CONFIG_PATH`.

## Giấy phép

MPL-2.0. Xem [`LICENSE`](./LICENSE).

## Credit

Copyright (c) 2026 Lê Hùng Quang Minh.
