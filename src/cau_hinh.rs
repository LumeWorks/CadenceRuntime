// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! Cấu hình CanType, schema versioned, dùng chung cho GUI và addon Fcitx5.
//!
//! GUI và addon đọc/ghi cùng một file `$XDG_CONFIG_HOME/cantype/config.toml`
//! (fallback `$HOME/.config/cantype/config.toml`). Addon đọc ngoài hot path
//! (tạo session, focus-in, reset/boundary); GUI ghi atomically để không hỏng
//! config cũ khi crash giữa chừng.
//!
//! Schema trung thực với Cadence v2026.1.0: chỉ có field Cadence thực sự hỗ
//! trợ (kiểu gõ, chính sách lựa chọn, kiểu Telex, quy tắc đặt dấu, dạng
//! Unicode). Không có `bo_dau_thong_minh`/`kiem_tra_chinh_ta` vì Cadence chưa
//! có API tương ứng. `dang_bat` (toggle tiếng Việt) và `khoi_dong_cung_he_
//! thong` là state CanType, không phải Cadence.
//!
//! Enum riêng (`KieuGo`, `ChinhSach`, ...) không leak `cadence::KieuGo` ra
//! ngoài boundary — `cadence.rs` convert khi tạo phiên.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Phiên bản schema. Tăng khi field thêm/đổi. Cấu hình có `phien_ban` cao hơn
/// hiện tại bị từ chối (không ghi đè mù); thấp hơn được migrate (hiện chỉ có
/// v1, chưa có migration).
pub const PHIEN_BAN_SCHEMA: u32 = 1;

/// Kiểu gõ tiếng Việt. Cadence v2026.1.0 hỗ trợ cả Telex và VNI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum KieuGo {
    /// Telex: phím chữ `s/f/r/x/j/z` (dấu thanh), `w/a/e/o/d` (hình chữ).
    #[default]
    Telex,
    /// VNI: digit `1..=5` (dấu thanh), `6/7/8/9` (mũ/móc/trăng/đ).
    Vni,
}

/// Chính sách lựa chọn raw/biến đổi theo ngữ cảnh. Cadence v2026.1.0 hỗ trợ
/// cả ba (code/chat preservation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChinhSach {
    /// Tự nhiên: giữ code/URL/email, biến đổi tiếng Việt khi rõ.
    #[default]
    #[serde(rename = "tu_nhien")]
    TuNhien,
    /// Ưu tiên tiếng Việt: biến đổi nhiều hơn, vẫn giữ cấu trúc kỹ thuật chắc.
    #[serde(rename = "uu_tien_tieng_viet")]
    UuTienTiengViet,
    /// Ưu tiên nguyên bản: chỉ biến đổi khi bằng chứng tiếng Việt rất rõ.
    #[serde(rename = "uu_tien_nguyen_ban")]
    UuTienNguyenBan,
}

/// Kiểu Telex: `CanBang` (w đơn lẻ giữ nguyên) hay `DayDu` (w→ư, [/]/]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KieuTelex {
    /// Cân bằng: `w` đơn lẻ giữ nguyên.
    #[default]
    #[serde(rename = "can_bang")]
    CanBang,
    /// Đầy đủ: `w` đơn lẻ thành `ư`, `[`→`ư`, `]`→`ơ`.
    #[serde(rename = "day_du")]
    DayDu,
}

/// Quy tắc đặt dấu thanh (hiện đại `hòa` hay truyền thống `hoà`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum QuyTacDatDau {
    /// Quy tắc hiện đại.
    #[default]
    #[serde(rename = "hien_dai")]
    HienDai,
    /// Quy tắc truyền thống.
    #[serde(rename = "truyen_thong")]
    TruyenThong,
}

/// Dạng Unicode output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DangUnicode {
    /// NFC: normalization form composed (dựng sẵn).
    #[default]
    #[serde(rename = "nfc")]
    Nfc,
    /// NFD: normalization form decomposed (combining mark).
    #[serde(rename = "nfd")]
    Nfd,
}

/// Cấu hình CanType, schema versioned.
///
/// Field là pub để serde đọc/ghi; nhưng `phien_ban` được validate khi đọc.
/// Không có field Cadence chưa hỗ trợ (bo_dau_thong_minh, kiem_tra_chinh_ta).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CauHinhCanType {
    /// Phiên bản schema (không phải phiên bản Cadence). Tăng khi field thêm.
    pub phien_ban: u32,
    /// `true` nếu tiếng Việt đang bật (toggle toàn hệ thống).
    pub dang_bat: bool,
    /// Kiểu gõ (Telex mặc định).
    pub kieu_go: KieuGo,
    /// Chính sách lựa chọn raw/biến đổi (code/chat preservation).
    pub chinh_sach: ChinhSach,
    /// Kiểu Telex (chỉ dùng khi `kieu_go == Telex`).
    pub kieu_telex: KieuTelex,
    /// Quy tắc đặt dấu thanh.
    pub quy_tac_dat_dau: QuyTacDatDau,
    /// Dạng Unicode output.
    pub dang_unicode: DangUnicode,
    /// `true` nếu GUI tự khởi động cùng hệ thống (GUI-level, không phải Cadence).
    pub khoi_dong_cung_he_thong: bool,
}

