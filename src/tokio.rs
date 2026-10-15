// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Wrapping of `tokio::process::Command` values under `strace`.
//!
//! This module is only compiled when the `tokio` feature is enabled.
//!
//! The module name mirrors the `tokio` crate and the `tokio` feature on
//! purpose: it is the async counterpart of [`crate::stdprocess`] and is
//! always used as `restrace::tokio`, so it does not shadow the crate for
//! callers. Inside the crate the `tokio` crate itself is referred to through
//! the `::tokio` path.

use crate::Event;
use crate::reconstruct::Reconstructor;
use crate::stdprocess::TRACE_FD;
use ::tokio::fs::File as TokioFile;
use ::tokio::io::{AsyncBufReadExt, BufReader, Lines};
use ::tokio::process::{Child, Command};
use ::tokio_stream::Stream;
use std::fs::File;
use std::os::fd::FromRawFd;
use std::pin::Pin;
use std::process::ExitStatus;
use std::task::{Context, Poll};

/// Turns `cmd` into a command that runs the same program with the same
/// arguments, but under `strace`.
///
/// The returned command executes `strace --absolute-timestamps=format:unix,precision:us
/// --follow-forks -a 0 <program> <args...>`.  The working directory and
/// environment of `cmd` are copied over to the wrapped command.
///
/// Note that [`crate::Event`] only parses lines that carry a pid.
/// `strace` prints a pid on every line, including those of the top-level
/// process, only when the trace is redirected with `-o`; see [`stream`] for a
/// ready-made command that does so.
#[must_use]
pub fn under_strace(cmd: Command) -> Command {
    crate::stdprocess::under_strace(cmd.into_std()).into()
}

/// A stream of [`Event`]s read from a command running under `strace`.
///
/// The trace is read from a dedicated pipe (see [`TRACE_FD`]), keeping the
/// standard output and standard error of the traced process untouched. Calls
/// that `strace` splits into an `<unfinished ...>` line and a `<... resumed>`
/// line are joined into a single [`Event`]; lines that are neither a system
/// call nor a process exit, such as the `--- SIGCHLD ... ---` messages, are
/// skipped, so that the stream only yields the parsed trace. Because the
/// trace is redirected with `-o` and `--follow-forks`, every line carries a
/// bare pid prefix (as parsed by [`Event`]).
///
/// A reconstructed event is yielded when its resumed line is reached, so
/// events are not necessarily ordered by timestamp.
pub struct EventStream {
    child: Child,
    lines: Lines<BufReader<TokioFile>>,
    reconstructor: Reconstructor,
}

impl EventStream {
    /// Waits for the traced process to exit, reaping it.
    ///
    /// Call this after the stream ends, or when discarding it, so that the
    /// `strace` process is not left as a zombie.
    pub async fn wait(&mut self) -> ::std::io::Result<ExitStatus> {
        self.child.wait().await
    }
}

impl Stream for EventStream {
    type Item = Event;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            match Pin::new(&mut this.lines).poll_next_line(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(Some(text))) => {
                    if let Some(line) = this.reconstructor.push(&text) {
                        return Poll::Ready(Some(line));
                    }
                }
                Poll::Ready(Ok(None) | Err(_)) => return Poll::Ready(None),
            }
        }
    }
}

