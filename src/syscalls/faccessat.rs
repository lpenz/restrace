// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`faccessat(2)`] system call.
//!
//! [`faccessat(2)`]: https://man7.org/linux/man-pages/man2/faccessat.2.html

use super::accessmode::AccessMode;
use super::dirfd::DirFd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `faccessat(2)` system call.
///
/// ```text
/// int faccessat(int dirfd, const char *pathname, int mode);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Faccessat {
    /// Directory against which the pathname is resolved, or
    /// [`DirFd::AT_FDCWD`].
    pub dirfd: DirFd,
    /// Pathname of the file to test, without the terminating NUL byte.
    ///
    /// Interpreted relative to `dirfd` unless it is absolute.
    pub pathname: CString,
    /// Permissions to test for, the `mode` argument.
    pub mode: AccessMode,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Faccessat {
    /// Builds an `faccessat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(
        dirfd: DirFd,
        pathname: P,
        mode: AccessMode,
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

impl fmt::Display for Faccessat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "faccessat({}, {:?}, {}) = {}",
            self.dirfd,
            self.pathname.to_string_lossy(),
            self.mode,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `faccessat(AT_FDCWD, "path", F_OK) = 0` or `... = -1 EACCES`.
impl FromStr for Faccessat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("faccessat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("faccessat(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 3 {
            return Err(ParseError::new("faccessat arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        let mode = AccessMode::from_str(parts[2].trim())?;
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
        let faccessat = Faccessat::from_raw(DirFd::AT_FDCWD, "/etc/hostname", AccessMode::R_OK, 0);
        assert_eq!(
            faccessat.to_string(),
            "faccessat(AT_FDCWD, \"/etc/hostname\", R_OK) = 0"
        );
        assert!(faccessat.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let faccessat = Faccessat::from_raw(DirFd::new(4), "file", AccessMode::W_OK, -13);
        assert_eq!(
            faccessat.to_string(),
            "faccessat(4, \"file\", W_OK) = -1 EACCES"
        );
    }

    #[test]
    fn parse_success() {
        let faccessat =
            Faccessat::from_str("faccessat(AT_FDCWD, \"/etc/hostname\", R_OK) = 0").unwrap();
        assert_eq!(
            faccessat,
            Faccessat::from_raw(DirFd::AT_FDCWD, "/etc/hostname", AccessMode::R_OK, 0)
        );
    }

    #[test]
    fn parse_failure() {
        let faccessat = Faccessat::from_str("faccessat(4, \"file\", W_OK) = -1 EACCES").unwrap();
        assert_eq!(faccessat.dirfd, DirFd::new(4));
        assert_eq!(faccessat.mode, AccessMode::W_OK);
        assert_eq!(faccessat.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Faccessat::from_str("faccessat(AT_FDCWD, \"/f\", R_OK) 0").is_err());
        assert!(Faccessat::from_str("faccessat(AT_FDCWD, \"/f\") = 0").is_err());
        assert!(Faccessat::from_str("faccessat(AT_FDCWD, \"/f\", R_OK, 0) = 0").is_err());
        assert!(Faccessat::from_str("faccessat(\"/f\", R_OK) = 0").is_err());
        assert!(Faccessat::from_str("access(\"/f\", R_OK) = 0").is_err());
        assert!(Faccessat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "faccessat(AT_FDCWD, \"/etc/hostname\", R_OK) = 0",
            "faccessat(4, \"file\", W_OK) = -1 EACCES",
            "faccessat(AT_FDCWD, \"a, b\", F_OK) = -1 ENOENT",
        ] {
            assert_eq!(Faccessat::from_str(line).unwrap().to_string(), line);
        }
    }
}
