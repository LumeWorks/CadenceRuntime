// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Kế hoạch sửa committed text: tính thay đổi từ nội dung đã hiển thị sang
//! nội dung Cadence mới.
//!
//! Thuật toán cố tình đơn giản (không phải general-purpose diff, không thêm
//! dependency diff):
//!
//! 1. Tìm common prefix dài nhất tại ranh giới Unicode hợp lệ.
//! 2. Phần còn lại của chuỗi cũ trở thành đoạn cần xóa trước con trỏ.
//! 3. Phần còn lại của chuỗi mới trở thành đoạn cần chèn.
//! 4. Runtime chỉ truyền vào `cu = da_hien_thi` (đoạn runtime sở hữu), nên
//!    `xoa_truoc` không bao giờ vượt nội dung runtime đã ghi nhận.
//!
//! [`KeHoachSua::tinh`] trả đủ ba đơn vị chiều dài (byte UTF-8, đơn vị UTF-16,
//! ký tự Unicode) để adapter nền tảng tương lai (UTF-16 như Windows TSF, hoặc
//! codepoint) không phải tự tính lại sai.

/// Độ dài văn bản theo ba đơn vị trung lập nền tảng.
///
/// `ky_tu_unicode` đếm **code point** (Unicode scalar value, `char::count`),
/// không phải grapheme. Một emoji ZWJ như "👨‍👩‍👧" là 5 code point nhưng 1
/// grapheme. Chưa thêm grapheme count vì API Cadence và test Phase 1 chưa
/// chứng minh cần thiết; thêm `unicode-segmentation` chỉ để phòng tương lai sẽ
/// là dependency thừa. Adapter nào cần grapheme count phải tự tính.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoDaiVanBan {
    /// Số byte UTF-8.
    pub byte_utf8: usize,
    /// Số đơn vị UTF-16 (một ký tự ngoài BMP chiếm 2 đơn vị).
    pub don_vi_utf16: usize,
    /// Số code point (Unicode scalar value), không phải grapheme.
    pub ky_tu_unicode: usize,
}

impl DoDaiVanBan {
    /// Độ dài rỗng (0/0/0).
    #[must_use]
    pub fn khong() -> Self {
        Self {
            byte_utf8: 0,
            don_vi_utf16: 0,
            ky_tu_unicode: 0,
        }
    }

    /// Tính độ dài của một chuỗi theo cả ba đơn vị.
    #[must_use]
    pub fn tinh(van_ban: &str) -> Self {
        Self {
            byte_utf8: van_ban.len(),
            don_vi_utf16: van_ban.encode_utf16().count(),
            ky_tu_unicode: van_ban.chars().count(),
        }
    }

    /// Trả `true` nếu độ dài rỗng.
    #[must_use]
    pub fn la_rong(self) -> bool {
        self.byte_utf8 == 0
    }
}

/// Kế hoạch sửa một đoạn committed text: xóa `xoa_truoc` trước con trỏ rồi chèn
/// `chen`. Đây là một hành động logic duy nhất; adapter nền tảng có thể thực
/// thi bằng delete rồi commit nhưng runtime chỉ thấy một `ThayThe`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeHoachSua {
    /// Đoạn cần xóa ngay trước con trỏ.
    pub xoa_truoc: DoDaiVanBan,
    /// Chuỗi cần chèn tại con trỏ sau khi xóa.
    pub chen: String,
}

impl KeHoachSua {
    /// Tính kế hoạch sửa từ `cu` (đã hiển thị) sang `moi` (Cadence mới).
    ///
    /// Common prefix được tìm theo ranh giới `char` (không bao giờ cắt giữa một
    /// code point UTF-8). Nếu hai chuỗi giống nhau, trả kế hoạch rỗng (không
    /// thao tác phá hoại).
    #[must_use]
    pub fn tinh(cu: &str, moi: &str) -> Self {
        let mut common_byte = 0usize;
        for (ky_cu, ky_moi) in cu.chars().zip(moi.chars()) {
            if ky_cu != ky_moi {
                break;
            }
            common_byte += ky_cu.len_utf8();
        }
        // common_byte nằm tại ranh giới char của cả cu lẫn moi vì các char khớp
        // từng đôi, nên slice an toàn.
        let cu_tail = &cu[common_byte..];
        let moi_tail = &moi[common_byte..];
        Self {
            xoa_truoc: DoDaiVanBan::tinh(cu_tail),
            chen: String::from(moi_tail),
        }
    }

    /// Trả `true` nếu kế hoạch không xóa và không chèn (hai chuỗi giống nhau).
    #[must_use]
    pub fn la_rong(&self) -> bool {
        self.xoa_truoc.la_rong() && self.chen.is_empty()
    }

    /// Trả `true` nếu kế hoạch chỉ xóa (không chèn) - mô phỏng backspace thuần.
    #[must_use]
    pub fn la_xoa(&self) -> bool {
        !self.xoa_truoc.la_rong() && self.chen.is_empty()
    }

    /// Trả `true` nếu kế hoạch chỉ chèn (không xóa) - insert thuần.
    #[must_use]
    pub fn la_chen(&self) -> bool {
        self.xoa_truoc.la_rong() && !self.chen.is_empty()
    }
}
