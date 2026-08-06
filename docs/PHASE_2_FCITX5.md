# CanType Phase 2 - RFC tích hợp Fcitx5

Tài liệu khởi đầu cho Phase 2. Mô tả mục tiêu, ranh giới, và blocker thiết kế
quan trọng nhất trước khi triển khai. Phiên bản đầy đủ (sơ đồ, ownership,
mapping, test matrix, ...) được cập nhật ở commit cuối Phase 2.

## 1. Mục tiêu

Tạo một addon Fcitx5 thực sự có thể:

```text
Fcitx5 KeyEvent
→ chuyển thành SuKienNhap
→ PhienNhap xử lý bằng Cadence
→ FcitxHost thực thi Chen hoặc ThayThe
→ committed text xuất hiện trong ứng dụng
```

## 2. Ranh giới

Phase 2 triển khai: addon Fcitx5, C++ shim mỏng, Rust FFI boundary, `FcitxHost`
triển khai `Host`, state riêng theo `InputContext`, ánh xạ `KeyEvent` sang
`SuKienNhap`, committed insert, native committed replacement bằng surrounding
text, focus/reset/deactivate lifecycle, user-local install/uninstall, automated
tests, smoke test addon, kiểm thử thực tế.

Phase 2 KHÔNG triển khai: IBus, Wayland-native, XIM, Windows TSF, GUI config,
tray icon, D-Bus daemon, IPC, uinput, evdev/libinput, global keyboard hook,
synthetic Backspace replacement, adaptive routing, app profile, Firefox/
LibreOffice hardcode, telemetry, async runtime, thread nền, plugin framework,
workspace, subcrate, packaging chính thức.

## 3. Blocker thiết kế quan trọng nhất: Fcitx không có ACK ứng dụng

Các lời gọi Fcitx như `inputContext->commitString(text)` hay
`inputContext->deleteSurroundingText(offset, size)` không trả kết quả từ ứng
dụng. Host không thể xác nhận ứng dụng đã áp dụng action.

Contract Phase 1 dùng tên `DaApDung`/`KhongApDung`/`KhongChac`. Tên `DaApDung`
mô tả "đã áp dụng" — không trung thực khi host Fcitx không có app-level ACK.

### Contract mục tiêu

Đổi tên cho trung thực về "đã phát" chứ không "đã áp dụng":

```rust
pub enum KetQuaHost {
    /// Host đã phát toàn bộ logical action theo đúng thứ tự vào nền tảng.
    /// Không có nghĩa ứng dụng đã ACK.
    DaPhat,
    /// Host chắc chắn chưa phát bất kỳ phần nào của action.
    KhongPhat,
    /// Có khả năng chỉ một phần action đã được phát.
    KhongChac,
}
```

Runtime semantics:

* `DaPhat`: toàn bộ lệnh đã gửi vào Fcitx theo đúng thứ tự. Runtime giữ state
  mới **có điều kiện**. Trước mọi action tiếp theo khi `da_hien_thi` không rỗng,
  surrounding phải được verify lại. Không mô tả là app đã ACK.
* `KhongPhat`: không lệnh text mutation nào được gửi. Runtime rollback Cadence.
  Adapter để phím gốc đi qua đúng một lần.
* `KhongChac`: có khả năng một phần action đã được gửi. Runtime reset/mất đồng bộ.
  Adapter không forward phím gốc. Không rollback bằng thao tác text đoán mò.

Đổi tên commit riêng, cập nhật toàn bộ test Phase 1, không làm yếu
verify-before-mutate, không thêm state machine lớn hay pending transaction.

## 4. Giữ đúng một crate

CanType tiếp tục: một repository, một `Cargo.toml`, một package, một
library crate. Không workspace, không `crates/`, không subcrate C++ riêng.

Cấu trúc mục tiêu:

```text
src/{lib,cadence,host,phien,sua,fcitx5,ffi}.rs
native/{fcitx5.cpp,fcitx5_ffi.h}
data/{cantype-addon.conf,cantype-inputmethod.conf}
scripts/{install-user.sh,uninstall-user.sh,run-fcitx-dev.sh}
```

## 5. Kiến trúc addon

C++ shim mỏng (mục tiêu `<= ~500 dòng`): định nghĩa `InputMethodEngineV2`,
đăng ký addon factory, đăng ký `InputContextProperty`, nhận lifecycle callback,
đọc `KeyEvent`/`InputContext`, chuyển dữ liệu qua C ABI, gọi commit/delete khi
Rust yêu cầu, accept hoặc pass-through event.

Rust runtime giữ `PhienNhap`, Cadence, `Host` contract và mọi invariant. C++
không chứa luật Telex, state Cadence, common-prefix diff, history replay, hay
heuristic tiếng Việt.

## 6. FFI policy

Phase 1 đang `forbid unsafe`. Phase 2 thay đổi tối thiểu: `unsafe_code = "deny"`,
chỉ `src/ffi.rs` được `#![allow(unsafe_code)]`. Mỗi unsafe block có `// SAFETY:`.
C ABI nhỏ, POD-only. Không truyền `String`/`Vec`/Rust enum không `repr`/reference/
trait object/panic/C++ exception qua ABI. Rust function xuất dùng `catch_unwind`,
panic không vượt FFI.

## 7. Thứ tự ưu tiên khi đánh đổi

1. không phá input
2. không duplicate hoặc nuốt phím
3. contract trung thực
4. zero-preedit
5. focus/context safety
6. một crate nhỏ, dễ audit
7. test tự động
8. UX trên app hỗ trợ surrounding
9. hiệu năng
10. độ phủ app

## 8. Developer install (Phase 2)

Phase 2 cài addon vào thư mục user (`~/.local`) qua `scripts/install-user.sh`.
System-wide packaging (`/usr/lib/...`, `.deb`, RPM, Nix) chuyển Phase 3.

Artifact CanType:

```text
~/.local/lib/fcitx5/libcantype.so
~/.local/share/fcitx5/addon/cantype.conf
~/.local/share/fcitx5/inputmethod/cantype.conf
```

Fcitx5 không quét `~/.local/lib/fcitx5/` mặc định — developer phải set
`FCITX_ADDON_DIRS=~/.local/lib/fcitx5` (script `run-fcitx-dev.sh` làm việc này).

### Cleanup artifact CadenceRuntime cũ

Dự án trước đây tên `CadenceRuntime`; artifact cũ có thể còn trong `~/.local`:

```text
~/.local/lib/fcitx5/cadence_runtime.so        (crate output cũ, không prefix lib)
~/.local/lib/fcitx5/libcadence_runtime.so     (dạng prefix lib, nếu có)
~/.local/share/fcitx5/addon/cadence-runtime.conf
~/.local/share/fcitx5/inputmethod/cadence-runtime.conf
```

`scripts/uninstall-user.sh --cleanup-old` và `scripts/install-user.sh
--cleanup-old` xóa các file này trước khi cài CanType. Chỉ xóa file có tên
chính xác do dự án cũ tạo — không glob rộng, không xóa addon khác (vd
`unilume.conf`, `lotus.conf` giữ nguyên).
