// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

/// C++ shim Fcitx5 mỏng cho CadenceRuntime.
///
/// Chỉ làm platform glue: định nghĩa `InputMethodEngineV2`, đăng ký addon
/// factory, đăng ký `InputContextProperty`, nhận lifecycle callback, đọc
/// `KeyEvent`/`InputContext`, chuyển dữ liệu qua C ABI (`fcitx5_ffi.h`), gọi
/// commit/delete khi Rust yêu cầu, accept hoặc pass-through event.
///
/// KHÔNG chứa: luật Telex, state Cadence, common-prefix diff, history replay,
/// heuristic tiếng Việt, app profile, adaptive route, fallback uinput. Toàn bộ
/// runtime logic nằm trong Rust (`src/fcitx5.rs`, `src/ffi.rs`).

#include "fcitx5_ffi.h"

#include <fcitx-utils/flags.h>
#include <fcitx-utils/i18n.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/standardpath.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addoninstance.h>
#include <fcitx/addonmanager.h>
#include <fcitx/event.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>
#include <fcitx/surroundingtext.h>

#include <cstdint>
#include <cstring>
#include <string>

namespace {

/// Tên property đăng ký với `InputContextManager`.
constexpr const char *kTenProperty = "cadence-runtime-context";

/// Property per `InputContext`: sở hữu opaque Rust `PhienNhap` handle, context
/// id riêng, và focus generation. Hai context không chia sẻ `PhienNhap`.
///
/// Lifecycle: tạo khi context cần state (lazy qua factory), destructor giải
/// phóng Rust handle đúng một lần. `needCopy()` trả false (không share state
/// giữa context).
class CadenceProperty : public fcitx::InputContextProperty {
public:
    explicit CadenceProperty(uint64_t context_id) : context_id_(context_id) {
        // Tạo PhienNhap Rust ngay khi property sinh. Handle NULL nếu lỗi.
        phien = cadence_phien_tao(context_id);
    }

    ~CadenceProperty() override {
        if (phien != nullptr) {
            cadence_phien_giai_phong(phien);
            phien = nullptr;
        }
    }

    CadenceProperty(const CadenceProperty &) = delete;
    CadenceProperty &operator=(const CadenceProperty &) = delete;
    CadenceProperty(CadenceProperty &&) = delete;
    CadenceProperty &operator=(CadenceProperty &&) = delete;

    bool needCopy() const override { return false; }

    uint64_t contextId() const { return context_id_; }

    /// Cập nhật focus generation khi một focus cycle mới bắt đầu (false→true).
    uint64_t capNhatFocus(bool has_focus) {
        if (!last_has_focus_ && has_focus) {
            // Saturating add: overflow u64 thực tế không đạt, saturate thay vì
            // wrap để tránh trùng context/focus generation.
            focus_generation_ =
                (focus_generation_ == UINT64_MAX) ? UINT64_MAX
                                                  : focus_generation_ + 1;
        }
        last_has_focus_ = has_focus;
        return focus_generation_;
    }

    uint64_t focusGeneration() const { return focus_generation_; }
    bool lastHasFocus() const { return last_has_focus_; }

    /// Opaque Rust PhienNhap handle. NULL nếu `cadence_phien_tao` thất bại.
    void *phien = nullptr;

private:
    uint64_t context_id_;
    uint64_t focus_generation_ = 0;
    bool last_has_focus_ = false;
};

// ---------------------------------------------------------------------------
// Callback C++ cho Rust `Host` qua `CadenceHostBang`.
// ---------------------------------------------------------------------------

/// Điền `CadenceContextSnapshot` từ `InputContext`. Trả 0=ok, nonzero=lỗi.
extern "C" int lay_boi_canh_cb(void *ic_ptr,
                                CadenceContextSnapshot *out) {
    auto *ic = static_cast<fcitx::InputContext *>(ic_ptr);
    if (ic == nullptr || out == nullptr) {
        return 1;
    }
    auto *prop = static_cast<CadenceProperty *>(ic->property(kTenProperty));
    if (prop == nullptr) {
        return 1;
    }
    out->context_id = prop->contextId();
    out->focus_generation = prop->focusGeneration();
    out->has_focus = ic->hasFocus() ? 1 : 0;

    const auto &st = ic->surroundingText();
    if (st.isValid()) {
        out->surrounding_valid = 1;
        out->cursor = st.cursor();
        out->anchor = st.anchor();
        // `st.text()` là `const std::string&` sở hữu bởi SurroundingText
        // (sống cùng IC). Rust copy ngay sau callback → ptr hợp lệ.
        out->text.ptr =
            reinterpret_cast<const uint8_t *>(st.text().data());
        out->text.len = st.text().size();
    } else {
        out->surrounding_valid = 0;
        out->cursor = 0;
        out->anchor = 0;
        out->text.ptr = nullptr;
        out->text.len = 0;
    }
    return 0;
}

/// Commit string UTF-8. Trả 0=DaPhat, 1=KhongPhat, 2=KhongChac.
extern "C" int chen_cb(void *ic_ptr, const uint8_t *ptr, size_t len) {
    auto *ic = static_cast<fcitx::InputContext *>(ic_ptr);
    if (ic == nullptr || (ptr == nullptr && len > 0)) {
        return 1;
    }
    std::string text(reinterpret_cast<const char *>(ptr), len);
    ic->commitString(text);
    // Fcitx không trả kết quả từ app → giả định DaPhat. Verify ở phím kế.
    return 0;
}

/// Xóa `xoa_ky_tu` ký tự trước con trỏ rồi commit string. Trả 0/1/2.
extern "C" int thay_the_cb(void *ic_ptr, uint32_t xoa_ky_tu,
                           const uint8_t *ptr, size_t len) {
    auto *ic = static_cast<fcitx::InputContext *>(ic_ptr);
    if (ic == nullptr || (ptr == nullptr && len > 0)) {
        return 1;
    }
    if (xoa_ky_tu > 0) {
        // deleteSurroundingText(offset, size): offset âm = trước con trỏ.
        // xoa_ky_tu là số code point (không phải byte), khớp cursor offset.
        ic->deleteSurroundingText(-static_cast<int>(xoa_ky_tu), xoa_ky_tu);
    }
    std::string text(reinterpret_cast<const char *>(ptr), len);
    ic->commitString(text);
    return 0;
}

// ---------------------------------------------------------------------------
// Engine.
// ---------------------------------------------------------------------------

/// Engine Fcitx5: triển khai `InputMethodEngineV2`.
class CadenceEngine : public fcitx::InputMethodEngineV2 {
public:
    CadenceEngine(fcitx::Instance &instance)
        : instance_(instance),
          factory_([this](fcitx::InputContext & /*ic*/) {
              return new CadenceProperty(nextContextId());
          }) {
        instance_.inputContextManager().registerProperty(kTenProperty,
                                                          &factory_);
    }

