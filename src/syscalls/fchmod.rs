// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fchmod(2)`] system call.
//!
//! [`fchmod(2)`]: https://man7.org/linux/man-pages/man2/fchmod.2.html

use super::chmodmode::ChmodMode;
use super::fd::Fd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `fchmod(2)` system call.
///
/// ```text
/// int fchmod(int fd, mode_t mode);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fchmod {
    /// File descriptor of the file to change permissions of.
    pub fd: Fd,
    /// File mode bits.
    pub mode: ChmodMode,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Fchmod {
    /// Builds a `fchmod(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, mode: ChmodMode, raw_result: i64) -> Self {
        Self {
            fd: Fd::new(fd),
            mode,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fchmod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fchmod({}, {}) = {}", self.fd, self.mode, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fchmod(3, 0600) = 0` or `fchmod(3, 0600) = -1 EBADF`.
impl FromStr for Fchmod {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fchmod call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("fchmod(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 2 {
            return Err(ParseError::new("fchmod arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[0]))?;
        let mode = ChmodMode::from_str(parts[1].trim())?;
        Ok(Self {
            fd,
            mode,
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
        let call = Fchmod::from_raw(3, ChmodMode(0o600), 0);
        assert_eq!(call.to_string(), "fchmod(3, 0600) = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Fchmod::from_raw(3, ChmodMode(0o600), -9);
        assert_eq!(call.to_string(), "fchmod(3, 0600) = -1 EBADF");
        assert!(!call.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let call = Fchmod::from_str("fchmod(3, 0600) = 0").unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.mode, ChmodMode(0o600));
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), "fchmod(3, 0600) = 0");
    }

    #[test]
    fn parse_failure() {
        let call = Fchmod::from_str("fchmod(3, 0644) = -1 EBADF").unwrap();
        assert_eq!(call.result, OkResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_invalid() {
        assert!(Fchmod::from_str("fchmod(3, 0644) 0").is_err());
        assert!(Fchmod::from_str("fchmod(3) = 0").is_err());
        assert!(Fchmod::from_str("fchmod(3, 0644, 1) = 0").is_err());
        assert!(Fchmod::from_str("chmod(3, 0644) = 0").is_err());
        assert!(Fchmod::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fchmod(3, 0600) = 0",
            "fchmod(3, 0644) = 0",
            "fchmod(3, 0755) = -1 EBADF",
        ] {
            assert_eq!(Fchmod::from_str(line).unwrap().to_string(), line);
        }
    }
}
