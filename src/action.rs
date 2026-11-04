// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`Action`] enum: a system call record or a process exit.

use crate::exit::Exit;
use crate::syscalls::{Generic, ParseError};
use std::fmt;
use std::str::FromStr;

/// What a traced process did: a system call, carried as [`Generic`], or the
/// [`Exit`] that terminated the process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// The exit of the traced process.
    Exit(Exit),
    /// A system call.
    Generic(Generic),
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exit(exit) => exit.fmt(f),
            Self::Generic(syscall) => syscall.fmt(f),
        }
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it, dispatching to the
/// matching [`Exit`] type or to [`Generic`].
impl FromStr for Action {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        if let Ok(exit) = Exit::from_str(s) {
            return Ok(Self::Exit(exit));
        }
        Generic::from_str(s).map(Self::Generic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_variants() {
        let is_exit: fn(&Action) -> bool = |s| matches!(s, Action::Exit(_));
        let is_generic: fn(&Action) -> bool = |s| matches!(s, Action::Generic(_));
        for (line, variant) in [
            ("+++ exited with 0 +++", is_exit),
            ("+++ killed by SIGKILL +++", is_exit),
            ("spill(0x1, 0x2) = 0", is_generic),
            ("open(\"/tmp\", O_RDONLY) = 3", is_generic),
            ("execve(\"/bin/echo\", [\"echo\"], []) = 0", is_generic),
            (
                "fcntl(0</dev/null>, F_DUPFD, 10) = 10</dev/null>",
                is_generic,
            ),
        ] {
            let syscall = Action::from_str(line).unwrap();
            assert!(variant(&syscall), "line {line:?} parsed wrongly");
        }
    }

    #[test]
    fn parse_invalid() {
        assert!(Action::from_str("").is_err());
        assert!(Action::from_str("no syscall here").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "chdir(\"/tmp\") = 0",
            "close(3) = -1 EBADF",
            "open(\"/tmp/file\", O_WRONLY|O_CREAT|O_TRUNC, 0644) = 3",
            "execve(\"/bin/echo\", [\"echo\", \"hello\"], [\"PATH=/bin\"]) = 0",
            "clone3({flags=CLONE_VM, exit_signal=SIGCHLD}, 88) = -1 EPERM",
            "spill(0x1000, -7, \"a, b\") = 0",
            "fcntl(0</dev/null>, F_DUPFD, 10) = 10</dev/null>",
            "fcntl(0</dev/null>, F_DUPFD, 10) = 10</tmp/gone>(deleted)",
            "+++ exited with 3 +++",
            "+++ killed by SIGSEGV (core dumped) +++",
        ] {
            let syscall = Action::from_str(line).unwrap();
            assert_eq!(syscall.to_string(), line);
        }
    }
}
