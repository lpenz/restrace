// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fsync(2)`] system call.
//!
//! [`fsync(2)`]: https://man7.org/linux/man-pages/man2/fsync.2.html

use super::fd::Fd;
use super::parse::ParseError;
use super::result::OkResult;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `fsync(2)` system call.
///
/// ```text
/// int fsync(int fd);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fsync {
    /// The file descriptor to synchronize, with the path `strace` resolved
    /// for it when run with `-y`.
    pub fd: Fd,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Fsync {
    /// Builds an `fsync(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, raw_result: i64) -> Self {
        Self {
            fd: Fd::new(fd),
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fsync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fsync({}) = {}", self.fd, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fsync(fd) = 0` or `fsync(fd) = -1 EBADF`.
impl FromStr for Fsync {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fsync call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let fd = args
            .strip_prefix("fsync(")
            .ok_or_else(syntax)?
            .trim()
            .parse::<Fd>()
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
        let fsync = Fsync::from_raw(3, 0);
        assert_eq!(fsync.to_string(), "fsync(3) = 0");
        assert!(fsync.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let fsync = Fsync::from_raw(3, -9);
        assert_eq!(fsync.to_string(), "fsync(3) = -1 EBADF");
        assert!(!fsync.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let fsync = Fsync::from_str("fsync(3) = 0").unwrap();
        assert_eq!(fsync, Fsync::from_raw(3, 0));
        assert_eq!(fsync.fd, Fd::new(3));
    }

    #[test]
    fn parse_failure() {
        let fsync = Fsync::from_str("fsync(3) = -1 EBADF").unwrap();
        assert_eq!(fsync.fd, Fd::new(3));
        assert_eq!(fsync.result, OkResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_annotated_fd() {
        let fsync = Fsync::from_str("fsync(3</tmp/file>) = 0").unwrap();
        assert_eq!(fsync.fd.fd(), 3);
        assert_eq!(
            fsync.fd.path().map(|p| p.to_str().unwrap()),
            Some("/tmp/file")
        );
        assert_eq!(fsync.to_string(), "fsync(3</tmp/file>) = 0");
    }

    #[test]
    fn parse_invalid() {
        assert!(Fsync::from_str("fsync(3) 0").is_err());
        assert!(Fsync::from_str("fsync(x) = 0").is_err());
        assert!(Fsync::from_str("fsync(3) = 1").is_err());
        assert!(Fsync::from_str("fsync(3) = -1").is_err());
        assert!(Fsync::from_str("fsync(3, 4) = 0").is_err());
        assert!(Fsync::from_str("close(3) = 0").is_err());
        assert!(Fsync::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fsync(0) = 0",
            "fsync(3) = -1 EBADF",
            "fsync(4) = -1 EINVAL",
            "fsync(42) = -1 9999",
            "fsync(3</tmp/file>) = 0",
        ] {
            assert_eq!(Fsync::from_str(line).unwrap().to_string(), line);
        }
    }
}
