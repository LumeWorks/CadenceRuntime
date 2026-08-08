# CanType Phase 3C — PlainComposition (zero visible decoration)

Tài liệu Phase 3C, phản ánh code thật. Phase 3C thay thế invariant "ZERO
PREEDIT" bằng "ZERO VISIBLE COMPOSITION DECORATION": client preedit được
dùng làm primitive kỹ thuật, nhưng phải không hiển thị decoration (underline,
highlight, candidate popup, ghost text). Text chỉ commit vào document tại
boundary.

## 1. Mục tiêu

Mở rộng khả năng gõ tiếng Việt của CanType trên các ứng dụng KHÔNG cung cấp
surrounding text hợp lệ (LibreOffice Writer, Google Chrome, VS Code, Konsole)
mà KHÔNG phá invariant an toàn Phase 1/2.

Phase 3 cũ: những app này passthrough toàn bộ → Vietnamese UNSUPPORTED.
Phase 3C: dùng PlainComposition (client preedit, no decoration) → Vietnamese
SUPPORTED trên app có preedit capability.

## 2. Ba đường output

| Route | HanhDong | Khi nào | Verify |
|-------|----------|---------|--------|
| **VerifiedReplace** | Chen/ThayThe | surrounding hợp lệ (`van_ban_truoc_con_tro.is_some()`) | verify suffix mỗi phím |
| **PlainComposition** | CapNhatSoanThao/KetThucSoanThao/XoaSoanThao | surrounding không hợp lệ nhưng client hỗ trợ preedit (`co_preedit=true`) | không cần — text trong preedit, không document |
| **Passthrough** | ChuyenTiep | sensitive context, hoặc không route khả dụng | không áp dụng |

### Route selection (`chon_duong`)

Thứ tự ưu tiên (composition mới, `duong == None`):

1. `co_sensitive` → Passthrough (password, sensitive — không preedit)
2. `van_ban_truoc_con_tro.is_some()` → VerifiedReplace (surrounding hợp lệ)
3. `co_preedit` → PlainComposition (client hỗ trợ preedit)
4. else → Passthrough

Route selection dùng **trạng thái hiện tại** (`van_ban_truoc_con_tro`),
không phải capability (`co_surrounding`): LibreOffice/Konsole có `sur_cap=1`
nhưng `sur_valid=0` mọi phím — nếu chọn VerifiedReplace dựa capability, phím 2
bị chặn (verify fail) và app không gõ được. Dựa trạng thái: sur_valid=0 →
PlainComposition → app gõ được.

### Route lock

Một composition giữ route từ khi bắt đầu đến boundary/reset. Route chỉ chọn
khi composition mới (`duong == None`). Sau khi chọn, route bị khóa — phím kế
tiếp dùng cùng route bất kể surrounding/preedit thay đổi.

Lý do: tránh route flapping giữa VerifiedReplace và PlainComposition khi
surrounding báo hợp lệ ở phím 2 nhưng không ở phím 1 (Kate/Qt pattern).

## 3. PlainComposition flow

### Xây composition (KyTu/XoaLui)

```
phím KyTu/XoaLui → Cadence them_ky_tu/xoa_lui → noi_dung_moi
→ CapNhatSoanThao(noi_dung_moi)  // cập nhật client preedit, NO decoration
→ host.thuc_thi → DaPhat → da_hien_thi = noi_dung_moi
```

Mỗi phím cập nhật client preedit với `TextFormatFlag::NoFlag` — zero visible
decoration. Text chưa vào document.

Khi Cadence rỗng (backspace đến empty):
```
→ XoaSoanThao  // clear client preedit
```

### Boundary (RanhGioiTu)

```
RanhGioiTu(space) → nếu có composition:
  KetThucSoanThao(da_hien_thi + space)  // commit text + boundary char, clear preedit
→ host.thuc_thi → DaPhat → relinquish (route cleared)
```

Nếu composition rỗng:
```
  Chen(space)  // chỉ chèn boundary char
```

### Relinquish (DatLai/DiChuyenConTro/focus-out)

```
DatLai/arrow/focus-out → nếu PlainComposition + da_hien_thi không rỗng:
  XoaSoanThao  // clear client preedit qua host
→ relinquish (Cadence reset, da_hien_thi clear, route cleared)
```

## 4. Zero visible decoration

`CapNhatSoanThao` gửi text qua Fcitx `setClientPreedit` với
`TextFormatFlag::NoFlag = 0`:

```cpp
fcitx::Text preedit(text, fcitx::TextFormatFlag::NoFlag);
preedit.setCursor(text.size());  // cursor cuối composition
ic->inputPanel().setClientPreedit(preedit);
ic->updatePreedit();
```

`NoFlag` = không underline, không highlight, không bold, không italic, không
strike. Client hiển thị text composition mà không có decoration — user thấy
text "với" xuất hiện khi gõ, không có gạch chân hay popup.

## 5. Safety invariants (giữ nguyên Phase 1/2 + mới)

### Giữ nguyên từ Phase 1/2/3

