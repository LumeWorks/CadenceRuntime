// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

/// C++ shim Fcitx5 mỏng cho CanType.
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

#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/flags.h>
#include <fcitx-utils/handlertable.h>
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
constexpr const char *kTenProperty = "cantype-context";

/// Property per `InputContext`: sở hữu opaque Rust `PhienNhap` handle, context
/// id riêng, focus generation và `active` state. Hai context không chia sẻ
/// `PhienNhap`.
///
/// Lifecycle đầy đủ (§4 Phase 2): `activate`/`focus-in` → active=true + bump
/// generation; `deactivate`/`focus-out` → relinquish + active=false + bump;
/// `reset` → relinquish + bump (active vẫn true, ic vẫn focus); context
/// destroy → free Rust handle đúng một lần. Mọi helper idempotent: gọi hai lần
/// liên tiếp (vd focus-out rồi deactivate) không double-free, không bump vô hạn
/// (chỉ bump khi `active_` thực sự chuyển, hoặc forced cho `reset`).
///
/// `phien` tạo lazy ở `damBaoPhien()` (lần đầu `keyEvent`) để IC không gõ tiếng
/// Việt không tạo Rust session; watcher FocusOut chỉ tạo shell nhẹ (không
/// phien) cho các IC khác — chúng bị bỏ qua qua `prop->phien != nullptr`.
class CanTypeProperty : public fcitx::InputContextProperty {
public:
    explicit CanTypeProperty(uint64_t context_id) : context_id_(context_id) {}

    ~CanTypeProperty() override {
        if (phien != nullptr) {
            cantype_phien_giai_phong(phien);
            phien = nullptr;
        }
    }

    CanTypeProperty(const CanTypeProperty &) = delete;
    CanTypeProperty &operator=(const CanTypeProperty &) = delete;
    CanTypeProperty(CanTypeProperty &&) = delete;
    CanTypeProperty &operator=(CanTypeProperty &&) = delete;

    bool needCopy() const override { return false; }

    uint64_t contextId() const { return context_id_; }
    uint64_t focusGeneration() const { return focus_generation_; }
    bool active() const { return active_; }
    bool lastHasFocus() const { return last_has_focus_; }

    /// Đảm bảo Rust `PhienNhap` tồn tại (tạo lazy lần đầu `keyEvent`). Trả
    /// `false` nếu tạo thất bại (handle NULL). Sau khi đã tạo, gọi lại là
    /// no-op.
    bool damBaoPhien() {
        if (phien == nullptr) {
            phien = cantype_phien_tao(context_id_);
        }
        return phien != nullptr;
    }

    /// Relinquish composition (đặt lại Rust session). Idempotent: gọi trên
    /// session rỗng là no-op (`cantype_phien_dat_lai` an toàn với NULL).
    void relinquish() {
        if (phien != nullptr) {
            cantype_phien_dat_lai(phien);
        }
    }

    /// Bump focus generation (saturating, không wrap để tránh trùng generation).
    void bumpGen() {
        focus_generation_ =
            (focus_generation_ == UINT64_MAX) ? UINT64_MAX : focus_generation_ + 1;
    }

    /// Đặt `active_`; bump generation CHỈ khi `active_` thực sự chuyển trạng
    /// thái. Tránh bump vô hạn khi callback trùng lặp (vd focus-out rồi
    /// deactivate cùng báo active=false).
    void datActive(bool co) {
        if (active_ != co) {
            active_ = co;
            bumpGen();
        }
    }

    /// Lifecycle `activate`/`focus-in`: relinquish + active=true. Bump chỉ khi
    /// false→true. Session sạch (relinquish idempotent trên session rỗng).
    void kichHoat() {
        relinquish();
        datActive(true);
        last_has_focus_ = true;
    }

    /// Lifecycle `deactivate`/`focus-out`: relinquish + active=false. Bump chỉ
    /// khi true→false. Idempotent: gọi lại khi đã false → chỉ relinquish no-op.
    void voHieuHoa() {
        relinquish();
        datActive(false);
        last_has_focus_ = false;
    }

    /// Lifecycle `reset` (ic vẫn focus): relinquish + forced bump, active vẫn
    /// true. Forced bump vì reset là ranh giới composition mới trong cùng focus
    /// cycle; Rust session cần thấy generation mới để treat phím kế tiếp là
    /// fresh (không tiếp tục composition cũ).
    void datLai() {
        relinquish();
        bumpGen();
        // active_ giữ nguyên (reset chỉ gọi khi ic focused).
    }

