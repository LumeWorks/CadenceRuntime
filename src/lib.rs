// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Lê Hùng Quang Minh

//! CadenceRuntime — runtime thuần Rust, độc lập nền tảng cho lõi gõ tiếng Việt
//! Cadence.
//!
//! Phase 1 dựng nền móng xử lý phiên nhập, tích hợp Cadence, tính kế hoạch sửa
//! committed text, bảo vệ đồng bộ context và mô phỏng host. Xem
//! [`docs/PHASE_1_RUNTIME.md`] cho thiết kế đầy đủ.
//!
//! [`docs/PHASE_1_RUNTIME.md`]: https://github.com/LumeWorks/CadenceRuntime

#![forbid(unsafe_code)]
#![warn(missing_docs)]
