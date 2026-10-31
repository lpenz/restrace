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
//!   pid, the timestamp and an [`Action`].
//! - [`Action`]: what the line reports: a system call record or the [`Exit`]
//!   that terminated the process.
//! - [`syscalls`]: typed records for the supported system calls, plus a
//!   [`syscalls::Generic`] fallback for calls without a dedicated type.
//! - [`Reconstructor`]: joins the two lines of a call that `strace` split
//!   across an `<unfinished ...>` / `<... resumed>` pair, for traces read from
//!   a file or any other [`std::io::BufRead`].
//! - [`stdprocess`]: run a [`std::process::Command`] under `strace` and get a
//!   blocking iterator of parsed [`Event`]s, each wrapped in a
//!   [`std::io::Result`], with [`stdprocess::stream`].
//! - [`tokio`]: the same for a `tokio::process::Command`, as an async stream
//!   of [`Event`]s wrapped in [`std::io::Result`]; only compiled with the
//!   `tokio` feature, which is enabled by default.
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
//! the raw arguments and return value. The `+++ exited with ... +++` and
//! `+++ killed by ... +++` lines that report the end of a traced process
//! parse into [`Exit`], carried as [`Action::Exit`].
//!
//! # Reconstructing split calls
//!
//! When more than one process or thread is traced at a time, a call that is
//! still running when another traced process emits output is printed in two
//! lines: the beginning ends with `<unfinished ...>` and the end begins with
//! `<... name resumed>`. [`stdprocess::stream`] and [`tokio::stream`] join
//! the two lines of every pid into the single [`Event`] the call would
//! otherwise produce, keeping the pid and the timestamp of the unfinished
//! line. A trace read from a file can be reconstructed the same way with
//! [`Reconstructor`].
//!
//! Reconstructed events keep the timestamp of the unfinished line, so they
//! are not necessarily ordered by time: a call that started early can be
//! emitted after one that completed in between. A resumed call whose merged
//! text is not decodable, such as one that resumes with the indeterminate
//! result `= ?`, is dropped, as it would be if `strace` had printed it on one
//! line.
//!
//! # Parsing a trace file
//!
//! A trace that `strace` wrote to a file with `-o FILE` can be parsed without
//! going through the streaming APIs. Feed the lines to a [`Reconstructor`] in
//! the order `strace` wrote them:
//!
//! ```rust,no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use restrace::Reconstructor;
//! use std::fs::File;
//! use std::io::{BufRead, BufReader};
//!
//! let file = File::open("trace.log")?;
//! let mut reconstructor = Reconstructor::new();
//! for line in BufReader::new(file).lines() {
//!     let line = line?;
//!     if let Some(event) = reconstructor.push(&line) {
//!         println!("{event}");
//!     }
//! }
//! # Ok(()) }
//! ```
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
//!     println!("{}", line.expect("read trace"));
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
//!     println!("{}", line?);
//! }
//! lines.wait().await?;
//! # Ok(()) }
//! ```

mod action;
mod event;
mod exit;
mod reconstruct;
pub mod stdprocess;
pub mod syscalls;

/// Tokio integration, only compiled with the `tokio` feature.
#[cfg(feature = "tokio")]
pub mod tokio;

pub use action::Action;
pub use event::Event;
pub use exit::Exit;
pub use reconstruct::Reconstructor;
pub use stdprocess::{
    StraceOptions, stream, stream_with, stream_with_options, under_strace, under_strace_with,
    under_strace_with_options,
};
pub use syscalls::ParseError;
#[cfg(feature = "tokio")]
pub use tokio::{
    stream as tokio_stream, stream_with as tokio_stream_with,
    stream_with_options as tokio_stream_with_options, under_strace as tokio_under_strace,
    under_strace_with as tokio_under_strace_with,
    under_strace_with_options as tokio_under_strace_with_options,
};