    /// Cập nhật focus từ `keyEvent` (lazy focus detection). Phát hiện focus-out
    /// ngay khi nhận phím trong lúc mất focus (hiếm): relinquish + active=false.
    /// Phát hiện focus-in (false→true): active=true (bump). Trả generation hiện
    /// tại để Rust đọc ở đầu `xu_ly`.
    uint64_t capNhatFocus(bool has_focus) {
        if (!has_focus && last_has_focus_) {
            // Đang nhận phím khi mất focus: relinquish ngay, không đợi phím kế.
            voHieuHoa();
        } else if (has_focus && !last_has_focus_) {
            // Focus mới: active=true (bump chỉ khi false→true).
            datActive(true);
        }
        last_has_focus_ = has_focus;
        return focus_generation_;
    }

    /// Opaque Rust PhienNhap handle. NULL cho đến khi `damBaoPhien()` tạo lần
    /// đầu (lazy). Destructor free đúng một lần.
    void *phien = nullptr;

private:
    uint64_t context_id_;
    uint64_t focus_generation_ = 0;
    bool active_ = false;
    bool last_has_focus_ = false;
};

// ---------------------------------------------------------------------------
// Callback C++ cho Rust `Host` qua `CanTypeHostBang`.
// ---------------------------------------------------------------------------

