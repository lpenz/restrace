// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fchown(2)`] system call.
//!
//! [`fchown(2)`]: https://man7.org/linux/man-pages/man2/fchown.2.html

use super::fd::Fd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use super::uid::Uid;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `fchown(2)` system call.
///
/// ```text
/// int fchown(int fd, uid_t owner, gid_t group);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fchown {
    /// File descriptor of the file to change ownership of.
    pub fd: Fd,
    /// User ID to set, or [`Uid::NO_CHANGE`] to leave it unchanged.
    pub owner: Uid,
    /// Group ID to set, or [`Uid::NO_CHANGE`] to leave it unchanged.
    pub group: Uid,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Fchown {
    /// Builds a `fchown(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, owner: Uid, group: Uid, raw_result: i64) -> Self {
        Self {
            fd: Fd::new(fd),
            owner,
            group,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fchown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fchown({}, {}, {}) = {}",
            self.fd, self.owner, self.group, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fchown(3, 1000, 1000) = 0` or `fchown(3, -1, -1) = -1 EBADF`.
impl FromStr for Fchown {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fchown call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("fchown(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 3 {
            return Err(ParseError::new("fchown arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[0]))?;
        let owner = Uid::from_str(parts[1].trim())?;
        let group = Uid::from_str(parts[2].trim())?;
        Ok(Self {
            fd,
            owner,
            group,
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
        let call = Fchown::from_raw(3, Uid(1000), Uid(1000), 0);
        assert_eq!(call.to_string(), "fchown(3, 1000, 1000) = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Fchown::from_raw(3, Uid::NO_CHANGE, Uid(1000), -9);
        assert_eq!(call.to_string(), "fchown(3, -1, 1000) = -1 EBADF");
        assert!(!call.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let call = Fchown::from_str("fchown(3, 1000, 1000) = 0").unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.owner, Uid(1000));
        assert_eq!(call.group, Uid(1000));
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), "fchown(3, 1000, 1000) = 0");
    }

    #[test]
    fn parse_annotated_fd() {
        let call = Fchown::from_str("fchown(3</tmp/f1>, -1, -1) = -1 EBADF").unwrap();
        assert_eq!(call.fd.fd(), 3);
        assert_eq!(
            call.fd.path().map(|path| path.to_str().unwrap()),
            Some("/tmp/f1")
        );
        assert_eq!(call.owner, Uid::NO_CHANGE);
        assert_eq!(call.group, Uid::NO_CHANGE);
        assert_eq!(call.result, OkResult::Errno(Errno::EBADF));
        assert_eq!(call.to_string(), "fchown(3</tmp/f1>, -1, -1) = -1 EBADF");
    }

    #[test]
    fn parse_invalid() {
        assert!(Fchown::from_str("fchown(3, 1000, 1000) 0").is_err());
        assert!(Fchown::from_str("fchown(3, 1000) = 0").is_err());
        assert!(Fchown::from_str("fchown(3, 1000, 1000, 1) = 0").is_err());
        assert!(Fchown::from_str("fchown(3, x, 1000) = 0").is_err());
        assert!(Fchown::from_str("chown(3, 1000, 1000) = 0").is_err());
        assert!(Fchown::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fchown(3, 1000, 1000) = 0",
            "fchown(3, -1, 1000) = -1 EBADF",
            "fchown(3</tmp/f1>, 0, 0) = -1 EPERM",
            "fchown(3, 4294967294, 4294967294) = 0",
        ] {
            assert_eq!(Fchown::from_str(line).unwrap().to_string(), line);
        }
    }
}
