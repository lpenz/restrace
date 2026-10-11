// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Rust template library.
#![deny(future_incompatible)]
#![deny(nonstandard_style)]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![allow(rustdoc::private_intra_doc_links)]

pub mod line;
pub mod stdprocess;
pub mod syscalls;

/// Tokio integration, only compiled with the `tokio` feature.
#[cfg(feature = "tokio")]
pub mod tokio;
