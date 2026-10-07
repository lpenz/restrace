// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`close(2)`] system call.
//!
//! [`close(2)`]: https://man7.org/linux/man-pages/man2/close.2.html

use super::parse::ParseError;
use super::result::OkResult;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `close(2)` system call.
///
/// ```text
/// int close(int fd);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Close {
    /// The file descriptor to close.
    pub fd: i32,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Close {
    /// Builds a `close(2)` record from its arguments and raw return value.
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

impl fmt::Display for Close {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "close({}) = {}", self.fd, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `close(fd) = 0` or `close(fd) = -1 EBADF`.
impl FromStr for Close {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("close call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let fd = args
            .strip_prefix("close(")
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
        let close = Close::from_raw(3, 0);
        assert_eq!(close.to_string(), "close(3) = 0");
        assert!(close.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let close = Close::from_raw(3, -9);
        assert_eq!(close.to_string(), "close(3) = -1 EBADF");
        assert!(!close.result.is_ok());
    }

    #[test]
    fn parse_success() {
        assert_eq!(
            Close::from_str("close(3) = 0").unwrap(),
            Close::from_raw(3, 0)
        );
        assert_eq!(OkResult::from_str("0"), Ok(OkResult::Ok));
    }

    #[test]
    fn parse_failure() {
        let close = Close::from_str("close(3) = -1 EBADF").unwrap();
        assert_eq!(close.fd, 3);
        assert_eq!(close.result, OkResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_invalid() {
        assert!(Close::from_str("close(3) 0").is_err());
        assert!(Close::from_str("close(x) = 0").is_err());
        assert!(Close::from_str("close(3) = 1").is_err());
        assert!(Close::from_str("close(3) = -1").is_err());
        assert!(Close::from_str("open(\"/f\", O_RDONLY) = 3").is_err());
    }

    #[test]
    fn round_trip() {
        for line in ["close(0) = 0", "close(3) = -1 EBADF", "close(42) = -1 9999"] {
            assert_eq!(Close::from_str(line).unwrap().to_string(), line);
        }
    }
}
