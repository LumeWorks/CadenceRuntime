# CanType Phase 3 — mở rộng tương thích Fcitx5

Tài liệu Phase 3, phản ánh code và khảo sát thật (Fcitx 5.1.12, KDE X11). Không
phải kiến trúc tưởng tượng. Phase 3 **không phải packaging** — trả lời "cơ chế
gõ của CanType có bao phủ các frontend phổ biến không" trước khi đóng gói.

## 1. Mục tiêu

Mở rộng khả năng gõ tiếng Việt của CanType trên nhiều nhóm ứng dụng Linux thực
tế hơn (browser, office, GTK, Chromium/Electron, editor/IDE, terminal, X11,
XWayland, Wayland native) **KHÔNG phá hỏng invariant an toàn Phase 1/2**.

Thứ tự ưu tiên (§1 spec): không phá input > không xóa nhầm > không duplicate >
zero-preedit > focus correctness > compatibility > performance > số app.

## 2. Non-goals (cố tình chưa làm)

Phase 3 **không** triển khai (chuyển Phase 4): `.deb`/RPM/Arch/Nix package,
installer GUI, auto-repair system-wide, package repository, updater, IBus
engine, Windows TSF, native macOS, native Wayland input-method addon riêng,
global hotkey. Wayland trong Phase 3 = khảo sát/test Fcitx5 frontend Wayland
hiện có, không viết Wayland IME mới.

## 3. Baseline Phase 2

Phase 2 (tag `phase-2`, merged): runtime chung, Cadence boundary, addon Fcitx5,
zero-preedit, NativeReplace bằng surrounding text, InputContextProperty
per-context, lifecycle focus/reset/deactivate, FFI boundary, GUI/tray, config.

Baseline đã xác nhận (Phase 2 + Phase 3 trace): Kate/Qt `as→á`, `vowsi→với`;
Kate có surrounding hợp lệ từ phím thứ hai → Route A (NativeReplace) hoạt động.
LibreOffice GTK3: capability có SurroundingText nhưng `surroundingText().
isValid() == false` mọi phím → NativeReplace không khả dụng → passthrough an
toàn. Delete có semantic riêng (relinquish + passthrough, không thành Backspace,
không commit U+007F).

## 4. Frontend taxonomy

Phân loại theo `ic->frontend()` (Fcitx API, không đoán app name). Nguồn: Fcitx
5.1.12 `src/frontend/` + `src/lib/fcitx/inputcontext.{h,cpp}`.

| Frontend | `frontend()` | Surrounding | deleteSurrounding | forwardKey | Toolkit path |
|----------|--------------|-------------|-------------------|------------|--------------|
| dbus | `"dbus"` | client D-Bus (SetSurroundingText) | real send | real send (IgnoredMask GTK) | GTK3/4, Qt5/6, Electron qua fcitx5-gtk/fcitx5-qt |
| wayland v1 | `"wayland"` | text-input-v1 | real (char→byte) | real | Wayland text-input v1 |
| wayland v2 | `"wayland_v2"` | text-input-v2 | real (reject offset>0) | real (press+auto-release) | Wayland text-input v2 |
| xim | `"xim"` | KHÔNG hỗ trợ | **no-op** | real (xcb_im_forward_event) | X11 raw (XMODIFIERS, không IM module) |
| ibus | `"ibus"` | IBus capability | real | real (ForwardKeyEvent) | IBus protocol (Chromium default Linux) |
| fcitx4 | `"fcitx4"` | client D-Bus | real | real | Legacy fcitx4 |

Máy test (KDE X11): mọi app = `dbus` (GTK_IM_MODULE=fcitx, QT_IM_MODULE=fcitx).

## 5. Capability matrix (trace `cantype diag`)

Diagnostic (`src/tuong_thich.rs`, feature `diag` + env `CANTYPE_DEBUG`) log
metadata KHÔNG chứa text user (chỉ surrounding length, suffix match boolean,
capability flags, cursor/anchor offset). Trace runtime:

| App | `sur_cap` | `sur_valid` | `route` | Vietnamese |
|-----|-----------|-------------|---------|------------|
| Kate (Qt5) | 1 | 0→1 (phím 2+) | native (ThayThe) | SUPPORTED |
| plasmashell (Qt5) | 1 | 0→1 | native | SUPPORTED |
| LibreOffice Writer (GTK3) | 1 | 0 (mọi phím) | passthrough | UNSUPPORTED |
| Google Chrome (URL bar) | 0 | 0 | passthrough | UNSUPPORTED |
| VS Code (Electron editor) | 0 | 0 | passthrough | UNSUPPORTED |
| Konsole (terminal) | 1 | 0 | passthrough | UNSUPPORTED |