- VerifiedReplace invariant không nới: `sur_valid=0` → không ThayThe mù.
- surrounding None không destructive NativeReplace.
- focus mismatch không mutation (relinquish + ChuyenTiep).
- suffix mismatch không mutation (relinquish + ChuyenTiep).
- Delete semantics đúng (DatLai + passthrough).
- shortcut passthrough (adapter lọc Ctrl/Alt/Super/Hyper/Meta).
- physical key không duplicate (một sự kiện → tối đa một `thuc_thi`).
- KhongChac containment: MatDongBo, không delete dựa state cũ, không replay.
- two contexts độc lập.
- no uinput, no global hook, no clipboard rewrite.

### Mới Phase 3C

- **Zero visible composition decoration**: `CapNhatSoanThao` chỉ gửi
  `TextFormatFlag::NoFlag`. KHÔNG underline, highlight, bold, italic, strike.
- **Không commit text trung gian vào document**: PlainComposition chỉ commit
  tại boundary (space, punctuation) qua `KetThucSoanThao`. Text trung gian nằm
  trong client preedit.
- **Sensitive/password → Passthrough**: `co_sensitive=true` → không preedit,
  không composition. Route selection ưu tiên cao nhất.
- **Route lock**: một composition giữ route đến boundary/reset. Không flapping.
- **Route selection dùng trạng thái, không capability**: `van_ban_truoc_con_tro.is_some()`,
  không `co_surrounding`.
- **Lifecycle clear preedit**: activate/deactivate/reset/focus-out gọi
  `clearPreedit()` (C++ `inputPanel().reset() + updatePreedit()`).

## 6. Diagnostic

`src/tuong_thich.rs` (feature `diag` + env `CANTYPE_DEBUG`) log metadata
KHÔNG chứa text user. Trace Phase 3C thêm:

- `route`: `native` (VerifiedReplace), `plain_composition`, `passthrough`
- `action`: `da_xu_ly`, `passthrough`, `reset`
- `preedit`: capability flag (trace only)

Production addon: NO `diag` feature → zero cost trên hot path.

## 7. Test coverage

### Unit tests (tests/plain_composition.rs — 27 tests)

- Route selection: sensitive → Passthrough; surrounding → VerifiedReplace;
  preedit → PlainComposition; none → Passthrough
- Route lock: PlainComposition giữ đến boundary
- Telex state: "as" → "á", "tieengs" → "tiếng", "af" → "à", "vowsi" → "với"
- Backspace: hoàn tác, đến empty → XoaSoanThao
- Boundary: KetThucSoanThao (commit + clear), Chen khi rỗng
- Escape/arrow/focus change: relinquish + clear preedit
- Fault injection: KhongPhat (rollback), KhongChac (MatDongBo)
- Two contexts: A PlainComposition, B VerifiedReplace, độc lập
- Zero visible decoration: không Chen/ThayThe trung gian

### Acceptance test (app thật)

| App | Frontend | sur_cap | sur_valid | preedit | Route | Vietnamese | Verified |
|-----|----------|---------|-----------|---------|-------|------------|----------|
| Kate | dbus/Qt5 | 1 | 0→1 | 1 | plain_composition | SUPPORTED | trace `route=plain_composition`, file "avới" |
| LibreOffice Writer | dbus/GTK3 | 1 | 0 | 1 | plain_composition (dự kiến) | SUPPORTED (dự kiến) | chưa verify (GUI automation hạn chế) |
| Google Chrome | dbus/Chromium | 0 | 0 | 1 | plain_composition (dự kiến) | SUPPORTED (dự kiến) | chưa verify (KWin focus stealing) |
| VS Code | dbus/Electron | 0 | 0 | 1 | plain_composition (dự kiến) | SUPPORTED (dự kiến) | chưa verify |
| Konsole | dbus/Qt5 | 1 | 0 | 1 | plain_composition (dự kiến) | SUPPORTED (dự kiến) | chưa verify |

Kate runtime-verified: `vowsi → với` qua PlainComposition, trace xác nhận
`route=plain_composition action=da_xu_ly outcome=da_ap_dung`. File Kate chứa
`avới` sau khi gõ `avowsi ` (a + vowsi + space).

Các app khác: route selection logic giống Kate (sur_valid=0 →
van_ban_truoc_con_tro=None → co_preedit=true → PlainComposition). Unit test
pin hành vi này (`khong_surrounding_co_preedit_chon_plain_composition`).
Runtime-verify bị hạn chế bởi GUI automation (KWin focus stealing prevention).

## 8. Non-goals

Phase 3C **không** triển khai:
- Preedit fallback decoration (underline, highlight, popup)
- uinput/evdev/XTest/xdotool runtime
- App-specific routing (`if program.contains("libreoffice")`)
- User-facing route setting (GUI dropdown)
- Adaptive/smart routing/AI/confidence
- Packaging system-wide

## 9. Cadence

Phase 3C KHÔNG sửa Cadence. Cadence boundary giữ nguyên: `PhienCadence`
wrap `PhienGo`, `ban_chup().noi_dung` là rendered text. Route selection và
output method nằm trong CanType runtime, không trong Cadence.