/// Điền `CanTypeContextSnapshot` từ `InputContext`. Trả 0=ok, nonzero=lỗi.
extern "C" int lay_boi_canh_cb(void *ic_ptr,
                                CanTypeContextSnapshot *out) {
    auto *ic = static_cast<fcitx::InputContext *>(ic_ptr);
    if (ic == nullptr || out == nullptr) {
        return 1;
    }
    auto *prop = static_cast<CanTypeProperty *>(ic->property(kTenProperty));
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

    // Phase 3 metadata: frontend/program/capability cho classification và
    // diagnostic. KHÔNG dùng cho verify-before-mutate (chỉ metadata). frontend()
    // trả const char* (NUL-terminated); program() trả const std::string&. Cả hai
    // sống cùng InputContext, Rust copy ngay sau callback.
    const char *frontend = ic->frontend();
    out->frontend.ptr = reinterpret_cast<const uint8_t *>(frontend);
    out->frontend.len = frontend ? std::strlen(frontend) : 0;

    const auto &program = ic->program();
    out->program.ptr =
        reinterpret_cast<const uint8_t *>(program.data());
    out->program.len = program.size();

    out->capability =
        static_cast<uint64_t>(ic->capabilityFlags().toInteger());
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
class CanTypeEngine : public fcitx::InputMethodEngineV2 {
public:
    CanTypeEngine(fcitx::Instance &instance)
        : instance_(instance),
          factory_([this](fcitx::InputContext & /*ic*/) {
              return new CanTypeProperty(nextContextId());
          }) {
        instance_.inputContextManager().registerProperty(kTenProperty,
                                                          &factory_);
        // Watch `InputContextFocusOut` để relinquish tường minh khi IC mất
        // focus, kể cả khi không có phím giữa focus-out và focus-in. Chỉ hành
        // động trên IC đã có Rust session (`prop->phien != nullptr`); các IC
        // khác chỉ tạo shell nhẹ (phien lazy) rồi bị bỏ qua. Handler table
        // entry phải sống cùng engine để callback không bị hủy.
        focus_out_watcher_ = instance_.watchEvent(
            fcitx::EventType::InputContextFocusOut,
            fcitx::EventWatcherPhase::Default,
            [this](fcitx::Event &event) { xuLyFocusOut(event); });
    }

    /// `activate`: IC chuyển sang input method này. Relinquish + active=true.
    void activate(const fcitx::InputMethodEntry & /*entry*/,
                  fcitx::InputContextEvent &event) override {
        auto *prop = propertyOf(event);
        if (prop == nullptr) {
            return;
        }
        prop->kichHoat();
    }

    /// `deactivate`: IC chuyển sang input method khác. Relinquish + active=
    /// false. (Override thay vì dựa default `deactivate`→`reset` để set
    /// active=false tường minh.)
    void deactivate(const fcitx::InputMethodEntry & /*entry*/,
                    fcitx::InputContextEvent &event) override {
        auto *prop = propertyOf(event);
        if (prop == nullptr) {
            return;
        }
        prop->voHieuHoa();
    }

    void keyEvent(const fcitx::InputMethodEntry & /*entry*/,
                  fcitx::KeyEvent &event) override {
        auto *ic = event.inputContext();
        if (ic == nullptr) {
            return;
        }
        auto *prop =
            static_cast<CanTypeProperty *>(ic->property(kTenProperty));
        if (prop == nullptr) {
            return;
        }
        // Tạo Rust session lazy lần đầu; nếu thất bại → passthrough (không
        // nuốt phím).
        if (!prop->damBaoPhien()) {
            return;
        }
        // Cập nhật focus generation trước khi gọi Rust (Rust kiểm tra ở đầu
        // xu_ly). Phát hiện focus-out ngay nếu nhận phím khi mất focus.
        prop->capNhatFocus(ic->hasFocus());

        const auto &key = event.key();

        // Key snapshot: điền từ KeyEvent/Key.
        CanTypeKeySnapshot snapshot;
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
        CanTypeHostBang bang;
        bang.ic = ic;
        bang.lay_boi_canh = lay_boi_canh_cb;
        bang.chen = chen_cb;
        bang.thay_the = thay_the_cb;

        auto ket_qua = cantype_xu_ly_phim(prop->phien, &snapshot, &bang);

        switch (ket_qua) {
        case CanTypeXuLy_DaApDung:
            // Runtime đã phát text. Không forward phím gốc.
            event.filterAndAccept();
            break;
        case CanTypeXuLy_MatDongBo:
            // Host có thể đã phát một phần. Không forward (tránh bản sao).
            event.filterAndAccept();
            break;
        case CanTypeXuLy_ChuyenTiep:
            // Passthrough: forward phím gốc (không filter, không accept).
            break;
        }
    }

    void reset(const fcitx::InputMethodEntry & /*entry*/,
               fcitx::InputContextEvent &event) override {
        auto *prop = propertyOf(event);
        if (prop == nullptr) {
            return;
        }
        // reset chỉ gọi khi ic focused (header Fcitx5); active vẫn true.
        prop->datLai();
    }

private:
    /// Lấy property từ event, trả `nullptr` nếu ic null. `property()` tạo
    /// lazy; cho activate/deactivate/reset điều này hợp lý vì ic đang dùng IM
    /// này.
    static CanTypeProperty *propertyOf(fcitx::InputContextEvent &event) {
        auto *ic = event.inputContext();
        if (ic == nullptr) {
            return nullptr;
        }
        return ic->propertyAs<CanTypeProperty>(kTenProperty);
    }

    /// Watcher FocusOut: relinquish IC có Rust session khi mất focus.
    void xuLyFocusOut(fcitx::Event &event) {
        if (event.type() != fcitx::EventType::InputContextFocusOut) {
            return;
        }
        auto *ic_event = static_cast<fcitx::InputContextEvent *>(&event);
        auto *ic = ic_event->inputContext();
        if (ic == nullptr) {
            return;
        }
        auto *prop = ic->propertyAs<CanTypeProperty>(kTenProperty);
        // Bỏ qua IC chưa có Rust session (chưa gõ tiếng Việt): `phien` lazy.
        if (prop == nullptr || prop->phien == nullptr) {
            return;
        }
        prop->voHieuHoa();
    }

    /// Ánh xạ keysym sang `CanTypeKeyDacBiet`.
    static CanTypeKeyDacBiet dacBietTuKeysym(fcitx::KeySym sym) {
        switch (sym) {
        case FcitxKey_BackSpace:
            return CanTypeKeyBackspace;
        case FcitxKey_Delete:
            // XK_Delete (0xFFFF) → keySymToUTF8 trả U+007F (DEL) — nếu không
            // phân loại đặc biệt, runtime nhận nó như ký tự in được và commit
            // DEL vào host, hỏng composition. Delete phía trước con trỏ khác
            // Backspace (xóa lùi); map sang CanTypeKeyDelete để runtime
            // relinquish + passthrough (không XoaLui, không commit U+007F).
            return CanTypeKeyDelete;
        case FcitxKey_Escape:
            return CanTypeKeyEscape;
        case FcitxKey_Return:
            return CanTypeKeyEnter;
        case FcitxKey_Tab:
            return CanTypeKeyTab;
        default:
            return CanTypeKeyKhac;
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
    fcitx::FactoryFor<CanTypeProperty> factory_;
    /// Giữ handler FocusOut sống cùng engine (hủy khi engine destruct).
    std::unique_ptr<fcitx::HandlerTableEntry<fcitx::EventHandler>>
        focus_out_watcher_;
    uint64_t next_context_id_ = 1;
};

/// Addon factory: tạo engine khi addon được nạp.
class CanTypeFactory : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override {
        return new CanTypeEngine(*manager->instance());
    }
};

} // namespace

/// Trả static `CanTypeFactory`. Rust `fcitx_addon_factory_instance` (xuất bằng
/// `#[no_mangle]` trong `src/ffi.rs`) gọi hàm này. Function-local static,
/// thread-safe init (C++11). Không dùng `FCITX_ADDON_FACTORY` vì Rust cdylib
/// chỉ export symbol `#[no_mangle]` Rust (version script).
extern "C" void *cantype_native_factory(void) {
    static CanTypeFactory factory;
    return &factory;
}
