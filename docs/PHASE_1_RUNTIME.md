# CadenceRuntime Phase 1 - thiết kế runtime

Tài liệu này mô tả thiết kế Phase 1 của CadenceRuntime, phản ánh code thật
trong `src/`. Không phải kiến trúc tưởng tượng.

## 1. Mục tiêu

Phase 1 dựng **nền móng bất biến** của runtime thuần Rust, độc lập nền tảng:

* Phiên nhập độc lập per-context, không state global.
* Tích hợp Cadence qua một boundary duy nhất (`cadence.rs`).
* Kế hoạch sửa committed text an toàn Unicode (byte/UTF-16/codepoint).
* Host abstraction tối thiểu và host mô phỏng để test.
* Zero-preedit: mọi chữ đi qua committed `Chen` hoặc `ThayThe`, không vẽ chữ tạm.
* Verify-before-mutate: không xóa dựa state cũ khi context/focus/surrounding lệch.
* State Cadence mới chỉ được chấp nhận khi host xác nhận `DaPhat`.
* Host uncertain (`KhongChac`) làm phiên mất đồng bộ an toàn.

Mục tiêu lớn hơn: runtime nhỏ đến mức một maintainer đọc hết trong một buổi, nhưng
đủ chặt để Phase 2 chỉ cần thêm adapter Fcitx5 mà không đập lõi.

## 2. Non-goals (cố tình chưa làm)

Phase 1 **không** triển khai:

* Fcitx5, IBus, Wayland, XIM, Windows TSF, macOS IMK, uinput, evdev, libinput.
* GUI, tray icon, CLI installer, đóng gói `.deb`/RPM/Arch/Nix.
* Nhận diện ứng dụng, app profile, workaround Firefox/LibreOffice/Minecraft.
* Async runtime, thread nền, IPC, D-Bus, telemetry.
* Test trên ứng dụng thật - Phase 1 chỉ test qua host mô phỏng.

## 3. Sơ đồ luồng dữ liệu

```text
SuKienNhap
  │
  ▼
PhienNhap::xu_ly(host)
  │
  ├─ host.boi_canh() → BoiCanhNhap
  │     ├─ context_id, the_he_focus, dang_co_focus
  │     └─ van_ban_truoc_con_tro: Option<String>
  │
  ├─ verify focus / context / surrounding
  │     da_hien_thi rỗng + Chen → cho phép (composition mới)
  │     da_hien_thi không rỗng + None/mismatch → relinquish + ChuyenTiep
  │     da_hien_thi không rỗng + khớp → cho phép Chen và ThayThe
  │
  ├─ PhienCadence::them_ky_tu / xoa_lui  (boundary, không lộ kiểu Cadence)
  │     ↓
  ├─ PhienCadence::ban_chup() → ChupBan { noi_dung }
  │
  ├─ KeHoachSua::tinh(da_hien_thi, noi_dung_moi)
  │     ├─ common prefix tại ranh giới char
  │     ├─ xoa_truoc: DoDaiVanBan (byte/utf16/codepoint)
  │     └─ chen: String
  │
  ├─ HanhDong { Chen | ThayThe | ChuyenTiep }   (không có nhánh preedit)
  │
  ├─ host.thuc_thi(hanh_dong) → KetQuaHost
  │
  └─ match KetQuaHost:
        DaPhat    → chấp nhận state, da_hien_thi = moi, ghi lich_su
        KhongPhat  → quay lui Cadence (replay lich_su), forward sự kiện
        KhongChac   → reset, MatDongBo, không delete dựa state cũ
```

## 4. Trách nhiệm từng module

CadenceRuntime là đúng một library crate, bốn module:

| Module | Trách nhiệm |
|---|---|
| `cadence` | Anti-corruption boundary. Module duy nhất `use cadence::...`. Tạo phiên Cadence, chuyển sự kiện trung lập thành lời gọi Cadence, đọc snapshot rendered, reset phiên. Bọc `PhienGo` thành `PhienCadence`, `BanChupSoan` thành `ChupBan`, `KetQuaXuLy` thành `KetQuaCadence`. |
| `sua` | Kế hoạch sửa committed text. `KeHoachSua::tinh(cu, moi)` tính common prefix tại ranh giới Unicode, trả `xoa_truoc` + `chen`. `DoDaiVanBan` tính byte/UTF-16/codepoint. |
| `host` | Host abstraction. `Host` trait, `BoiCanhNhap`, `HanhDong`, `KetQuaHost`, `ContextId`. Đây là seam Phase 2 adapter triển khai. |
| `phien` | Phiên nhập. `PhienNhap` per-context, `SuKienNhap` semantic, `TrangThaiPhien` (Rong/DangGo/MatDongBo), `KetQuaXuLy`. State machine tối thiểu, replay lịch sử để quay lui. |

