// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fchmodat(2)`] system call.
//!
//! [`fchmodat(2)`]: https://man7.org/linux/man-pages/man2/fchmodat.2.html

use super::chmodmode::ChmodMode;
use super::dirfd::DirFd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `fchmodat(2)` system call.
///
/// ```text
/// int fchmodat(int dirfd, const char *pathname, mode_t mode, int flags);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fchmodat {
    /// Directory against which the pathname is resolved, or
    /// [`DirFd::AT_FDCWD`].
    pub dirfd: DirFd,
    /// Pathname of the file to change permissions of, without the terminating NUL byte.
    pub pathname: CString,
    /// File mode bits.
    pub mode: ChmodMode,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Fchmodat {
    /// Builds a `fchmodat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(
        dirfd: DirFd,
        pathname: P,
        mode: ChmodMode,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            mode,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fchmodat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fchmodat({}, {:?}, {}) = {}",
            self.dirfd,
            self.pathname.to_string_lossy(),
            self.mode,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fchmodat(AT_FDCWD, "/path", 0644) = 0` or `... = -1 ENOENT`.
impl FromStr for Fchmodat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fchmodat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("fchmodat(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 3 {
            return Err(ParseError::new("fchmodat arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        let mode = ChmodMode::from_str(parts[2].trim())?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
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
        let call = Fchmodat::from_raw(DirFd::AT_FDCWD, "/tmp/test_chmod2", ChmodMode(0o755), 0);
        assert_eq!(
            call.to_string(),
            "fchmodat(AT_FDCWD, \"/tmp/test_chmod2\", 0755) = 0"
        );
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Fchmodat::from_raw(DirFd::AT_FDCWD, "/nonexistent", ChmodMode(0o644), -2);
        assert_eq!(
            call.to_string(),
            "fchmodat(AT_FDCWD, \"/nonexistent\", 0644) = -1 ENOENT"
        );
        assert!(!call.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let call =
            Fchmodat::from_str("fchmodat(AT_FDCWD, \"/tmp/test_chmod2\", 0755) = 0").unwrap();
        assert_eq!(call.dirfd, DirFd::AT_FDCWD);
        assert_eq!(call.pathname.to_str().unwrap(), "/tmp/test_chmod2");
        assert_eq!(call.mode, ChmodMode(0o755));
        assert_eq!(call.result, OkResult::Ok);
    }

    #[test]
    fn parse_failure() {
        let call = Fchmodat::from_str("fchmodat(AT_FDCWD, \"/x\", 0644) = -1 ENOENT").unwrap();
        assert_eq!(call.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Fchmodat::from_str("fchmodat(AT_FDCWD, \"/x\", 0644) 0").is_err());
        assert!(Fchmodat::from_str("fchmodat(AT_FDCWD, \"/x\") = 0").is_err());
        assert!(Fchmodat::from_str("fchmodat(AT_FDCWD, \"/x\", 0644, 0) = 0").is_err());
        assert!(Fchmodat::from_str("chmod(\"/x\", 0644) = 0").is_err());
        assert!(Fchmodat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fchmodat(AT_FDCWD, \"/tmp/test_chmod2\", 0755) = 0",
            "fchmodat(AT_FDCWD, \"/tmp/f\", 0600) = -1 ENOENT",
        ] {
            assert_eq!(Fchmodat::from_str(line).unwrap().to_string(), line);
        }
    }
}
