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
///
/// Commit này chỉ dựng skeleton: engine + property + factory compile và link,
/// keyEvent pass-through. Wire FFI (gọi `cadence_*`) ở commit FFI boundary.

#include "fcitx5_ffi.h"

#include <fcitx-utils/i18n.h>
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

#include <cstdint>

namespace {

/// Property per `InputContext`: sở hữu opaque Rust `PhienNhap` handle, context
/// id riêng, và focus generation. Hai context không chia sẻ `PhienNhap`.
///
/// Lifecycle: tạo khi context cần state (lazy qua factory), destructor giải
/// phóng Rust handle đúng một lần. `needCopy()` trả false (không share state
/// giữa context).
class CadenceProperty : public fcitx::InputContextProperty {
public:
    CadenceProperty(uint64_t context_id) : context_id_(context_id) {}

    ~CadenceProperty() override;

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

    /// Opaque Rust PhienNhap handle. NULL cho đến khi FFI được wire (commit 4).
    void *phien = nullptr;

private:
    uint64_t context_id_;
    uint64_t focus_generation_ = 0;
    bool last_has_focus_ = false;
};

CadenceProperty::~CadenceProperty() {
    // Commit 3: phien luôn NULL. Giải phóng Rust handle wire ở commit 4.
}

/// Engine Fcitx5: triển khai `InputMethodEngineV2`.
class CadenceEngine : public fcitx::InputMethodEngineV2 {
public:
    CadenceEngine(fcitx::Instance &instance)
        : instance_(instance),
          factory_([this](fcitx::InputContext & /*ic*/) {
              return new CadenceProperty(nextContextId());
          }) {
        // Đăng ký property per-context. Factory lambda nhận context, cấp
        // context_id tăng đơn diệu từ engine counter.
        instance_.inputContextManager().registerProperty(
            "cadence-runtime-context", &factory_);
    }

    void keyEvent(const fcitx::InputMethodEntry & /*entry*/,
                  fcitx::KeyEvent & /*event*/) override {
        // Commit 3: pass-through. Logic key mapping + Rust wire ở commit 4.
    }

    void reset(const fcitx::InputMethodEntry & /*entry*/,
               fcitx::InputContextEvent & /*event*/) override {
        // Commit 3: no-op. DatLai phien wire ở commit 4.
    }

private:
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
