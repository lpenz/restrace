// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fchdir(2)`] system call.
//!
//! [`fchdir(2)`]: https://man7.org/linux/man-pages/man2/fchdir.2.html

use super::parse::ParseError;
use super::result::OkResult;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `fchdir(2)` system call.
///
/// ```text
/// int fchdir(int fd);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fchdir {
    /// File descriptor of the directory to change to.
    pub fd: i32,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Fchdir {
    /// Builds an `fchdir(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, raw_result: i64) -> Self {
        Self {
            fd,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fchdir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fchdir({}) = {}", self.fd, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fchdir(fd) = 0` or `fchdir(fd) = -1 EBADF`.
impl FromStr for Fchdir {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fchdir call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let fd = args
            .strip_prefix("fchdir(")
            .ok_or_else(syntax)?
            .trim()
            .parse::<i32>()
            .map_err(|_| syntax())?;
        Ok(Self {
            fd,
            result: OkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_success() {
        let fchdir = Fchdir::from_raw(3, 0);
        assert_eq!(fchdir.to_string(), "fchdir(3) = 0");
        assert!(fchdir.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let fchdir = Fchdir::from_raw(3, -9);
        assert_eq!(fchdir.to_string(), "fchdir(3) = -1 EBADF");
        assert!(!fchdir.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let fchdir = Fchdir::from_str("fchdir(3) = 0").unwrap();
        assert_eq!(fchdir, Fchdir::from_raw(3, 0));
        assert_eq!(fchdir.fd, 3);
    }

    #[test]
    fn parse_failure() {
        let fchdir = Fchdir::from_str("fchdir(3) = -1 ENOTDIR").unwrap();
        assert_eq!(fchdir.result, OkResult::Errno(Errno::ENOTDIR));
    }

    #[test]
    fn parse_invalid() {
        assert!(Fchdir::from_str("fchdir(3) 0").is_err());
        assert!(Fchdir::from_str("fchdir(x) = 0").is_err());
        assert!(Fchdir::from_str("fchdir(3) = 1").is_err());
        assert!(Fchdir::from_str("close(3) = 0").is_err());
        assert!(Fchdir::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fchdir(0) = 0",
            "fchdir(3) = -1 EBADF",
            "fchdir(4) = -1 ENOTDIR",
            "fchdir(42) = -1 9999",
        ] {
            assert_eq!(Fchdir::from_str(line).unwrap().to_string(), line);
        }
    }
}