/// Spawns `cmd` under `strace` and returns a stream of the traced [`Event`]s.
///
/// The command is wrapped with [`under_strace`], with an added `-o /dev/fd/3`
/// that makes `strace` write its trace to a dedicated pipe instead of
/// stderr. The trace is parsed into [`Event`]s, while the standard input,
/// output and error of `cmd` are inherited, so the traced process sees and
/// prints its own streams as usual. See [`EventStream`] for how non-syscall
/// lines of output are handled.
///
/// # Errors
///
/// Returns an error if the trace pipe cannot be created or `strace` cannot be
/// spawned.
pub fn stream(cmd: Command) -> ::std::io::Result<EventStream> {
    let mut fds = [0 as libc::c_int; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(::std::io::Error::last_os_error());
    }
    let reader_fd = fds[0];
    let writer_fd = fds[1];
    // Keep both ends private to this process: the only descriptor that
    // survives the exec is the dup2'd TRACE_FD copy.
    for fd in [reader_fd, writer_fd] {
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags >= 0 {
                libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
            }
        }
    }
    let trace_path = format!("/dev/fd/{TRACE_FD}");
    let mut wrapped: Command =
        crate::stdprocess::under_strace_with(cmd.into_std(), ["-o", trace_path.as_str()]).into();
    unsafe {
        wrapped.pre_exec(move || {
            // Give strace its own descriptor for the trace output. If it was
            // already TRACE_FD, dup2 is a no-op but keeps FD_CLOEXEC, so
            // clear it to make sure the descriptor survives the exec.
            if libc::dup2(writer_fd, TRACE_FD) < 0 {
                return Err(::std::io::Error::last_os_error());
            }
            if libc::fcntl(TRACE_FD, libc::F_SETFD, 0) < 0 {
                return Err(::std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let reader_file = unsafe { File::from_raw_fd(reader_fd) };
    let child = match wrapped.spawn() {
        Ok(child) => child,
        Err(e) => {
            unsafe {
                libc::close(writer_fd);
            }
            return Err(e);
        }
    };
    // Close our copy of the writer so that the reader reaches EOF when
    // strace exits.
    unsafe {
        libc::close(writer_fd);
    }
    let trace = TokioFile::from_std(reader_file);
    Ok(EventStream {
        child,
        lines: BufReader::new(trace).lines(),
        reconstructor: Reconstructor::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn program_and_args() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let wrapped = under_strace(cmd).into_std();
        assert_eq!(wrapped.get_program(), OsStr::new("strace"));
        assert_eq!(
            wrapped.get_args().collect::<Vec<_>>(),
            vec![
                OsStr::new("--absolute-timestamps=format:unix,precision:us"),
                OsStr::new("--follow-forks"),
                OsStr::new("-a"),
                OsStr::new("0"),
                OsStr::new("echo"),
                OsStr::new("hello"),
            ]
        );
    }

    #[test]
    fn current_dir_and_env_copied() {
        let mut cmd = Command::new("pwd");
        cmd.current_dir("/tmp");
        cmd.env("FOO", "bar");
        let wrapped = under_strace(cmd).into_std();
        assert_eq!(
            wrapped.get_current_dir(),
            Some(std::path::Path::new("/tmp"))
        );
        let envs = wrapped.get_envs().collect::<Vec<_>>();
        assert_eq!(envs, vec![(OsStr::new("FOO"), Some(OsStr::new("bar")))]);
    }

    /// Returns whether `strace` can trace forks in this environment.
    fn strace_can_trace_forks() -> bool {
        let out = ::std::process::Command::new("strace")
            .args([
                "--absolute-timestamps=format:unix,precision:us",
                "--follow-forks",
                "sh",
                "-c",
                "sleep 0.02 & wait",
            ])
            .output();
        match out {
            Ok(out) => String::from_utf8_lossy(&out.stderr).contains("[pid "),
            Err(_) => false,
        }
    }

    #[::tokio::test(crate = "::tokio")]
    async fn stream_yields_lines() {
        use ::tokio_stream::StreamExt;
        if !strace_can_trace_forks() {
            return;
        }
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 0.02 & wait"]);
        let mut lines = stream(cmd).expect("spawn strace");
        let mut collected = Vec::new();
        while let Some(line) = lines.next().await {
            collected.push(line);
        }
        lines.wait().await.expect("wait for strace");
        assert!(!collected.is_empty());
        assert!(collected.iter().map(|line| line.pid).all(|pid| pid > 0));
    }
}
