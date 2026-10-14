// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Wrapping of [`std::process::Command`] values under `strace`.
//!
//! [`under_strace`] and [`stream`] are the usual entry points;
//! [`under_strace_with`] is a lower-level helper for callers that need to
//! inject extra `strace` arguments.

use crate::Event;
use crate::reconstruct::Reconstructor;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader, Lines};
use std::os::fd::FromRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus};

/// Flags passed to `strace` before the traced program.
///
/// `-a 0` disables the column alignment of return values, so a call is always
/// rendered as `name(...) = result` with a single space before the `=`.
const STRACE_FLAGS: [&str; 4] = [
    "--absolute-timestamps=format:unix,precision:us",
    "--follow-forks",
    "-a",
    "0",
];

/// File descriptor strace writes its trace output to.
///
/// The descriptor is the write end of an anonymous pipe we create, passed to
/// `strace` as `-o /dev/fd/3`, so that the trace does not interfere with the
/// standard streams of the traced process.
pub(crate) const TRACE_FD: libc::c_int = 3;

/// Turns `cmd` into a command that runs the same program with the same
/// arguments, but under `strace`.
///
/// The returned command executes `strace --absolute-timestamps=format:unix,precision:us
/// --follow-forks -a 0 <program> <args...>`.  The working directory and
/// environment of `cmd` are copied over to the wrapped command.
///
/// Note that [`crate::Event`] only parses lines that carry a pid.
/// `strace` prints a pid on every line, including those of the top-level
/// process, only when the trace is redirected with `-o`; use
/// [`under_strace_with`] to add it.
#[must_use]
pub fn under_strace(cmd: Command) -> Command {
    under_strace_with(cmd, [] as [&OsStr; 0])
}

/// Like [`under_strace`], but inserts `extra` `strace` arguments between the
/// base flags and the traced program.
///
/// This is a lower-level helper: prefer [`under_strace`] unless you need the
/// extra `strace` arguments.
#[must_use]
pub fn under_strace_with<I, S>(cmd: Command, extra: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut wrapped = Command::new("strace");
    wrapped.args(STRACE_FLAGS);
    wrapped.args(extra);
    wrapped.arg(cmd.get_program());
    wrapped.args(cmd.get_args());
    if let Some(dir) = cmd.get_current_dir() {
        wrapped.current_dir(dir);
    }
    for (key, value) in cmd.get_envs() {
        if let Some(value) = value {
            wrapped.env(key, value);
        } else {
            wrapped.env_remove(key);
        }
    }
    wrapped
}

/// An iterator of [`Event`]s read from a command running under `strace`.
///
/// The trace is read from a dedicated pipe (see [`TRACE_FD`]), keeping the
/// standard output and standard error of the traced process untouched. Calls
/// that `strace` splits into an `<unfinished ...>` line and a `<... resumed>`
/// line are joined into a single [`Event`]; lines that are neither a system
/// call nor a process exit, such as the `--- SIGCHLD ... ---` messages, are
/// skipped, so that the iterator only yields the parsed trace. Because
/// the trace is redirected with `-o` and `--follow-forks`, every line carries
/// a bare pid prefix (as parsed by [`Event`]).
///
/// A reconstructed event is yielded when its resumed line is reached, so
/// events are not necessarily ordered by timestamp.
pub struct EventIter {
    child: Child,
    lines: Lines<BufReader<File>>,
    reconstructor: Reconstructor,
}

impl EventIter {
    /// Waits for the traced process to exit, reaping it.
    ///
    /// Call this after the iterator ends, or when discarding it, so that the
    /// `strace` process is not left as a zombie.
    pub fn wait(&mut self) -> ::std::io::Result<ExitStatus> {
        self.child.wait()
    }
}

impl Iterator for EventIter {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        loop {
            match self.lines.next() {
                Some(Ok(text)) => {
                    if let Some(line) = self.reconstructor.push(&text) {
                        return Some(line);
                    }
                }
                Some(Err(_)) | None => return None,
            }
        }
    }
}

/// Spawns `cmd` under `strace` and returns an iterator of the traced [`Event`]s.
///
/// The command is wrapped with [`under_strace`], with an added `-o /dev/fd/3`
/// that makes `strace` write its trace to a dedicated pipe instead of stderr.
/// The trace is parsed into [`Event`]s, while the standard input, output and
/// error of `cmd` are inherited, so the traced process sees and prints its own
/// streams as usual. See [`EventIter`] for how non-syscall lines of output are
/// handled.
///
/// # Errors
///
/// Returns an error if the trace pipe cannot be created or `strace` cannot be
/// spawned.
pub fn stream(cmd: Command) -> ::std::io::Result<EventIter> {
    stream_with(cmd, [] as [&OsStr; 0])
}

