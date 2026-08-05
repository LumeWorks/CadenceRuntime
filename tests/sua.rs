// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Test thuật toán sửa committed text (§14).
//!
//! Bao phủ: chuỗi giống nhau, insert vào rỗng, xóa toàn bộ, thay ký tự cuối,
//! thay nhiều ký tự cuối, thay dấu ở giữa từ, Unicode NFC tiếng Việt, surrogate
//! pair UTF-16, không cắt sai UTF-8 boundary, xóa không vượt đoạn sở hữu.

use cadence_runtime::{DoDaiVanBan, KeHoachSua};

fn sua(cu: &str, moi: &str) -> KeHoachSua {
    KeHoachSua::tinh(cu, moi)
}

fn dodai(s: &str) -> DoDaiVanBan {
    DoDaiVanBan::tinh(s)
}

#[test]
fn hai_chuoi_giong_nhau_tra_ke_hoach_rong() {
    let ke = sua("tiếng", "tiếng");
    assert!(ke.la_rong());
    assert!(ke.xoa_truoc.la_rong());
    assert!(ke.chen.is_empty());
    // Không có thao tác phá hoại khi hai chuỗi giống nhau.
    assert_eq!(ke.xoa_truoc.byte_utf8, 0);
}

#[test]
fn insert_vao_chuoi_rong() {
    let ke = sua("", "abc");
    assert!(ke.la_chen());
    assert!(ke.xoa_truoc.la_rong());
    assert_eq!(ke.chen, "abc");
}

#[test]
fn xoa_toan_bo_chuoi() {
    let ke = sua("abc", "");
    assert!(ke.la_xoa());
    assert_eq!(ke.xoa_truoc, dodai("abc"));
    assert!(ke.chen.is_empty());
}

#[test]
fn thay_ky_tu_cuoi_tie_thanh_tiê() {
    // Cũ "tie", mới "tiê": common "ti" (2 byte), xóa "e" (1 ký tự), chèn "ê".
    let ke = sua("tie", "tiê");
    assert!(!ke.la_rong());
    assert_eq!(
        ke.xoa_truoc,
        DoDaiVanBan {
            byte_utf8: 1,
            don_vi_utf16: 1,
            ky_tu_unicode: 1,
        }
    );
    assert_eq!(ke.chen, "ê");
}

#[test]
fn thay_nhieu_ky_tu_cuoi_hoa_thanh_hoa() {
    // Cũ "hoa", mới "hóa": common "h" (1 byte), xóa "oa" (2 ký tự), chèn "óa".
    let ke = sua("hoa", "hóa");
    assert_eq!(
        ke.xoa_truoc,
        DoDaiVanBan {
            byte_utf8: 2,
            don_vi_utf16: 2,
            ky_tu_unicode: 2,
        }
    );
    assert_eq!(ke.chen, "óa");
}

#[test]
fn thay_dau_o_giua_tu_van_xoa_chen_suffix() {
    // Runtime chỉ có thể xóa phía trước con trỏ rồi chèn lại suffix, nên thay
    // dấu ở giữa từ vẫn phải xóa từ sau common prefix đến cuối.
    // "hoàn" → "hoán": common "ho" (2 byte), xóa "àn" (3 ký tự), chèn "án".
    let ke = sua("hoàn", "hoán");
    assert_eq!(ke.xoa_truoc, dodai("àn"));
    assert_eq!(ke.chen, "án");
}

#[test]
fn unicode_nfc_tieng_viet_tieengs_sang_tieng() {
    // Cũ "tieng", mới "tiếng". char 2: 'e' != 'ế' nên common là "ti" (2 byte),
    // không phải "tie". xóa "eng" (3 ký tự), chèn "ếng".
    let ke = sua("tieng", "tiếng");
    assert_eq!(ke.xoa_truoc, dodai("eng"));
    assert_eq!(ke.chen, "ếng");
}

#[test]
fn utf16_surrogate_pair_dung_2_don_vi() {
    // U+1F600 (😀): 4 byte UTF-8, 2 đơn vị UTF-16, 1 codepoint.
    let ke = sua("😀", "a");
    assert_eq!(
        ke.xoa_truoc,
        DoDaiVanBan {
            byte_utf8: 4,
            don_vi_utf16: 2,
            ky_tu_unicode: 1,
        }
    );
    assert_eq!(ke.chen, "a");
}

#[test]
fn khong_cat_sai_utf8_boundary_xoa_sau_multibyte() {
    // Common prefix phải dừng tại ranh giới char, không cắt giữa byte của "ế".
    // "ếng" → "ến": common "ến" (3 byte, nguyên "ế" 2 byte + "n" 1 byte),
    // xóa "g" (1 byte), chèn "" (rỗng). Kiểm chứng ranh giới UTF-8 an toàn.
    let ke = sua("ếng", "ến");
    assert_eq!(ke.xoa_truoc, dodai("g"));
    assert!(ke.chen.is_empty());
    // Xóa đúng 1 byte (toàn bộ "g"), không phải cắt giữa "ế" (2 byte).
    assert_eq!(ke.xoa_truoc.byte_utf8, 1);
}

