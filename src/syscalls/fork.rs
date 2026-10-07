// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `fork` system call.
//!
//! [man7]: https://man7.org/linux/man-pages/man2/fork.2.html

use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `fork(2)` system call.
///
/// ```text
/// pid_t fork(void);
/// ```
///
/// Strace typically outputs `fork() = pid` or `fork() = -1 errno`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fork {
    /// Return value: child PID in the parent, 0 in the child, or -1 on error.
    pub result: ForkResult,
}

impl Fork {
    /// Builds a `fork(2)` record from its raw return value.
    #[must_use]
    pub fn from_raw(raw_result: i64) -> Self {
        Self {
            result: ForkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fork() = {}", self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it: `fork() = ...`.
impl FromStr for Fork {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fork call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let prefix = args.strip_prefix("fork(").ok_or_else(syntax)?;
        if !prefix.trim().is_empty() {
            return Err(ParseError::new("fork arguments", args));
        }
        Ok(Self {
            result: ForkResult::from_str(result)?,
        })
    }
}

/// The return value of `fork(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkResult {
    /// Fork succeeded; in the parent this is the PID of the child, in the
    /// child this is 0.
    Pid(i32),
    /// Fork failed with this errno.
    Errno(crate::syscalls::errno::Errno),
}

impl ForkResult {
    /// Converts a raw `fork(2)` return value into a [`ForkResult`].
    #[must_use]
    pub fn from_raw(raw: i64) -> Self {
        if (-4095..=-1).contains(&raw) {
            Self::Errno(crate::syscalls::errno::Errno(-raw as i32))
        } else {
            Self::Pid(raw as i32)
        }
    }
}

impl fmt::Display for ForkResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pid(pid) => write!(f, "{pid}"),
            Self::Errno(errno) => write!(f, "-1 {errno}"),
        }
    }
}

/// Parses a return value as written by strace.
impl FromStr for ForkResult {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("return value", s);
        if let Some(errno) = s.strip_prefix("-1 ") {
            return Ok(Self::Errno(
                crate::syscalls::errno::Errno::from_str(errno.trim()).map_err(|_| error())?,
            ));
        }
        if s == "-1" {
            return Err(error());
        }
        s.parse::<i32>().map(Self::Pid).map_err(|_| error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_display_parent() {
        let fork = Fork::from_raw(123);
        assert_eq!(fork.to_string(), "fork() = 123");
    }

    #[test]
    fn fork_display_child() {
        let fork = Fork::from_raw(0);
        assert_eq!(fork.to_string(), "fork() = 0");
    }

    #[test]
    fn fork_display_error() {
        let fork = Fork::from_raw(-12);
        assert_eq!(fork.to_string(), "fork() = -1 ENOMEM");
    }

    #[test]
    fn fork_parse() {
        assert_eq!(Fork::from_str("fork() = 123").unwrap(), Fork::from_raw(123));
        assert_eq!(Fork::from_str("fork() = 0").unwrap(), Fork::from_raw(0));
        assert_eq!(
            Fork::from_str("fork() = -1 ENOMEM").unwrap(),
            Fork::from_raw(-12)
        );
        assert!(Fork::from_str("fork(1) = 1").is_err());
        assert!(Fork::from_str("clone(...) = 1").is_err());
        assert!(Fork::from_str("").is_err());
    }

    #[test]
    fn fork_round_trip() {
        for line in ["fork() = 0", "fork() = 123", "fork() = -1 EAGAIN"] {
            assert_eq!(Fork::from_str(line).unwrap().to_string(), line);
        }
    }
}