    void keyEvent(const fcitx::InputMethodEntry & /*entry*/,
                  fcitx::KeyEvent &event) override {
        auto *ic = event.inputContext();
        if (ic == nullptr) {
            return;
        }
        auto *prop =
            static_cast<CadenceProperty *>(ic->property(kTenProperty));
        if (prop == nullptr || prop->phien == nullptr) {
            return;
        }
        // Cập nhật focus generation trước khi gọi Rust (Rust kiểm tra ở đầu
        // xu_ly).
        prop->capNhatFocus(ic->hasFocus());

        const auto &key = event.key();

        // Key snapshot: điền từ KeyEvent/Key.
        CadenceKeySnapshot snapshot;
        snapshot.is_release = event.isRelease() ? 1 : 0;
        snapshot.is_cursor_move = key.isCursorMove() ? 1 : 0;
        snapshot.is_modifier = key.isModifier() ? 1 : 0;
        // has_modifier: Ctrl/Alt/Super/Hyper/Meta (KHÔNG tính Shift).
        const auto states = key.states();
        const auto mod_mask =
            fcitx::KeyStates(fcitx::KeyState::Ctrl) | fcitx::KeyState::Alt |
            fcitx::KeyState::Super | fcitx::KeyState::Hyper |
            fcitx::KeyState::Meta;
        snapshot.has_modifier = states.testAny(mod_mask) ? 1 : 0;
        snapshot.dac_biet = dacBietTuKeysym(key.normalize().sym());

        // UTF-8: keySymToUTF8 trả std::string; giữ sống trong scope này.
        std::string utf8 = fcitx::Key::keySymToUTF8(key.normalize().sym());
        snapshot.utf8.ptr =
            reinterpret_cast<const uint8_t *>(utf8.data());
        snapshot.utf8.len = utf8.size();

        // Host bang: callback table + ic.
        CadenceHostBang bang;
        bang.ic = ic;
        bang.lay_boi_canh = lay_boi_canh_cb;
        bang.chen = chen_cb;
        bang.thay_the = thay_the_cb;

        auto ket_qua = cadence_xu_ly_phim(prop->phien, &snapshot, &bang);

        switch (ket_qua) {
        case CadenceXuLy_DaApDung:
            // Runtime đã phát text. Không forward phím gốc.
            event.filterAndAccept();
            break;
        case CadenceXuLy_MatDongBo:
            // Host có thể đã phát một phần. Không forward (tránh bản sao).
            event.filterAndAccept();
            break;
        case CadenceXuLy_ChuyenTiep:
            // Passthrough: forward phím gốc (không filter, không accept).
            break;
        }
    }

    void reset(const fcitx::InputMethodEntry & /*entry*/,
               fcitx::InputContextEvent &event) override {
        auto *ic = event.inputContext();
        if (ic == nullptr) {
            return;
        }
        auto *prop =
            static_cast<CadenceProperty *>(ic->property(kTenProperty));
        if (prop == nullptr || prop->phien == nullptr) {
            return;
        }
        cadence_phien_dat_lai(prop->phien);
    }

private:
    /// Ánh xạ keysym sang `CadenceKeyDacBiet`.
    static CadenceKeyDacBiet dacBietTuKeysym(fcitx::KeySym sym) {
        switch (sym) {
        case FcitxKey_BackSpace:
            return CadenceKeyBackspace;
        case FcitxKey_Escape:
            return CadenceKeyEscape;
        case FcitxKey_Return:
            return CadenceKeyEnter;
        case FcitxKey_Tab:
            return CadenceKeyTab;
        default:
            return CadenceKeyKhac;
        }
    }

    /// Cấp context id tăng đơn diệu (saturating, không wrap).
    uint64_t nextContextId() {
        if (next_context_id_ == UINT64_MAX) {
            return UINT64_MAX;
        }
        return next_context_id_++;
    }

    fcitx::Instance &instance_;
    fcitx::FactoryFor<CadenceProperty> factory_;
    uint64_t next_context_id_ = 1;
};

/// Addon factory: tạo engine khi addon được nạp.
class CadenceFactory : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        return new CadenceEngine(*manager->instance());
    }
};

} // namespace

/// Trả static `CadenceFactory`. Rust `fcitx_addon_factory_instance` (xuất bằng
/// `#[no_mangle]` trong `src/ffi.rs`) gọi hàm này. Function-local static,
/// thread-safe init (C++11). Không dùng `FCITX_ADDON_FACTORY` vì Rust cdylib
/// chỉ export symbol `#[no_mangle]` Rust (version script).
extern "C" void *cadence_native_factory(void) {
    static CadenceFactory factory;
    return &factory;
}
