// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`dup2(2)`] system call.
//!
//! [`dup2(2)`]: https://man7.org/linux/man-pages/man2/dup2.2.html

use super::fd::FdResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `dup2(2)` system call.
///
/// ```text
/// int dup2(int oldfd, int newfd);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dup2 {
    /// The file descriptor to duplicate.
    pub oldfd: i32,
    /// The file descriptor to make a copy of `oldfd`.
    pub newfd: i32,
    /// Return value: the new file descriptor, or the errno that caused the
    /// failure.
    pub result: FdResult,
}

impl Dup2 {
    /// Builds a `dup2(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the new
    /// file descriptor (>= 0) on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(oldfd: i32, newfd: i32, raw_result: i64) -> Self {
        Self {
            oldfd,
            newfd,
            result: FdResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Dup2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dup2({}, {}) = {}", self.oldfd, self.newfd, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `dup2(oldfd, newfd) = fd` or `dup2(oldfd, newfd) = -1 EBADF`.
impl FromStr for Dup2 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("dup2 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("dup2(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 2 {
            return Err(ParseError::new("dup2 arguments", args));
        }
        let oldfd = parts[0].trim().parse::<i32>().map_err(|_| syntax())?;
        let newfd = parts[1].trim().parse::<i32>().map_err(|_| syntax())?;
        Ok(Self {
            oldfd,
            newfd,
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
        let dup2 = Dup2::from_raw(0, 3, 3);
        assert_eq!(dup2.to_string(), "dup2(0, 3) = 3");
        assert_eq!(dup2.result.fd(), Some(3));
    }

    #[test]
    fn display_failure() {
        let dup2 = Dup2::from_raw(3, 4, -9);
        assert_eq!(dup2.to_string(), "dup2(3, 4) = -1 EBADF");
        assert_eq!(dup2.result.fd(), None);
    }

    #[test]
    fn parse_success() {
        let dup2 = Dup2::from_str("dup2(0, 3) = 3").unwrap();
        assert_eq!(dup2, Dup2::from_raw(0, 3, 3));
        assert_eq!(dup2.oldfd, 0);
        assert_eq!(dup2.newfd, 3);
    }

    #[test]
    fn parse_failure() {
        let dup2 = Dup2::from_str("dup2(1, 3) = -1 EBADF").unwrap();
        assert_eq!(dup2.oldfd, 1);
        assert_eq!(dup2.newfd, 3);
        assert_eq!(dup2.result, FdResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_invalid() {
        assert!(Dup2::from_str("dup2(0, 3) 3").is_err());
        assert!(Dup2::from_str("dup2(0) = 3").is_err());
        assert!(Dup2::from_str("dup2(0, 3, 4) = 3").is_err());
        assert!(Dup2::from_str("dup2(x, 3) = 3").is_err());
        assert!(Dup2::from_str("dup2(0, 3) = -1").is_err());
        assert!(Dup2::from_str("dup(0) = 3").is_err());
        assert!(Dup2::from_str("dup3(0, 3, 0) = 3").is_err());
        assert!(Dup2::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "dup2(0, 3) = 3",
            "dup2(1, 4) = -1 EBADF",
            "dup2(42, 43) = -1 9999",
        ] {
            assert_eq!(Dup2::from_str(line).unwrap().to_string(), line);
        }
    }
}
