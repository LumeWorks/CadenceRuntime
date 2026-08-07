# CanType — Ma trận tương thích frontend

Bảng kết quả khảo sát Phase 3, runtime-test trên máy thật (KDE, X11 native,
Fcitx 5.1.12, CanType addon diagnostic `--features "fcitx5 diag"`, env
`CANTYPE_DEBUG=1`). Trace thu được qua `cantype diag` (KHÔNG chứa text user —
chỉ surrounding length, suffix match boolean, capability flags, cursor/anchor
offset).

Trạng thái dùng đúng terminology Phase 3 (§45): `SUPPORTED` (gõ tiếng Việt
được qua route an toàn), `SAFE PASSTHROUGH` (không gõ được nhưng không phá
nhập), `UNSUPPORTED` (không gõ được), `BLOCKED BY FRONTEND` (frontend không
cung cấp contract đủ). Tách rõ Vietnamese behavior và Safety behavior.

## Ma trận app (runtime-verified)

| App | Toolkit/Frontend | Session | Surrounding | Route | Vietnamese | Safety | Notes |
|-----|------------------|---------|--------------|-------|------------|--------|-------|
| Kate | Qt5 / dbus | X11 native | valid (từ phím 2) | Native (ThayThe) | SUPPORTED | PASS | `sur_cap=1 sur_valid=1` từ phím thứ 2, `route=native` |
| KWrite | Qt5 / dbus | X11 native | valid (giống Kate) | Native | SUPPORTED | PASS | Cùng Qt text widget — baseline Qt |
| plasmashell (KRunner/notifications) | Qt5 / dbus | X11 native | valid | Native | SUPPORTED | PASS | `sur_cap=1 sur_valid=0→1` |
| LibreOffice Writer | GTK3 / dbus | X11 native | invalid | Passthrough | UNSUPPORTED | PASS | `sur_cap=1 sur_valid=0` mọi phím — capability có nhưng GTK3 frontend không report surrounding thực |
| Google Chrome (URL bar) | Chromium / dbus | X11 native | absent | Passthrough | UNSUPPORTED | PASS | `sur_cap=0 sur_valid=0` — URL bar không advertise SurroundingText |
| VS Code (Electron editor) | Electron / dbus | X11 native | absent | Passthrough | UNSUPPORTED | PASS | `sur_cap=0 sur_valid=0` — Electron IM module không advertise surrounding |
| Konsole (terminal) | Qt5 / dbus | X11 native | invalid | Passthrough | UNSUPPORTED | PASS | `sur_cap=1 sur_valid=0`, `term=0` (Terminal flag không set qua dbus) — passthrough shell an toàn |

## Ma trận session (chưa runtime-verify)

| Session | Frontend Fcitx | Wayland native test | Ghi chú |
|---------|----------------|--------------------|---------|
| X11 native (KDE) | dbus (GTK/Qt IM module), xim (raw XIM) | N/A | Đã verify (ma trận trên) |
| XWayland | dbus + xim | Chưa test | Máy test chỉ có X11 native |
| Wayland native | wayland / wayland_v2 | Chưa test | Cần session/VM Wayland riêng |

## Frontend taxonomy (Fcitx 5.1.12, `ic->frontend()`)

| `frontend()` string | Frontend class | SurroundingText | deleteSurroundingText | forwardKey | Toolkit path |
|---------------------|----------------|-----------------|------------------------|------------|--------------|
| `dbus` | DBusInputContext1 (dbusfrontend) | client-driven (SetSurroundingText D-Bus) | real send | real send (IgnoredMask chặn recursion GTK) | GTK3/4, Qt5/6, Electron (qua fcitx5-gtk/fcitx5-qt) |
| `wayland` | WaylandIMInputContextV1 | compositor → text-input-v1 | real send (char→byte offset) | real send (sendKeyToVK) | Wayland text-input v1 |
| `wayland_v2` | WaylandIMInputContextV2 | compositor → text-input-v2 | real send (reject positive offset) | real send (press+auto-release) | Wayland text-input v2 |
| `xim` | XIMInputContext | KHÔNG hỗ trợ | no-op | real send (xcb_im_forward_event) | X11 raw (XMODIFIERS=@im=fcitx, không IM module) |
| `ibus` | IBusInputContext | client-driven (IBus capability) | real send | real send (ForwardKeyEvent, keycode-8) | IBus protocol (Chromium default Linux path) |
| `fcitx4` | Fcitx4InputContext | client-driven | real send | real send | Legacy fcitx4 D-Bus |

Máy test (KDE X11): mọi app dùng `dbus` (GTK_IM_MODULE=fcitx, QT_IM_MODULE=fcitx).
Không bắt gặp `wayland`/`wayland_v2`/`xim`/`ibus`/`fcitx4` — chưa runtime-verify.

## Capability matrix (trace `cantype diag`)

| App | `sur_cap` | `sur_valid` | `pw` | `term` | `sens` | `preedit` | `route` |
|-----|-----------|-------------|------|-------|--------|----------|---------|
| Kate (Qt) | 1 | 0→1 | 0 | 0 | 0 | 1 | native |
| LibreOffice (GTK3) | 1 | 0 | 0 | 0 | 0 | 1 | passthrough |
| Chrome (URL bar) | 0 | 0 | 0 | 0 | 0 | 1 | passthrough |
| VS Code (Electron) | 0 | 0 | 0 | 0 | 0 | 1 | passthrough |
| Konsole (terminal) | 1 | 0 | 0 | 0 | 0 | 1 | passthrough |

`pw`/`sens` = 0 cho mọi context test (không phải password). `term` = 0 cho
Konsole (CapabilityFlag::Terminal không set qua dbus Qt IM module). `preedit`
= 1 (capability có, CanType không dùng — zero-preedit invariant).