#[test]
fn xoa_khong_vuot_doan_runtime_so_huu() {
    // xoa_truoc được tính từ cu_tail (suffix của cu). Khi runtime truyền
    // cu = da_hien_thi (đoạn sở hữu), xoa_truoc không vượt quá cu.
    let da_hien_thi = "tiếng";
    let ke = sua(da_hien_thi, "tiêng");
    assert!(
        ke.xoa_truoc.byte_utf8 <= da_hien_thi.len(),
        "xoa_truoc ({}) khong duoc vuot da_hien_thi ({})",
        ke.xoa_truoc.byte_utf8,
        da_hien_thi.len()
    );
    // Cụ thể: common "ti" (2 byte, vì 'ế' != 'ê'), xóa "ếng", chèn "êng".
    // Vẫn <= da_hien_thi.
    assert_eq!(ke.xoa_truoc, dodai("ếng"));
}

#[test]
fn chen_vao_cuoi_khong_xoa() {
    // Cũ "a", mới "ab": common "a", xóa "" (rỗng), chèn "b".
    let ke = sua("a", "ab");
    assert!(ke.la_chen());
    assert!(ke.xoa_truoc.la_rong());
    assert_eq!(ke.chen, "b");
}

#[test]
fn doi_dau_thanh_oa_thanh_óa() {
    // Trường hợp Cadence: "hoa" → "hóa" (đã test ở trên), thử "òa" → "óa".
    let ke = sua("òa", "óa");
    // common "" (vì 'ò' != 'ó'), xóa "òa", chèn "óa".
    assert_eq!(ke.xoa_truoc, dodai("òa"));
    assert_eq!(ke.chen, "óa");
}

// --- Unicode diff: NFD, combining marks, emoji ZWJ ---

#[test]
fn nfd_combining_mark_khong_cat_sai_boundary() {
    // NFD "é" = "e" + U+0301 (combining acute). NFC "é" = U+00E9.
    // Common prefix = 0 vì 'e' != 'é'. Xóa toàn bộ NFD, chèn NFC.
    let nfd = "e\u{0301}";
    let nfc = "é";
    let ke = sua(nfd, nfc);
    assert_eq!(ke.xoa_truoc, dodai(nfd));
    assert_eq!(
        ke.xoa_truoc.ky_tu_unicode, 2,
        "NFD e+U+0301 la 2 code point"
    );
    assert_eq!(ke.chen, nfc);
}

#[test]
fn combining_mark_append_la_chen() {
    // "tie" → "tie\u{0301}" (append combining mark). Common prefix "tie"
    // (3 byte), xóa "" (rỗng), chèn "\u{0301}". la_chen = true.
    let ke = sua("tie", "tie\u{0301}");
    assert!(ke.la_chen());
    assert!(ke.xoa_truoc.la_rong());
    assert_eq!(ke.chen, "\u{0301}");
}

#[test]
fn emoji_zwj_char_level_diff() {
    // "👨‍👩‍👧" = '👨' + U+200D + '👩' + U+200D + '👧' (5 code point, 1 grapheme).
    // "👨‍👩‍👦" = '👨' + U+200D + '👩' + U+200D + '👦' (5 code point, 1 grapheme).
    // Common prefix: 4 code point khớp, '👧' != '👦' → break.
    let cu = "👨‍👩‍👧";
    let moi = "👨‍👩‍👦";
    let ke = sua(cu, moi);
    // xoa_truoc = "👧" (1 code point, 4 byte UTF-8, 2 đơn vị UTF-16).
    assert_eq!(ke.xoa_truoc.ky_tu_unicode, 1);
    assert_eq!(ke.xoa_truoc.byte_utf8, 4);
    assert_eq!(ke.xoa_truoc.don_vi_utf16, 2);
    // chen = "👦".
    assert_eq!(ke.chen, "👦");
}

#[test]
fn ky_tu_unicode_dem_codepoint_khong_phai_grapheme() {
    // "👨‍👩‍👧" là 1 grapheme nhưng 5 code point. ky_tu_unicode đếm code point.
    let d = dodai("👨‍👩‍👧");
    assert_eq!(d.ky_tu_unicode, 5, "5 code point, khong phai 1 grapheme");
    assert_eq!(d.byte_utf8, 18); // 4+3+4+3+4
    assert_eq!(d.don_vi_utf16, 8); // 2+1+2+1+2
}