`pw`/`sens` = 0, `term` = 0 cho mọi context test (Konsole không set Terminal
flag qua dbus Qt IM module). `preedit` = 1 (capability có, CanType không dùng).

## 6. Route A — NativeReplace (route hiện tại)

Pseudo (giữ nguyên Phase 2, không nới):

```text
if !focus: passthrough
if generation mismatch: relinquish; passthrough
if surrounding invalid: route A unavailable → passthrough
if selection: route A unavailable → passthrough
if suffix mismatch: relinquish; passthrough
else: deleteSurroundingText; commitString
```

Invariant (Phase 1/2, giữ nguyên): `surrounding == None` hoặc invalid hoặc
suffix mismatch KHÔNG BAO GIỜ coi là "chắc text vẫn đúng" (`None => false`
trong `khop_surrounding`). Không xóa committed text bằng NativeReplace nếu
không verify được suffix. Test pin: `tests/compatibility.rs`, `tests/focus.rs`.

Route A hoạt động với Kate/plasmashell (Qt text widget report `sur_valid=1`
từ phím thứ 2). KHÔNG hoạt động với LibreOffice/Chrome/VS Code/Konsole
(`sur_valid=0`/`sur_cap=0`) → passthrough an toàn.

## 7. Route B — ContextKeyReplace (NGHIÊN CỨU, REJECTED)

### Ý tưởng

Thay `deleteSurroundingText()`, dùng `InputContext::forwardKey(Backspace)` đến
client hiện tại, rồi `commitString(new_suffix)`. Không phải uinput — forwardKey
là API Fcitx gửi key event thật đến client qua frontend.

### Nghiên cứu API (Fcitx 5.1.12)

`InputContext::forwardKey(const Key&, bool isRelease=false, int time=0)` tồn
tại, queue `ForwardKeyEvent` → `forwardKeyImpl` per-frontend → gửi thật đến
client. **Không recursion**: GTK client (fcitx5-gtk) tag synthesized event với
`IgnoredMask`, short-circuit khi re-enter `fcitx_im_context_filter_keypress`
(gọi slave context, không gọi `process_key`). DBus frontend `forwardKeyImpl`
gửi D-Bus signal thật, `bus()->flush()`.

### 20 câu hỏi bắt buộc (§18) — trả lời

1. forwardKey gửi đúng client InputContext hiện tại? **YES** — `forwardKeyImpl`
   dispatch trên `this` (IC đang xử lý).
2. Biết dispatch đầy đủ? **NO** — Fcitx queue event, không ACK app.
3. ACK từ app? **NO** — `commitString`/`forwardKey` không trả kết quả app.
4. Backspace #1 OK, #2 gửi, process chết trước #3? **Containment KHÔNG có** —
   runtime chỉ biết `KetQuaHost` per-call, không transaction.
5. Delete bằng key rồi commitString fail? Runtime `KhongChac` → MatDongBo.
6. Mouse click chuyển cursor: CanType nhận event/reset/surrounding update?
   **Tùy toolkit** (xem §10).
7. Selection bằng chuột: CanType biết khi surrounding unavailable? **NO** nếu
   surrounding None/invalid.
8. Undo/redo: frontend reset input context? **NO** — GTK/Qt IM module không
   có undo/redo hook, chỉ catch qua surrounding mismatch phím kế.
9. App autocomplete tự thay text: CanType có callback? **NO** — chỉ phát hiện
   qua surrounding mismatch (nếu surrounding có).
10. IM switch giữa composition: state reset chắc chắn? **YES** (Phase 2).
11. focus out/in: invalidation chắc? **YES** (Phase 2 FocusOut watcher).
12. same InputContext reuse cho vị trí cursor mới? **YES** — runtime không
    phân biệt, chỉ biết qua surrounding.
13. forwardKey Backspace recursion? **NO** (IgnoredMask GTK, xem trên).
14. Key release forwarded Backspace cần gửi? dbus gửi `isRelease` flag;
    wayland_v2 auto-inject release. Wayland v1 không auto-release.
15. Modifiers: forwardKey truyền `Key(sym, states, code)` — states full.
16. Timestamp: `forwardKey(key, isRelease, time)` — dùng `time` param (0 OK).
17. Frontend nào implement forwardKey thật? **Tất cả** (dbus, wayland v1/v2,
    xim, ibus, fcitx4) — `forwardKeyImpl` per-frontend.