Không có `platforms/`, `adapters/`, `services/`, `domain/` hay tầng thư mục
khác. Module chỉ thêm khi có code thật.

## 5. Bất biến zero-preedit

`HanhDong` có đúng ba biến thể:

```rust
pub enum HanhDong {
    Chen(String),
    ThayThe(KeHoachSua),
    ChuyenTiep,
}
```

Không tồn tại `Preedit`, `ClientPreedit`, `CapNhatPreedit` hay `CommitPreedit`.
Mọi chữ user nhìn thấy đều đi qua `Chen` (insert committed) hoặc `ThayThe`
(replace committed suffix). `ChuyenTiep` báo host chuyển tiếp phím gốc, không
chèn/replace gì.

Bất biến được test hành vi (`tests/runtime.rs`):

* Mọi action trong history host là `Chen`/`ThayThe`/`ChuyenTiep` (enum không có
  nhánh khác, match đầy đủ trong test).
* Một sự kiện tạo tối đa một lời gọi `host.thuc_thi` (`lich_su_hanh_dong.len() <= so_su_kien`).
* `ThayThe` không xóa vượt `da_hien_thi`: sau mỗi `DaApDung`, `host.van_ban`
  kết thúc bằng `phien.da_hien_thi()`.
* `xoa_truoc.byte_utf8` luôn <= `da_hien_thi` tại thời điểm tính (xem test
  `xoa_khong_vuot_doan_runtime_so_huu` ở `tests/sua.rs`).

## 6. Host outcome semantics

Bất biến tối quan trọng: **mỗi input event xuất hiện tối đa một lần trong ứng
dụng**. Runtime và adapter Phase 2 cùng bảo đảm:

```text
một sự kiện → tối đa một host.thuc_thi → tối đa một lần xuất hiện
```

Không được tồn tại đường "host đã nhận một phần → runtime lại forward event gốc".

`KetQuaHost` phân biệt ba kết quả:

```rust
pub enum KetQuaHost {
    DaPhat,
    KhongPhat,
    KhongChac,
}
```

| Kết quả | Semantics | Runtime xử lý | Adapter |
|---|---|---|---|
| `DaPhat` | Host đã phát toàn bộ action theo đúng thứ tự vào nền tảng. Không có app ACK. | Chấp nhận state Cadence mới (có điều kiện). `da_hien_thi = noi_dung_moi`. Ghi sự kiện vào `lich_su`. `trang_thai = Rong` nếu Cadence rỗng, `DangGo` nếu còn. | **Không** chuyển tiếp phím gốc. Text đã được phát đúng một lần. |
| `KhongPhat` | Host chắc chắn chưa phát lệnh text mutation nào. | Không chấp nhận state mới. Quay lui Cadence về trước sự kiện qua `xay_lai_cadence(lich_su)`. `da_hien_thi` và `lich_su` giữ nguyên. Không retry destructive. | **Chuyển tiếp** phím gốc. Sự kiện chưa xuất hiện, forward đúng một lần. |
| `KhongChac` | Không biết ứng dụng nhận một phần hay toàn bộ. | `cadence.dat_lai()`, xóa `da_hien_thi`, xóa `lich_su`, `trang_thai = MatDongBo`. Không delete dựa state cũ. Không replay mù phím đã xử lý. | **Không** chuyển tiếp phím gốc. Sự kiện có thể đã xuất hiện (tối đa một lần). |

Runtime không bao giờ coi timeout/lỗi không rõ là `DaPhat`. Runtime không retry
destructive action sau `KhongPhat`, và không replay mù phím đã xử lý sau
`KhongChac`. Fake host (`tests/common/mod.rs`) phân biệt ba kết quả và mô phỏng
cả trường hợp `KhongChac` có áp dụng lẫn không áp dụng.

## 7. Verify-before-mutate

Khi runtime đang sở hữu suffix (`da_hien_thi` không rỗng), `xu_ly` kiểm tra
trước khi gửi **bất kỳ** action nào (cả `Chen` lẫn `ThayThe`):

