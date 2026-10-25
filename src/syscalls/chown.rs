// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`chown(2)`] system call.
//!
//! [`chown(2)`]: https://man7.org/linux/man-pages/man2/chown.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use super::uid::Uid;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `chown(2)` system call.
///
/// ```text
/// int chown(const char *pathname, uid_t owner, gid_t group);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chown {
    /// Pathname of the file to change ownership of, without the terminating
    /// NUL byte.
    pub pathname: CString,
    /// User ID to set, or [`Uid::NO_CHANGE`] to leave it unchanged.
    pub owner: Uid,
    /// Group ID to set, or [`Uid::NO_CHANGE`] to leave it unchanged.
    pub group: Uid,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Chown {
    /// Builds a `chown(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(pathname: P, owner: Uid, group: Uid, raw_result: i64) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            owner,
            group,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Chown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "chown({:?}, {}, {}) = {}",
            self.pathname.to_string_lossy(),
            self.owner,
            self.group,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `chown("path", 1000, 1000) = 0` or `chown("path", -1, -1) = -1 EPERM`.
impl FromStr for Chown {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("chown call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("chown(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 3 {
            return Err(ParseError::new("chown arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        let owner = Uid::from_str(parts[1].trim())?;
        let group = Uid::from_str(parts[2].trim())?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
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
        let call = Chown::from_raw("/tmp/f1", Uid(1000), Uid(1000), 0);
        assert_eq!(call.to_string(), "chown(\"/tmp/f1\", 1000, 1000) = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Chown::from_raw("/tmp/f1", Uid::NO_CHANGE, Uid::NO_CHANGE, -1);
        assert_eq!(call.to_string(), "chown(\"/tmp/f1\", -1, -1) = -1 EPERM");
        assert!(!call.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let call = Chown::from_str("chown(\"/tmp/f1\", 1000, 1000) = 0").unwrap();
        assert_eq!(call.pathname.to_str().unwrap(), "/tmp/f1");
        assert_eq!(call.owner, Uid(1000));
        assert_eq!(call.group, Uid(1000));
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), "chown(\"/tmp/f1\", 1000, 1000) = 0");
    }

    #[test]
    fn parse_no_change_and_failure() {
        let call = Chown::from_str("chown(\"/tmp/f1\", -1, 1000) = -1 ENOENT").unwrap();
        assert_eq!(call.owner, Uid::NO_CHANGE);
        assert_eq!(call.group, Uid(1000));
        assert_eq!(call.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Chown::from_str("chown(\"/tmp/f1\", 1000, 1000) 0").is_err());
        assert!(Chown::from_str("chown(/tmp/f1, 1000, 1000) = 0").is_err());
        assert!(Chown::from_str("chown(\"/tmp/f1\", 1000) = 0").is_err());
        assert!(Chown::from_str("chown(\"/tmp/f1\", 1000, 1000, 1) = 0").is_err());
        assert!(Chown::from_str("chown(\"/tmp/f1\", x, 1000) = 0").is_err());
        assert!(Chown::from_str("fchown(3, 1000, 1000) = 0").is_err());
        assert!(Chown::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "chown(\"/tmp/f1\", 1000, 1000) = 0",
            "chown(\"/tmp/f1\", -1, -1) = -1 EPERM",
            "chown(\"/tmp/f1\", 0, 1000) = -1 ENOENT",
            "chown(\"/a, b\", 4294967294, 4294967294) = 0",
        ] {
            assert_eq!(Chown::from_str(line).unwrap().to_string(), line);
        }
    }
}