18. Wayland semantics khác X11? **YES** — wayland_v2 auto-inject release,
    deleteSurroundingText reject `offset > 0`.
19. GTK/Qt/LibreOffice xử lý forwarded key khác nhau? GTK: synthesized GdkEvent
    (IgnoredMask). Qt: forwardKey D-Bus → Qt platform input context. LibreOffice:
    qua GTK3 IM module (như GTK). Hành vi client-side khác.
20. API mạnh hơn forwardKey mà không cần surrounding? **NO** —
    `commitStringWithCursor` cần capability `CommitStringWithCursor` và vẫn
    không verify cursor. Không có API "delete tại vị trí cụ thể" mà không cần
    surrounding.

### Quyết định: REJECTED

**Route B KHÔNG được implement production.** Lý do cốt lõi — cursor
invalidation contract yếu, không đủ an toàn cho zero-preedit:

- **GTK3**: click trong widget → `button-press-event` gửi Reset unconditional
  (an toàn). NHƯNG undo/redo/autocomplete không reset, chỉ catch qua surrounding
  mismatch phím kế. LibreOffice GTK3: `sur_valid=0` mọi phím → KHÔNG catch
  được mismatch → Route B mù cursor.
- **GTK4**: KHÔNG có click handler → click move cursor KHÔNG gửi reset/
  surrounding update. Runtime giữ ownership suffix cũ (stale) cho đến khi app
  gửi `set_surrounding` kế (nếu có). Nếu surrounding absent → Route B xóa sai vị trí.
- **Qt5/Qt6 generic widget**: KHÔNG reset trên click (chỉ Kate/LibreOffice/
  Konsole + chỉ khi có preedit — zero-preedit IME → không bao giờ trigger).
  Surrounding chỉ tới khi widget tự gọi `QInputMethod::update(ImSurroundingText)`.
- **XIM**: no surrounding + `deleteSurroundingText` no-op → Route B = forwardKey
  Backspace mù (KHÔNG verify được vị trí) → unsafe.
- **LibreOffice** (priority cao): `sur_valid=0` mọi phím → Route B dựa shadow
  suffix không có contract invalidate → corruption risk khi click/undo/autocomplete.

Route B không giải quyết LibreOffice (lý do chính Phase 3) và không thêm gì cho
frontend đã có Route A (Kate — Route A atomic hơn, có surrounding verify).
Tiêu chí §19: "Route B chỉ được phép nếu frontend có contract đủ mạnh để
invalidate composition khi cursor/selection/content thay đổi ngoài CanType" —
contract đó KHÔNG tồn tại cho GTK4/Qt generic/XIM/LibreOffice.

### Containment nếu route tương lai được xét lại

Nếu frontend sau này có contract click→reset (GTK3 đã có), Route B có thể được
ACCEPT cho frontend class đó (không global). Test cần (§29): happy path, fail
trước dispatch (KhongPhat), fail sau Backspace đầu (KhongChac), fail commit
(KhongChac), focus change, reset, mouse cursor invalidation, selection,
shortcut, Delete, arrow, Escape, IM switch, context destroy, two contexts,
fast typing, key repeat, Unicode. `tests/compatibility.rs` đã pin containment
cho partial dispatch (KhongChac không rollback, không replay mù).

## 8. Safety invariants (giữ nguyên Phase 1/2)

- NativeReplace invariant KHÔNG nới (§16): `sur_valid=0`/`sur_cap=0` →
  passthrough, không ThayThe mù.
- surrounding None không destructive NativeReplace (`khop_surrounding`:
  `None => false`).
- focus mismatch không mutation (relinquish + ChuyenTiep).
- suffix mismatch không mutation (relinquish + ChuyenTiep).
- Delete semantics đúng (DatLai + passthrough, không Backspace, không U+007F).
- shortcut passthrough (adapter lọc Ctrl/Alt/Super/Hyper/Meta → BoQua).
- physical key không duplicate (một sự kiện → tối đa một `thuc_thi`).
- KhongChac containment: MatDongBo, không delete dựa state cũ, không replay.
- two contexts độc lập (per-context PhienNhap, không state global).
- no preedit (HanhDong chỉ Chen/ThayThe/ChuyenTiep — enum không có nhánh khác).
- no uinput, no global hook, no clipboard rewrite.

## 9. Partial dispatch model

Nếu Route B được implement tương lai (N Backspace + commitString), đây là nhiều
platform call. Mapping `KetQuaHost`:

- Fail trước call đầu → `KhongPhat` → rollback Cadence, ChuyenTiep.
- Đã gửi ≥1 Backspace, chưa hoàn tất → `KhongChac` → MatDongBo, không rollback
  text, không replay.
- Tất cả calls đã dispatch → `DaPhat` → state tiến, verify phím kế.

KHÔNG gọi atomic, KHÔNG có ACK app. Runtime không rollback text sau partial
dispatch (không commit text cũ, không forward raw keys, không reconstruct
document) — reset ownership, MatDongBo, consume physical event. Test pin:
`tests/compatibility.rs::partial_dispatch_*`.

## 10. Focus / cursor behavior

- **focus out/in**: Phase 2 FocusOut watcher relinquish IC có Rust session.
  Invariant: ownership cleared, phím đầu sau focus-in compose tươi.
- **mouse click trong widget**: GTK3 gửi Reset unconditional (an toàn). GTK4
  KHÔNG gửi gì. Qt5/6 generic KHÔNG reset (chỉ Kate/LibreOffice/Konsole khi có
  preedit — zero-preedit → không trigger). XIM không có click handler.
- **selection**: GTK3/4 qua `set_surrounding` (anchor); nếu surrounding
  invalid → KHÔNG biết. Qt qua `QInputMethod::update`.
- **undo/redo/autocomplete**: KHÔNG reset, KHÔNG guaranteed surrounding update
  từ IM module. Chỉ catch qua surrounding mismatch phím kế (nếu surrounding có).
- **cursor move (arrow)**: `SuKienNhap::DiChuyenConTro` → relinquish + passthrough.

Trace test (Kate, `sur_valid=1`): Route A verify suffix mỗi phím → click/undo
phát hiện qua mismatch. Trace (LibreOffice/Chrome/VS Code/Konsole,
`sur_valid=0`): runtime relinquish khi `da_hien_thi` không rỗng + surrounding
invalid → passthrough an toàn (không xóa mù).

## 11. App matrix

Xem [`docs/COMPATIBILITY.md`](./COMPATIBILITY.md) cho bảng đầy đủ. Tóm tắt:

| App | Frontend | Session | Route | Vietnamese | Safety |
|-----|----------|---------|-------|------------|--------|
| Kate | Qt5/dbus | X11 native | Native | SUPPORTED | PASS |
| LibreOffice Writer | GTK3/dbus | X11 native | Passthrough | UNSUPPORTED | PASS |
| Google Chrome (URL bar) | Chromium/dbus | X11 native | Passthrough | UNSUPPORTED | PASS |
| VS Code | Electron/dbus | X11 native | Passthrough | UNSUPPORTED | PASS |
| Konsole | Qt5/dbus | X11 native | Passthrough | UNSUPPORTED | PASS |

## 12. X11 / XWayland / Wayland matrix

| Session | Đã test? | Frontend thấy | Ghi chú |
|---------|----------|---------------|---------|
| X11 native (KDE) | YES (ma trận trên) | dbus (mọi app) | GTK_IM_MODULE=fcitx, QT_IM_MODULE=fcitx |
| XWayland | NO | dbus + xim (dự kiến) | Cần session Wayland + app XWayland |
| Wayland native | NO | wayland / wayland_v2 (dự kiến) | Cần session/VM Wayland |

Máy test chỉ có X11 native (KDE). Wayland chưa runtime-verify — ghi rõ, không
giả. Chromium/Electron ozone platform chưa phân biệt (máy X11 native, không
Wayland session).

## 13. Limitations (trung thực)

- **Chỉ 1 frontend family runtime-verify** (`dbus`, KDE X11). `wayland`/
  `wayland_v2`/`xim`/`ibus`/`fcitx4` chưa test trên máy này.
- **Wayland chưa runtime validation** — cần session/VM riêng, không logout phá
  môi trường user.
- **Chỉ Qt text widget thật (Kate/KWrite/plasmashell) SUPPORTED**. LibreOffice,
  Chrome, VS Code, Konsole: Vietnamese UNSUPPORTED (passthrough an toàn).
- **Route B REJECTED** — không tìm được route zero-preedit thứ hai có cursor
  invalidation contract đủ mạnh. Phase 3 kết luận "secondary route unsafe for
  current frontend contracts" (§68).
- **GUI automation hạn chế**: mouse cursor torture test trên app thật khó tự
  động hóa đáng tin cậy (KWin focus stealing prevention, xdotool `type` không
  activate). Đã test `tests/compatibility.rs` (harness) + trace Kate Route A
  (sur_valid=1 verify mỗi phím).