1. `boi_canh.context_id == self.context_id` - còn đúng context.
2. `boi_canh.dang_co_focus` - context còn focus.
3. `boi_canh.the_he_focus == self.the_he_focus` - focus generation chưa đổi.
4. `boi_canh.van_ban_truoc_con_tro` phải là `Some` và kết thúc bằng
   `da_hien_thi` - surrounding text khớp (cursor ngay sau composition).

Bất kỳ điều kiện nào sai:

* Không tạo delete, không chèn, không xóa thử, không đoán số ký tự.
* `relinquish()`: đặt lại Cadence, xóa `da_hien_thi`, xóa `lich_su`, về `Rong`.
* Trả `ChuyenTiep` (chuyển tiếp sự kiện gốc).
* Sai lệch của một context không ảnh hưởng context khác (mỗi `PhienNhap` riêng).

Lý do verify cả `Chen`: khi runtime đang sở hữu suffix, `Chen` chèn text vào
vị trí cursor hiện tại. Nếu cursor đã lệch (surrounding mismatch hoặc `None`),
chèn sai vị trí rồi cập nhật `da_hien_thi` → state lệch, phím sau mới gây lỗi
nặng (đầu độc state). Chỉ khi `da_hien_thi` rỗng (composition mới, phím đầu
tiên), `Chen` thuần an toàn không cần surrounding vì không sở hữu suffix cũ.

`van_ban_truoc_con_tro` là `Option<String>`: nhiều host thật không cung cấp
surrounding (game, terminal). Khi `None`:

* **`da_hien_thi` rỗng + Chen** → cho phép (phím đầu vào composition mới).
* **`da_hien_thi` không rỗng + bất kỳ action** → relinquish + `ChuyenTiep`.
* Không đoán text, không gửi Backspace giả, không coi `None` là "optimistic
  success".

Phase 1 chỉ cho destructive replace (và tiếp tục composition) khi host cung
cấp surrounding text đủ để verify. Phase 2 tinh chỉnh theo capability host.

Phase 1 verify cursor **gián tiếp** qua `van_ban_truoc_con_tro`: không có
trường `vi_tri_con_tro` riêng trong contract. Nếu surrounding kết thúc bằng
`da_hien_thi`, con trỏ được coi là ngay sau composition. Cursor di chuyển
phát hiện qua mismatch surrounding (khi host cung cấp `Some`), hoặc qua sự
kiện `DiChuyenConTro` (explicit relinquish). Không tuyên bố verify cursor
tuyệt đối.

## 8. Giữ state Cadence chưa commit cho tới khi host thành công

`PhienCadence` không `Clone` (vì `PhienGo` của Cadence không `Clone`). Để không
advance state khi host chưa thành công, runtime dùng **replay lịch sử**:

* `PhienNhap` giữ `lich_su: Vec<SuKienNhapDaChapNhan>` - các sự kiện `KyTu`/`XoaLui`
  đã được host chấp nhận.
* Khi áp dụng một sự kiện mới, runtime mutate `PhienCadence` tại chỗ, tính action,
  gửi host.
* Nếu host `KhongPhat`: runtime dựng lại `PhienCadence` qua
  `xay_lai_cadence(&lich_su)` (replay các sự kiện đã chấp nhận, không kể sự kiện
  thất bại). State quay về điểm trước sự kiện. Sự kiện thất bại không vào
  `lich_su`.
* Nếu host `DaPhat`: sự kiện được push vào `lich_su`. State mới được giữ.
* Nếu host `KhongChac`: `dat_lai()` Cadence, xóa `lich_su`. Mất đồng bộ an toàn.

Lịch sử chỉ chứa `KyTu`/`XoaLui` (xây composition). Các sự kiện relinquish
(`RanhGioiTu`, `DiChuyenConTro`, `DatLai`) xóa `lich_su` nên không bao giờ xuất
hiện trong đó.

Cách này không sửa Cadence (không cần trait `Clone` hay transaction). Chỉ cần
`PhienCadence::moi()` + `them_ky_tu`/`xoa_lui` đã có.

