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
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::os::fd::FromRawFd;
use std::pin::Pin;
use std::process::ExitStatus;
use std::task::{Context, Poll};

/// Turns `cmd` into a command that runs the same program with the same
/// arguments, but under `strace`.
///
/// The returned command executes `strace --absolute-timestamps=format:unix,precision:us
/// --follow-forks -a 0 -v <program> <args...>`.  The working directory and
/// environment of `cmd` are copied over to the wrapped command.
///
/// Note that [`crate::Event`] only parses lines that carry a pid.
/// `strace` prints a pid on every line, including those of the top-level
/// process, only when the trace is redirected with `-o`; see [`stream`] for a
/// ready-made command that does so.
#[must_use]
pub fn under_strace(cmd: Command) -> Command {
    under_strace_with(cmd, [] as [&OsStr; 0])
}

/// Like [`under_strace`], but inserts `extra` `strace` arguments between the
/// base flags and the traced program.
///
/// This is the async counterpart of
/// [`stdprocess::under_strace_with`](crate::stdprocess::under_strace_with);
/// prefer [`under_strace`] unless you need the extra `strace` arguments.
#[must_use]
pub fn under_strace_with<I, S>(cmd: Command, extra: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    crate::stdprocess::under_strace_with(cmd.into_std(), extra).into()
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
    stream_with(cmd, [] as [&OsStr; 0])
}

/// Like [`stream`], but inserts `extra` `strace` arguments between the base
/// flags and the traced program.
///
/// This is the async counterpart of
/// [`stdprocess::stream_with`](crate::stdprocess::stream_with); prefer
/// [`stream`] unless you need the extra `strace` arguments.
///
/// The internal `-o /dev/fd/3` is placed after `extra`, so it takes precedence
/// over any output redirection in `extra` (such as `-o FILE`): the trace is
/// always read from the dedicated pipe.
///
/// # Errors
///
/// Returns an error if the trace pipe cannot be created or `strace` cannot be
/// spawned.
pub fn stream_with<I, S>(cmd: Command, extra: I) -> ::std::io::Result<EventStream>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
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
    // The output redirection goes last so that it takes precedence over any
    // output selection in `extra`: `strace` keeps the last `-o`, and the trace
    // must reach the pipe for the stream to yield events.
    let mut args: Vec<OsString> = extra
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    args.push("-o".into());
    args.push(trace_path.into());
    let mut wrapped: Command = crate::stdprocess::under_strace_with(cmd.into_std(), args).into();
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
                OsStr::new("-v"),
                OsStr::new("echo"),
                OsStr::new("hello"),
            ]
        );
    }

    #[test]
    fn extra_args_injected() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let wrapped = under_strace_with(cmd, ["-e", "trace=openat"]).into_std();
        assert_eq!(
            wrapped.get_args().collect::<Vec<_>>(),
            vec![
                OsStr::new("--absolute-timestamps=format:unix,precision:us"),
                OsStr::new("--follow-forks"),
                OsStr::new("-a"),
                OsStr::new("0"),
                OsStr::new("-v"),
                OsStr::new("-e"),
                OsStr::new("trace=openat"),
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

    #[::tokio::test(crate = "::tokio")]
    async fn stream_with_filters_syscalls() {
        use ::tokio_stream::StreamExt;
        if !strace_can_trace_forks() {
            return;
        }
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "true"]);
        let mut lines = stream_with(cmd, ["-e", "trace=openat"]).expect("spawn strace");
        let mut collected = Vec::new();
        while let Some(line) = lines.next().await {
            collected.push(line);
        }
        lines.wait().await.expect("wait for strace");
        assert!(!collected.is_empty());
        assert!(
            collected.iter().all(|line| matches!(
                line.action,
                crate::Action::Openat(_) | crate::Action::Exit(_)
            )),
            "unexpected syscall with -e trace=openat: {collected:?}"
        );
    }

    #[::tokio::test(crate = "::tokio")]
    async fn stream_with_caller_output_still_streams() {
        use ::tokio_stream::StreamExt;
        if !strace_can_trace_forks() {
            return;
        }
        // A caller-supplied `-o` must not steal the trace from the pipe: the
        // internal redirection is placed last and wins, so the stream still
        // yields the parsed events.
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "true"]);
        let mut lines = stream_with(cmd, ["-o", "/dev/null"]).expect("spawn strace");
        let mut collected = Vec::new();
        while let Some(line) = lines.next().await {
            collected.push(line);
        }
        lines.wait().await.expect("wait for strace");
        assert!(!collected.is_empty(), "expected the trace over the pipe");
        assert!(
            collected
                .iter()
                .any(|line| matches!(line.action, crate::Action::Exit(_)))
        );
    }
}
