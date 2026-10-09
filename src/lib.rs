// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Rust template library.
#![deny(future_incompatible)]
#![deny(nonstandard_style)]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![allow(rustdoc::private_intra_doc_links)]

mod cli;
pub mod line;
pub mod stdprocess;
pub mod syscalls;

/// Tokio integration, only compiled with the `tokio` feature.
#[cfg(feature = "tokio")]
pub mod tokio;

use clap::Parser;
use std::error::Error;

/// main function, the single pub function in this lib.
#[::tokio::main(crate = "::tokio", flavor = "current_thread")]
pub async fn main() -> Result<(), Box<dyn Error>> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::ACTIVE)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = cli::Cli::parse();
    eprintln!("{:?}", args);
    Ok(())
}