Replay chỉ chạy trên đường thất bại (`KhongPhat`), không phải mỗi phím. Hot
path `DaApDung` là O(1) — chỉ push vào `lich_su`. Lịch sử được cắt tại
`RanhGioiTu`/`DiChuyenConTro`/`DatLai`/`KhongChac`/focus change, và giới hạn
256 sự kiện (`GIOI_HAN_LICH_SU`) để chống input độc hại. Khi `lich_su` vượt
giới hạn, runtime relinquish (composition đã commit trong host, runtime chỉ
ngừng theo dõi suffix).

## 9. Cách CadenceRuntime pin Cadence

`Cargo.toml`:

```toml
[dependencies]
cadence = {
    package = "cadence-ime",
    git = "https://github.com/LumeWorks/Cadence",
    rev = "52b3bb49403245e68a5f5291b04741953fd666b6",
}
```

* Pin theo **full commit SHA**, không theo branch. Cadence đang phát triển song
  song và `main` có thể thay đổi bất kỳ lúc nào.
* Tên package `cadence-ime` lấy từ `Cargo.toml` thật của Cadence, lib name
  `cadence`.
* `Cargo.lock` được commit để khóa transitively dependencies.
* SHA `52b3bb4...` là HEAD của Cadence tại thời điểm triển khai Phase 1, working
  tree sạch, build xanh, có Telex engine đầy đủ và phân đoạn ngữ cảnh (Phase 3).

API Cadence đang dùng (chỉ `cadence.rs` biết):

* `BoGo::new(CauHinh::mac_dinh()) -> Result<BoGo, LoiCauHinh>`
* `BoGo::tao_phien() -> PhienGo`
* `PhienGo::them_ky_tu(char) -> KetQuaXuLy`
* `PhienGo::xoa_lui() -> KetQuaXuLy`
* `PhienGo::dat_lai()`
* `PhienGo::ban_chup() -> &BanChupSoan`
* `PhienGo::dang_trong() -> bool`
* `BanChupSoan::noi_dung() -> &str`

Nếu Cadence thay đổi API, mục tiêu là chỉ cần sửa `cadence.rs`.

## 10. Giới hạn hiện tại

* **Chưa test trên ứng dụng thật.** Mọi kiểm chứng qua `HostMoPhong`. UX thực
  (không mất chữ khi gõ nhanh, không nhân đôi ký tự) chưa thể chứng minh ở Phase 1.
* **Surrounding `None` chặn ThayThe.** Host không cung cấp surrounding thì
  runtime chỉ cho phép `Chen` (insert thuần), chặn `ThayThe` (destructive
  replace). Telex transformation cần surrounding để verify. Phase 2 cần
  capability negotiation hoặc adapter tự theo dõi cursor.
* **MatDongBo phục hồi thụ động.** Sau `KhongChac`, runtime về `Rong` khi
  `da_hien_thi` rỗng và sự kiện kế tiếp verify-passing. Không có cơ chế chủ động
  reconcicle văn bản thật.
* **Cadence rebuild O(n) mỗi phím.** Cadence nội bộ dựng lại snapshot từ lịch
  sử sau mỗi `them_ky_tu`/`xoa_lui` (thiết kế Cadence, không phải runtime).
  Composition dài → chi phí tăng. Runtime replay (`xay_lai_cadence`) chỉ chạy
  khi `KhongPhat`, không phải mỗi phím. Lịch sử bị cắt bởi `DatLai`/relinquish
  thường xuyên trong thực tế và giới hạn 256 sự kiện để chống input độc hại.
* **Chưa có `SessionManager`.** Phase 1 một `PhienNhap` per context; adapter tự
  giữ map context → `PhienNhap`. Test tạo hai `PhienNhap` để chứng minh độc lập.
* **`RanhGioiTu` chèn ký tự raw.** Ký tự ranh giới (space) đi qua `Chen` như văn
  thường, không qua Cadence. Phase 2 có thể tinh chỉnh nếu cần.

## 11. Tiêu chí bước sang Phase 2

Phase 2 bắt đầu khi:

* Lõi runtime Phase 1 xanh trên CI (fmt, clippy, test, release, doc, MSRV 1.85).
* Adapter Fcitx5 có thể triển khai `Host` trait mà không đập `PhienNhap`/`sua`/`cadence`.
* Boundary `cadence.rs` đủ bao thay đổi API Cadence mà không lan module khác.

Hạng mục đầu tiên Phase 2: **adapter Fcitx5** - triển khai `Host`, chuyển
`FcitxKey` event thành `SuKienNhap`, mapping surrounding text, và xử lý focus
generation thực.
