// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

#![deny(future_incompatible)]
#![deny(nonstandard_style)]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![allow(rustdoc::private_intra_doc_links)]

//! *restrace* runs a command under [`strace`](https://strace.io) and parses
//! the emitted system calls into Rust values.
//!
//! It is easier to explain the crate in terms of the types it provides:
//! - [`Event`]: a single, complete line of `strace` output, carrying the
//!   pid, the timestamp and a [`syscalls::Syscall`].
//! - [`syscalls`]: typed records for the supported system calls behind the
//!   [`syscalls::Syscall`] enum, plus a [`syscalls::Generic`] fallback.
//! - [`stdprocess`]: run a [`std::process::Command`] under `strace` and get a
//!   blocking iterator of [`Event`]s with [`stdprocess::stream`].
//! - [`tokio`]: the same for a `tokio::process::Command`, as an async stream
//!   of [`Event`]s; only compiled with the `tokio` feature, which is
//!   enabled by default.
//!
//! `strace` must be installed in the environment where the commands run; this
//! crate only spawns it and parses its output.
//!
//! # Parsing trace lines
//!
//! An [`Event`] parses the `strace` output produced with
//! `--absolute-timestamps=format:unix,precision:us`, `--follow-forks` and a
//! trace redirected with `-o`, which makes `strace` prefix every line with the
//! pid of the process that made the call:
//!
//! ```rust
//! use restrace::Event;
//!
//! let line: Event = "251751 1700000000.000000 execve(\"/bin/echo\", [\"echo\"], []) = 0"
//!     .parse()
//!     .unwrap();
//! assert_eq!(line.pid, 251751);
//! ```
//!
//! Every supported call parses into its own type inside [`syscalls`]; calls
//! without a dedicated type fall back to [`syscalls::Generic`], which keeps
//! the raw arguments and return value.
//!
//! # Running a command under `strace`
//!
//! [`stdprocess::stream`] wraps a [`std::process::Command`] under `strace` and
//! returns a blocking [`stdprocess::EventIter`] of parsed lines. The trace is
//! read from a dedicated pipe, so the standard streams of the traced process
//! are left untouched:
//!
//! ```rust,no_run
//! use std::process::Command;
//!
//! let mut cmd = Command::new("ls");
//! cmd.arg("-l");
//! let mut lines = restrace::stdprocess::stream(cmd).unwrap();
//! for line in lines.by_ref() {
//!     println!("{line}");
//! }
//! lines.wait().unwrap();
//! ```
//!
//! # Tokio
//!
//! With the `tokio` feature (enabled by default), [`tokio::stream`] does the
//! same for a `tokio::process::Command`, returning a [`tokio::EventStream`] of
//! [`Event`]s:
//!
//! ```rust,no_run
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use tokio::process::Command;
//! use tokio_stream::StreamExt;
//!
//! let mut cmd = Command::new("ls");
//! cmd.arg("-l");
//! let mut lines = restrace::tokio::stream(cmd)?;
//! while let Some(line) = lines.next().await {
//!     println!("{line}");
//! }
//! lines.wait().await?;
//! # Ok(()) }
//! ```

mod event;
pub mod stdprocess;
pub mod syscalls;

/// Tokio integration, only compiled with the `tokio` feature.
#[cfg(feature = "tokio")]
pub mod tokio;

pub use event::Event;
pub use syscalls::{ParseError, Syscall};
