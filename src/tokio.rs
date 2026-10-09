// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Wrapping of `tokio::process::Command` values under `strace`.
//!
//! This module is only compiled when the `tokio` feature is enabled.

use crate::line::Line;
use ::tokio::io::{AsyncBufReadExt, BufReader, Lines};
use ::tokio::process::{Child, ChildStderr, Command};
use ::tokio_stream::Stream;
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::task::{Context, Poll};

/// Turns `cmd` into a command that runs the same program with the same
/// arguments, but under `strace`.
///
/// The returned command executes `strace --absolute-timestamps=format:unix,precision:us
/// --follow-forks <program> <args...>`, so that the traced calls can be read
/// with [`crate::line::Line`].  The working directory and environment of
/// `cmd` are copied over to the wrapped command.
#[must_use]
pub fn under_strace(cmd: Command) -> Command {
    crate::stdprocess::under_strace(cmd.into_std()).into()
}

/// A stream of [`Line`]s read from a command running under `strace`.
///
/// Lines of strace output that do not form a complete, single system call are
/// skipped, so that the stream only yields the parsed trace: fragments such as
/// `<unfinished ...>`, `<... resumed>` and the `+++ exited ... +++` messages.
/// The lines strace prints before the first `fork`/`clone`, which carry no
/// `[pid ...]` prefix, are skipped for the same reason.
pub struct LineStream {
    child: Child,
    lines: Lines<BufReader<ChildStderr>>,
}

impl LineStream {
    /// Waits for the traced process to exit, reaping it.
    ///
    /// Call this after the stream ends, or when discarding it, so that the
    /// `strace` process is not left as a zombie.
    pub async fn wait(&mut self) -> ::std::io::Result<ExitStatus> {
        self.child.wait().await
    }
}

impl Stream for LineStream {
    type Item = Line;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            match Pin::new(&mut this.lines).poll_next_line(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(Some(text))) => {
                    if let Ok(line) = text.parse::<Line>() {
                        return Poll::Ready(Some(line));
                    }
                }
                Poll::Ready(Ok(None) | Err(_)) => return Poll::Ready(None),
            }
        }
    }
}

/// Spawns `cmd` under `strace` and returns a stream of the traced [`Line`]s.
///
/// The command is wrapped with [`under_strace`]; its standard error is
/// captured and parsed into [`Line`]s, while its standard input and output are
/// inherited. See [`LineStream`] for how non-syscall lines of output are
/// handled.
///
/// # Errors
///
/// Returns an error if `strace` cannot be spawned.
pub fn stream(cmd: Command) -> ::std::io::Result<LineStream> {
    let mut wrapped = under_strace(cmd);
    wrapped.stderr(Stdio::piped());
    let mut child = wrapped.spawn()?;
    let stderr = child.stderr.take().expect("stderr was piped");
    Ok(LineStream {
        child,
        lines: BufReader::new(stderr).lines(),
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
        assert!(collected.iter().all(|line| line.pid > 0));
    }
}
