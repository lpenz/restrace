// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Wrapping of [`std::process::Command`] values under `strace`.

use std::ffi::OsStr;
use std::process::Command;

/// Flags passed to `strace` before the traced program.
const STRACE_FLAGS: [&str; 2] = [
    "--absolute-timestamps=format:unix,precision:us",
    "--follow-forks",
];

/// Turns `cmd` into a command that runs the same program with the same
/// arguments, but under `strace`.
///
/// The returned command executes `strace --absolute-timestamps=format:unix,precision:us
/// --follow-forks <program> <args...>`.  The working directory and environment
/// of `cmd` are copied over to the wrapped command.
///
/// Note that [`crate::line::Line`] only parses lines that carry a pid.
/// `strace` prints a pid on every line, including those of the top-level
/// process, only when the trace is redirected with `-o`; use
/// [`under_strace_with`] to add it.
#[must_use]
pub fn under_strace(cmd: Command) -> Command {
    under_strace_with(cmd, [] as [&OsStr; 0])
}

/// Like [`under_strace`], but inserts `extra` `strace` arguments between the
/// base flags and the traced program.
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
                OsStr::new("echo"),
                OsStr::new("hello"),
            ]
        );
    }

    #[test]
    fn no_args() {
        let wrapped = under_strace(Command::new("true"));
        assert_eq!(wrapped.get_args().collect::<Vec<_>>().len(), 3);
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
}