- **Chrome textarea / contenteditable chưa test** riêng (chỉ URL bar, `sur_cap=0`).
  Dự kiến textarea có thể `sur_cap=1` — chưa verify.

## 14. Rejected approaches

- **Preedit fallback** (§4): TUYỆT ĐỐI KHÔNG. Không thêm `updatePreedit`/
  `clientPreedit`/`setPreedit`/`commitPreedit` hay setting "dùng preedit"/"legacy
  mode". CanType giữ zero-preedit.
- **uinput/evdev/libinput/XTest/xdotool runtime** (§5): TUYỆT ĐỐI KHÔNG. xdotool
  chỉ dùng cho TEST.
- **Optimistic surrounding** (§6): TUYỆT ĐỐI KHÔNG. `None => false` trong verify.
- **App-specific hack** (`if program.contains("libreoffice")`) (§21): tránh.
  Phase 3 phân loại theo frontend/capability, không app name. Không quirk rải rác.
- **Route B (ContextKeyReplace)** (§7): REJECTED — cursor invalidation contract
  yếu (GTK4/Qt/XIM/LibreOffice). Không ship demo "gõ được" khi unsafe.
- **Adaptive/smart routing/AI/confidence** (§48): KHÔNG. Routing deterministic
  theo capability.
- **User-facing route setting** (§22): KHÔNG. GUI không có dropdown
  Native/Compatible/Legacy/Safe. Routing tự động.

## 15. Phase 3 exit criteria

### Safety (§65) — tất cả pass

- [x] NativeReplace invariant không bị nới.
- [x] surrounding None không destructive NativeReplace.
- [x] focus mismatch không mutation.
- [x] suffix mismatch không mutation.
- [x] Delete semantics đúng (DatLai + passthrough).
- [x] shortcut passthrough đúng (adapter lọc modifier).
- [x] physical key không duplicate (một sự kiện → tối đa một thuc_thi).
- [x] KhongChac containment đúng (MatDongBo, không replay).
- [x] two contexts độc lập.
- [x] no preedit (HanhDong enum không có nhánh preedit).
- [x] no uinput, no global hook.
- [x] no app-specific hack rải rác.

### Compatibility (§66) — partial

- [x] Qt: Vietnamese SUPPORTED (Kate/KWrite/plasmashell, Route A).
- [ ] Traditional GTK: chưa test GTK3/GTK4 text field riêng (LibreOffice là GTK3
  frontend nhưng custom, không đại diện GTK thông thường). Cần GTK3/4 harness.
- [x] LibreOffice: BLOCKED BY FRONTEND (sur_valid=0) + SAFE PASSTHROUGH, bằng
  chứng rõ (trace `sur_cap=1 sur_valid=0` mọi phím). Route B không giải quyết được
  (cursor invalidation yếu).
- [x] Browser: Chrome (URL bar) runtime test — Vietnamese UNSUPPORTED, Safety
  PASS. (Firefox không có trên máy — N/A.)
- [x] Electron: VS Code runtime test — Vietnamese UNSUPPORTED, Safety PASS.
- [x] Terminal: Konsole SAFE PASSTHROUGH — Vietnamese UNSUPPORTED (route thật sự
  an toàn chưa có, passthrough acceptable theo §36).

### Minimum product bar (§68)

- [x] Ít nhất hai frontend family khác nhau thực sự transform được: chỉ Qt
  (Kate/plasmashell) — **không đạt "hai frontend family"** (dbus là một frontend,
  Qt/GTK cùng dbus; cần Wayland hoặc XIM test riêng). **Hạn chế rõ.**
- [x] Browser path đã test (Chrome URL bar — UNSUPPORTED, ghi rõ).
- [x] Office path đã phân loại (LibreOffice — BLOCKED BY FRONTEND).
- [x] Terminal không bị phá (Konsole SAFE PASSTHROUGH).
- [ ] Mouse/cursor torture test pass trên route ship: Kate Route A verify suffix
  mỗi phím (sur_valid=1) — test trace; torture test GUI automation chưa tự động
  hóa đáng tin cậy, harness `tests/compatibility.rs` pin invariant.
- [x] Không regress Kate (trace xác nhận `as→á`, `vowsi→với`, Route A).
- [x] zero-preedit giữ nguyên.

**Kết luận Phase 3**: compatibility research concluded secondary route
(ContextKeyReplace) unsafe for current frontend contracts. Coverage còn hạn chế
(một frontend family dbus, Wayland/XIM chưa verify). Phase 4 packaging chưa
nên quảng bá rộng — cần runtime-verify Wayland + GTK3/4 text field riêng trước.