/// Like [`stream`], but inserts `extra` `strace` arguments between the base
/// flags and the traced program.
///
/// This is a lower-level helper: prefer [`stream`] unless you need the extra
/// `strace` arguments.
///
/// The internal `-o /dev/fd/3` is placed after `extra`, so it takes precedence
/// over any output redirection in `extra` (such as `-o FILE`): the trace is
/// always read from the dedicated pipe.
///
/// # Errors
///
/// Returns an error if the trace pipe cannot be created or `strace` cannot be
/// spawned.
pub fn stream_with<I, S>(cmd: Command, extra: I) -> ::std::io::Result<EventIter>
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
    // must reach the pipe for the iterator to yield events.
    let mut args: Vec<std::ffi::OsString> = extra
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    args.push("-o".into());
    args.push(trace_path.into());
    let mut wrapped = under_strace_with(cmd, args);
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
    Ok(EventIter {
        child,
        lines: BufReader::new(reader_file).lines(),
        reconstructor: Reconstructor::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::path::Path;

    #[test]
    fn program_and_args() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let wrapped = under_strace(cmd);
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
    fn no_args() {
        let wrapped = under_strace(Command::new("true"));
        assert_eq!(wrapped.get_args().collect::<Vec<_>>().len(), 5);
    }

    #[test]
    fn extra_args_injected() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let wrapped = under_strace_with(cmd, ["-o", "/dev/fd/3"]);
        assert_eq!(
            wrapped.get_args().collect::<Vec<_>>(),
            vec![
                OsStr::new("--absolute-timestamps=format:unix,precision:us"),
                OsStr::new("--follow-forks"),
                OsStr::new("-a"),
                OsStr::new("0"),
                OsStr::new("-o"),
                OsStr::new("/dev/fd/3"),
                OsStr::new("echo"),
                OsStr::new("hello"),
            ]
        );
    }

    #[test]
    fn current_dir_copied() {
        let mut cmd = Command::new("pwd");
        cmd.current_dir("/tmp");
        let wrapped = under_strace(cmd);
        assert_eq!(wrapped.get_current_dir(), Some(Path::new("/tmp")));
    }

    #[test]
    fn env_copied() {
        let mut cmd = Command::new("env");
        cmd.env("FOO", "bar");
        let wrapped = under_strace(cmd);
        let envs = wrapped.get_envs().collect::<Vec<_>>();
        assert_eq!(envs, vec![(OsStr::new("FOO"), Some(OsStr::new("bar")))]);
    }

    #[test]
    fn env_removed() {
        let mut cmd = Command::new("env");
        cmd.env_remove("TERM");
        let wrapped = under_strace(cmd);
        let envs = wrapped.get_envs().collect::<Vec<_>>();
        assert_eq!(envs, vec![(OsStr::new("TERM"), None)]);
    }

    /// Returns whether `strace` can trace forks in this environment.
    fn strace_can_trace_forks() -> bool {
        let out = Command::new("strace")
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

    #[test]
    fn stream_yields_lines() {
        if !strace_can_trace_forks() {
            return;
        }
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 0.02 & wait"]);
        let mut lines = stream(cmd).expect("spawn strace");
        let collected: Vec<Event> = lines.by_ref().collect();
        lines.wait().expect("wait for strace");
        assert!(!collected.is_empty());
        assert!(collected.iter().map(|line| line.pid).all(|pid| pid > 0));
    }

    #[test]
    fn stream_with_caller_output_still_streams() {
        if !strace_can_trace_forks() {
            return;
        }
        // A caller-supplied `-o` must not steal the trace from the pipe: the
        // internal redirection is placed last and wins, so the iterator still
        // yields the parsed events.
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "true"]);
        let mut lines = stream_with(cmd, ["-o", "/dev/null"]).expect("spawn strace");
        let collected: Vec<Event> = lines.by_ref().collect();
        lines.wait().expect("wait for strace");
        assert!(!collected.is_empty(), "expected the trace over the pipe");
        assert!(
            collected
                .iter()
                .any(|line| matches!(line.action, crate::Action::Exit(_)))
        );
    }
}