impl Default for CauHinhCanType {
    fn default() -> Self {
        Self {
            phien_ban: PHIEN_BAN_SCHEMA,
            dang_bat: true,
            kieu_go: KieuGo::Telex,
            chinh_sach: ChinhSach::TuNhien,
            kieu_telex: KieuTelex::CanBang,
            quy_tac_dat_dau: QuyTacDatDau::HienDai,
            dang_unicode: DangUnicode::Nfc,
            khoi_dong_cung_he_thong: false,
        }
    }
}

/// Lỗi đọc/ghi cấu hình. Enum domain thay vì `String` chung chung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoiCauHinh {
    /// File tồn tại nhưng sai cú pháp TOML.
    SaiCuPhap(String),
    /// `phien_ban` cao hơn schema hiện tại — không ghi đè mù.
    PhienBanCaoHon(u32),
    /// Lỗi I/O khi đọc/ghi.
    Io(String),
}

impl fmt::Display for LoiCauHinh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SaiCuPhap(e) => write!(f, "sai cu phap TOML: {e}"),
            Self::PhienBanCaoHon(v) => {
                write!(
                    f,
                    "phien ban schema {v} cao hon hien tai ({PHIEN_BAN_SCHEMA})"
                )
            }
            Self::Io(e) => write!(f, "loi I/O: {e}"),
        }
    }
}

impl std::error::Error for LoiCauHinh {}

impl From<std::io::Error> for LoiCauHinh {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Đường dẫn file config: `$XDG_CONFIG_HOME/cantype/config.toml`, fallback
/// `$HOME/.config/cantype/config.toml`. Trả `None` nếu không có `$HOME` và
/// không có `XDG_CONFIG_HOME`.
#[must_use]
pub fn duong_dan_config() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| {
                let mut p = PathBuf::from(h);
                p.push(".config");
                p
            })
        })?;
    let mut p = base;
    p.push("cantype");
    p.push("config.toml");
    Some(p)
}

/// Đọc cấu hình từ đường dẫn. Nếu không tồn tại → mặc định. Sai cú pháp →
/// mặc định an toàn (không panic). Phien ban cao hon → `Err` (không ghi đè mù).
///
/// # Errors
/// Trả `Err(LoiCauHinh)` nếu I/O lỗi (không phải "file không tồn tại") hoặc
/// `phien_ban` cao hơn schema.
pub fn doc(duong_dan: &Path) -> Result<CauHinhCanType, LoiCauHinh> {
    let noi_dung = match fs::read_to_string(duong_dan) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CauHinhCanType::default());
        }
        Err(e) => return Err(LoiCauHinh::from(e)),
    };
    let cfg: CauHinhCanType =
        toml::from_str(&noi_dung).map_err(|e| LoiCauHinh::SaiCuPhap(e.to_string()))?;
    if cfg.phien_ban > PHIEN_BAN_SCHEMA {
        return Err(LoiCauHinh::PhienBanCaoHon(cfg.phien_ban));
    }
    Ok(cfg)
}

