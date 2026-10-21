// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`ftruncate(2)`] system call.
//!
//! [`ftruncate(2)`]: https://man7.org/linux/man-pages/man2/ftruncate.2.html

use super::fd::Fd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `ftruncate(2)` system call.
///
/// ```text
/// int ftruncate(int fd, off_t length);
/// ```
///
/// `strace` prints the length as an unsigned value, so a negative length
/// shows up as a large number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ftruncate {
    /// File descriptor of the file to resize, with the path `strace` resolved
    /// for it when run with `-y`.
    pub fd: Fd,
    /// The new length of the file, in bytes.
    pub length: u64,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Ftruncate {
    /// Builds an `ftruncate(2)` record from its arguments and raw return
    /// value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, length: u64, raw_result: i64) -> Self {
        Self {
            fd: Fd::new(fd),
            length,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Ftruncate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ftruncate({}, {}) = {}",
            self.fd, self.length, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `ftruncate(fd, length) = 0` or `ftruncate(fd, length) = -1 EINVAL`.
impl FromStr for Ftruncate {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("ftruncate call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("ftruncate(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 2 {
            return Err(ParseError::new("ftruncate arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[0]))?;
        let length = parts[1]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("length", parts[1]))?;
        Ok(Self {
            fd,
            length,
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
        let call = Ftruncate::from_raw(3, 1024, 0);
        assert_eq!(call.to_string(), "ftruncate(3, 1024) = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Ftruncate::from_raw(3, 1024, -22);
        assert_eq!(call.to_string(), "ftruncate(3, 1024) = -1 EINVAL");
        assert!(!call.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let call = Ftruncate::from_str("ftruncate(3, 1024) = 0").unwrap();
        assert_eq!(call, Ftruncate::from_raw(3, 1024, 0));
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.length, 1024);
    }

    #[test]
    fn parse_failure() {
        let call = Ftruncate::from_str("ftruncate(3, 1024) = -1 EINVAL").unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.length, 1024);
        assert_eq!(call.result, OkResult::Errno(Errno::EINVAL));
    }

    #[test]
    fn parse_annotated_fd() {
        let call = Ftruncate::from_str("ftruncate(3</tmp/file>, 0) = 0").unwrap();
        assert_eq!(call.fd.fd(), 3);
        assert_eq!(
            call.fd.path().map(|p| p.to_str().unwrap()),
            Some("/tmp/file")
        );
        assert_eq!(call.to_string(), "ftruncate(3</tmp/file>, 0) = 0");
    }

    #[test]
    fn parse_invalid() {
        assert!(Ftruncate::from_str("ftruncate(3, 1024) 0").is_err());
        assert!(Ftruncate::from_str("ftruncate(x, 1024) = 0").is_err());
        assert!(Ftruncate::from_str("ftruncate(3, x) = 0").is_err());
        assert!(Ftruncate::from_str("ftruncate(3) = 0").is_err());
        assert!(Ftruncate::from_str("ftruncate(3, 1024, 2) = 0").is_err());
        assert!(Ftruncate::from_str("ftruncate(3, 1024) = 1").is_err());
        assert!(Ftruncate::from_str("ftruncate(3, 1024) = -1").is_err());
        assert!(Ftruncate::from_str("truncate(3, 1024) = 0").is_err());
        assert!(Ftruncate::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "ftruncate(3, 1024) = 0",
            "ftruncate(3, 0) = 0",
            "ftruncate(-1, 16405315391733093453) = -1 EBADF",
            "ftruncate(3, 1024) = -1 EINVAL",
            "ftruncate(4, 1024) = -1 9999",
            "ftruncate(3</tmp/file>, 0) = 0",
        ] {
            assert_eq!(Ftruncate::from_str(line).unwrap().to_string(), line);
        }
    }
}
