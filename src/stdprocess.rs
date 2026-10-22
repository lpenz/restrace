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
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufRead, BufReader, Lines};
use std::os::fd::FromRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus};

/// Flags passed to `strace` before the traced program.
///
/// `-a 0` disables the column alignment of return values, so a call is always
/// rendered as `name(...) = result` with a single space before the `=`.
/// `-v` stops `strace` from abbreviating structures and arrays, so that calls
/// like `stat(2)` and `getdents(2)` carry their decoded fields.
/// `-s 1048576` raises the default 32-byte limit on decoded strings, so that
/// buffers such as `readlink(2)` results are not truncated with a `...`.
/// `-y` resolves file descriptors and current working directory to their paths
/// and appends them as `<...>` annotations to the trace lines.
const STRACE_FLAGS: [&str; 8] = [
    "--absolute-timestamps=format:unix,precision:us",
    "--follow-forks",
    "-a",
    "0",
    "-v",
    "-s",
    "1048576",
    "-y",
];

/// Options controlling how `strace` is invoked.
///
/// The default options reproduce the current behaviour of [`under_strace`]
/// and [`stream`]. Use a builder to override individual settings; for the
/// new `*_with_options` entry points the options are applied as-is, while
/// the existing entry points keep their historical defaults.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StraceOptions {
    /// When true, pass `-v` to `strace` to avoid abbreviating structures.
    pub verbose: bool,
    /// The `-s` limit for string decoding, in bytes.
    pub strings: usize,
    /// An optional `-e trace=` filter.
    pub trace_filter: Option<OsString>,
    /// When true, pass `--follow-forks`.
    pub follow_forks: bool,
    /// The format passed to `--absolute-timestamps`.
    pub timestamps: OsString,
    /// When true, pass `-y` to resolve file descriptors.
    pub decode_fds: bool,
    /// The `-a` column value.
    pub align: OsString,
}

impl Default for StraceOptions {
    fn default() -> Self {
        Self {
            verbose: true,
            strings: 1048576,
            trace_filter: None,
            follow_forks: true,
            timestamps: OsString::from("format:unix,precision:us"),
            decode_fds: true,
            align: OsString::from("0"),
        }
    }
}

impl StraceOptions {
    /// Creates options that reproduce the current defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether `-v` is passed.
    #[must_use]
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Sets the `-s` string limit.
    #[must_use]
    pub fn strings(mut self, strings: usize) -> Self {
        self.strings = strings;
        self
    }

    /// Sets the `-e trace=` filter.
    #[must_use]
    pub fn trace_filter<S: Into<OsString>>(mut self, filter: S) -> Self {
        self.trace_filter = Some(filter.into());
        self
    }

    /// Sets whether `--follow-forks` is passed.
    #[must_use]
    pub fn follow_forks(mut self, follow_forks: bool) -> Self {
        self.follow_forks = follow_forks;
        self
    }

    /// Sets the `--absolute-timestamps=` format.
    #[must_use]
    pub fn timestamps<S: Into<OsString>>(mut self, timestamps: S) -> Self {
        self.timestamps = timestamps.into();
        self
    }

    /// Sets whether `-y` is passed.
    #[must_use]
    pub fn decode_fds(mut self, decode_fds: bool) -> Self {
        self.decode_fds = decode_fds;
        self
    }

    /// Sets the `-a` column value.
    #[must_use]
    pub fn align<S: Into<OsString>>(mut self, align: S) -> Self {
        self.align = align.into();
        self
    }
}

/// Builds the `strace` argument vector for the given options.
pub(crate) fn strace_args_for_options_internal(opts: &StraceOptions) -> Vec<OsString> {
    let mut args = Vec::new();
    let mut ts = OsString::from("--absolute-timestamps=");
    ts.push(&opts.timestamps);
    args.push(ts);
    if opts.follow_forks {
        args.push(OsString::from("--follow-forks"));
    }
    args.push(OsString::from("-a"));
    args.push(opts.align.clone());
    if opts.verbose {
        args.push(OsString::from("-v"));
    }
    args.push(OsString::from("-s"));
    args.push(opts.strings.to_string().into());
    if opts.decode_fds {
        args.push(OsString::from("-y"));
    }
    if let Some(filter) = &opts.trace_filter {
        let mut combined = OsString::from("trace=");
        combined.push(filter);
        args.push(OsString::from("-e"));
        args.push(combined);
    }
    args
}

/// Builds the `strace` argument vector for the given options.
fn strace_args_for_options(opts: &StraceOptions) -> Vec<OsString> {
    strace_args_for_options_internal(opts)
}

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
/// --follow-forks -a 0 -v -s 1048576 -y <program> <args...>`.  The working
/// directory and environment of `cmd` are copied over to the wrapped command.
///
/// Note that [`crate::Event`] only parses lines that carry a pid.
/// `strace` prints a pid on every line, including those of the top-level
/// process, only when the trace is redirected with `-o`; use
/// [`under_strace_with`] to add it.
#[must_use]
pub fn under_strace(cmd: Command) -> Command {
    under_strace_with_options(cmd, StraceOptions::default())
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

/// Like [`under_strace`], but uses the provided [`StraceOptions`].
#[must_use]
pub fn under_strace_with_options(cmd: Command, options: StraceOptions) -> Command {
    let mut wrapped = Command::new("strace");
    for arg in strace_args_for_options(&options) {
        wrapped.arg(arg);
    }
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
    stream_with_options(cmd, StraceOptions::default(), [] as [&OsStr; 0])
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
    stream_with_options(cmd, StraceOptions::default(), extra)
}

/// Like [`stream`], but uses the provided [`StraceOptions`] and inserts
/// `extra` `strace` arguments after those options.
pub fn stream_with_options<I, S>(
    cmd: Command,
    options: StraceOptions,
    extra: I,
) -> ::std::io::Result<EventIter>
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
    let mut args: Vec<OsString> = extra
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    args.push("-o".into());
    args.push(trace_path.into());
    let mut wrapped = Command::new("strace");
    for arg in strace_args_for_options(&options) {
        wrapped.arg(arg);
    }
    wrapped.args(args);
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
                OsStr::new("-v"),
                OsStr::new("-s"),
                OsStr::new("1048576"),
                OsStr::new("-y"),
                OsStr::new("echo"),
                OsStr::new("hello"),
            ]
        );
    }

    #[test]
    fn no_args() {
        let wrapped = under_strace(Command::new("true"));
        assert_eq!(wrapped.get_args().collect::<Vec<_>>().len(), 9);
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
                OsStr::new("-v"),
                OsStr::new("-s"),
                OsStr::new("1048576"),
                OsStr::new("-y"),
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