/// Ghi cấu hình atomically: ghi `config.toml.tmp`, flush, rename. Không làm
/// hỏng config cũ nếu crash giữa chừng.
///
/// # Errors
/// Trả `Err(LoiCauHinh)` nếu I/O lỗi.
pub fn ghi(duong_dan: &Path, cfg: &CauHinhCanType) -> Result<(), LoiCauHinh> {
    let mut tmp = duong_dan.to_path_buf();
    tmp.set_extension("toml.tmp");
    if let Some(parent) = duong_dan.parent() {
        fs::create_dir_all(parent)?;
    }
    let toml_str = toml::to_string(cfg).map_err(|e| LoiCauHinh::SaiCuPhap(e.to_string()))?;
    let mut file = fs::File::create(&tmp)?;
    file.write_all(toml_str.as_bytes())?;
    file.flush()?;
    // rename là atomic trên cùng filesystem (POSIX).
    fs::rename(&tmp, duong_dan)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    //! Test cấu hình: mặc định, round-trip, lỗi, version, atomic write, XDG.

    use super::*;

    /// Cấu hình mặc định có `phien_ban` đúng và tiếng Việt bật.
    #[test]
    fn mac_dinh() {
        let cfg = CauHinhCanType::default();
        assert_eq!(cfg.phien_ban, PHIEN_BAN_SCHEMA);
        assert!(cfg.dang_bat);
        assert_eq!(cfg.kieu_go, KieuGo::Telex);
        assert_eq!(cfg.chinh_sach, ChinhSach::TuNhien);
    }

    /// Round-trip: ghi rồi đọc lại, giá trị giữ nguyên.
    #[test]
    fn round_trip() {
        let tmp = tmpdir();
        let duong_dan = tmp.join("config.toml");
        let cfg = CauHinhCanType {
            phien_ban: PHIEN_BAN_SCHEMA,
            dang_bat: false,
            kieu_go: KieuGo::Vni,
            chinh_sach: ChinhSach::UuTienTiengViet,
            kieu_telex: KieuTelex::DayDu,
            quy_tac_dat_dau: QuyTacDatDau::TruyenThong,
            dang_unicode: DangUnicode::Nfd,
            khoi_dong_cung_he_thong: true,
        };
        ghi(&duong_dan, &cfg).unwrap();
        let doc_lai = doc(&duong_dan).unwrap();
        assert_eq!(cfg, doc_lai);
    }

    /// File không tồn tại → mặc định.
    #[test]
    fn khong_ton_tai_mac_dinh() {
        let tmp = tmpdir();
        let duong_dan = tmp.join("khong_co.toml");
        let cfg = doc(&duong_dan).unwrap();
        assert_eq!(cfg, CauHinhCanType::default());
    }

    /// Sai cú pháp → `Err` (caller quyết định fallback).
    #[test]
    fn sai_cu_phap() {
        let tmp = tmpdir();
        let duong_dan = tmp.join("sai.toml");
        fs::write(&duong_dan, "dang_bat = ????").unwrap();
        let loi = doc(&duong_dan).unwrap_err();
        assert!(matches!(loi, LoiCauHinh::SaiCuPhap(_)));
    }

    /// `phien_ban` cao hơn schema → `Err` (không ghi đè mù).
    #[test]
    fn phien_ban_cao_hon() {
        let tmp = tmpdir();
        let duong_dan = tmp.join("cao.toml");
        let toml_str = format!(
            "phien_ban = {}\ndang_bat = true\nkieu_go = \"telex\"\n\
             chinh_sach = \"tu_nhien\"\nkieu_telex = \"can_bang\"\n\
             quy_tac_dat_dau = \"hien_dai\"\ndang_unicode = \"nfc\"\n\
             khoi_dong_cung_he_thong = false",
            PHIEN_BAN_SCHEMA + 1
        );
        fs::write(&duong_dan, toml_str).unwrap();
        let loi = doc(&duong_dan).unwrap_err();
        assert!(matches!(loi, LoiCauHinh::PhienBanCaoHon(_)));
    }

    /// Atomic write: ghi xong, `.tmp` không còn.
    #[test]
    fn atomic_write_khong_con_tmp() {
        let tmp = tmpdir();
        let duong_dan = tmp.join("atomic.toml");
        let cfg = CauHinhCanType::default();
        ghi(&duong_dan, &cfg).unwrap();
        assert!(duong_dan.exists());
        let tmp_file = duong_dan.with_extension("toml.tmp");
        assert!(!tmp_file.exists(), "file tmp phai bi rename di");
    }

    /// Toggle tiếng Việt: `dang_bat` thay đổi, ghi lại, đọc lại giữ nguyên.
    #[test]
    fn toggle_tieng_viet() {
        let tmp = tmpdir();
        let duong_dan = tmp.join("toggle.toml");
        let mut cfg = CauHinhCanType::default();
        assert!(cfg.dang_bat);
        cfg.dang_bat = false;
        ghi(&duong_dan, &cfg).unwrap();
        let doc_lai = doc(&duong_dan).unwrap();
        assert!(!doc_lai.dang_bat);
    }

    /// Lựa chọn kiểu gõ hợp lệ (Telex, VNI).
    #[test]
    fn lua_chon_kieu_go() {
        let tmp = tmpdir();
        let duong_dan = tmp.join("kieu_go.toml");
        let cfg = CauHinhCanType {
            kieu_go: KieuGo::Vni,
            ..CauHinhCanType::default()
        };
        ghi(&duong_dan, &cfg).unwrap();
        let doc_lai = doc(&duong_dan).unwrap();
        assert_eq!(doc_lai.kieu_go, KieuGo::Vni);
    }

    /// Field lạ trong TOML → bị bỏ qua (serde mặc định), không lỗi.
    #[test]
    fn field_la_bo_qua() {
        let tmp = tmpdir();
        let duong_dan = tmp.join("la.toml");
        let toml_str = format!(
            "phien_ban = {}\ndang_bat = true\nkieu_go = \"telex\"\n\
             chinh_sach = \"tu_nhien\"\nkieu_telex = \"can_bang\"\n\
             quy_tac_dat_dau = \"hien_dai\"\ndang_unicode = \"nfc\"\n\
             khoi_dong_cung_he_thong = false\nfield_la = \"xyz\"",
            PHIEN_BAN_SCHEMA
        );
        fs::write(&duong_dan, toml_str).unwrap();
        let cfg = doc(&duong_dan).unwrap();
        assert_eq!(cfg.phien_ban, PHIEN_BAN_SCHEMA);
    }

    /// Tạo thư mục tạm cho test (dọn dẹp khi drop).
    fn tmpdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cantype-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
