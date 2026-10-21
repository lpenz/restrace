// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fdatasync(2)`] system call.
//!
//! [`fdatasync(2)`]: https://man7.org/linux/man-pages/man2/fdatasync.2.html

use super::fd::Fd;
use super::parse::ParseError;
use super::result::OkResult;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `fdatasync(2)` system call.
///
/// ```text
/// int fdatasync(int fd);
/// ```
///
/// Like [`fsync(2)`](super::fsync::Fsync), but it only flushes the data
/// needed to retrieve the file, and not the metadata that is not required for
/// that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fdatasync {
    /// The file descriptor to synchronize, with the path `strace` resolved
    /// for it when run with `-y`.
    pub fd: Fd,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Fdatasync {
    /// Builds an `fdatasync(2)` record from its arguments and raw return
    /// value.
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

impl fmt::Display for Fdatasync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fdatasync({}) = {}", self.fd, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fdatasync(fd) = 0` or `fdatasync(fd) = -1 EBADF`.
impl FromStr for Fdatasync {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fdatasync call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let fd = args
            .strip_prefix("fdatasync(")
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
        let fdatasync = Fdatasync::from_raw(3, 0);
        assert_eq!(fdatasync.to_string(), "fdatasync(3) = 0");
        assert!(fdatasync.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let fdatasync = Fdatasync::from_raw(3, -9);
        assert_eq!(fdatasync.to_string(), "fdatasync(3) = -1 EBADF");
        assert!(!fdatasync.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let fdatasync = Fdatasync::from_str("fdatasync(3) = 0").unwrap();
        assert_eq!(fdatasync, Fdatasync::from_raw(3, 0));
        assert_eq!(fdatasync.fd, Fd::new(3));
    }

    #[test]
    fn parse_failure() {
        let fdatasync = Fdatasync::from_str("fdatasync(3) = -1 EBADF").unwrap();
        assert_eq!(fdatasync.fd, Fd::new(3));
        assert_eq!(fdatasync.result, OkResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_annotated_fd() {
        let fdatasync = Fdatasync::from_str("fdatasync(3</tmp/file>) = 0").unwrap();
        assert_eq!(fdatasync.fd.fd(), 3);
        assert_eq!(
            fdatasync.fd.path().map(|p| p.to_str().unwrap()),
            Some("/tmp/file")
        );
        assert_eq!(fdatasync.to_string(), "fdatasync(3</tmp/file>) = 0");
    }

    #[test]
    fn parse_invalid() {
        assert!(Fdatasync::from_str("fdatasync(3) 0").is_err());
        assert!(Fdatasync::from_str("fdatasync(x) = 0").is_err());
        assert!(Fdatasync::from_str("fdatasync(3) = 1").is_err());
        assert!(Fdatasync::from_str("fdatasync(3) = -1").is_err());
        assert!(Fdatasync::from_str("fdatasync(3, 4) = 0").is_err());
        assert!(Fdatasync::from_str("fsync(3) = 0").is_err());
        assert!(Fdatasync::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fdatasync(0) = 0",
            "fdatasync(3) = -1 EBADF",
            "fdatasync(4) = -1 EINVAL",
            "fdatasync(42) = -1 9999",
            "fdatasync(3</tmp/file>) = 0",
        ] {
            assert_eq!(Fdatasync::from_str(line).unwrap().to_string(), line);
        }
    }
}
