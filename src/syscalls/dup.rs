// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`dup(2)`] system call.
//!
//! [`dup(2)`]: https://man7.org/linux/man-pages/man2/dup.2.html

use super::fd::{Fd, FdResult};
use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `dup(2)` system call.
///
/// ```text
/// int dup(int oldfd);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dup {
    /// The file descriptor to duplicate, with the path `strace` resolved for
    /// it when run with `-y`.
    pub oldfd: Fd,
    /// Return value: the new file descriptor, or the errno that caused the
    /// failure.
    pub result: FdResult,
}

impl Dup {
    /// Builds a `dup(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: a new file
    /// descriptor (>= 0) on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(oldfd: i32, raw_result: i64) -> Self {
        Self {
            oldfd: Fd::new(oldfd),
            result: FdResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Dup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dup({}) = {}", self.oldfd, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `dup(oldfd) = fd` or `dup(oldfd) = -1 EBADF`.
impl FromStr for Dup {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("dup call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let oldfd = args
            .strip_prefix("dup(")
            .ok_or_else(syntax)?
            .trim()
            .parse::<Fd>()
            .map_err(|_| syntax())?;
        Ok(Self {
            oldfd,
            result: FdResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_success() {
        let dup = Dup::from_raw(0, 3);
        assert_eq!(dup.to_string(), "dup(0) = 3");
        assert_eq!(dup.result.fd(), Some(3));
    }

    #[test]
    fn display_failure() {
        let dup = Dup::from_raw(3, -9);
        assert_eq!(dup.to_string(), "dup(3) = -1 EBADF");
        assert_eq!(dup.result.fd(), None);
    }

    #[test]
    fn parse_success() {
        let dup = Dup::from_str("dup(0) = 3").unwrap();
        assert_eq!(dup, Dup::from_raw(0, 3));
        assert_eq!(dup.oldfd, Fd::new(0));
    }

    #[test]
    fn parse_failure() {
        let dup = Dup::from_str("dup(3) = -1 EBADF").unwrap();
        assert_eq!(dup.oldfd, Fd::new(3));
        assert_eq!(dup.result, FdResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_invalid() {
        assert!(Dup::from_str("dup(0) 3").is_err());
        assert!(Dup::from_str("dup(x) = 3").is_err());
        assert!(Dup::from_str("dup(0) = -1").is_err());
        assert!(Dup::from_str("dup2(0, 3) = 3").is_err());
        assert!(Dup::from_str("close(0) = 0").is_err());
        assert!(Dup::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in ["dup(0) = 3", "dup(3) = -1 EBADF", "dup(42) = -1 9999"] {
            assert_eq!(Dup::from_str(line).unwrap().to_string(), line);
        }
    }
}
