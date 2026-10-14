// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`access(2)`] system call.
//!
//! [`access(2)`]: https://man7.org/linux/man-pages/man2/access.2.html

use super::accessmode::AccessMode;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `access(2)` system call.
///
/// ```text
/// int access(const char *pathname, int mode);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Access {
    /// Pathname of the file to test, without the terminating NUL byte.
    pub pathname: CString,
    /// Permissions to test for, the `mode` argument.
    pub mode: AccessMode,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Access {
    /// Builds an `access(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(pathname: P, mode: AccessMode, raw_result: i64) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            mode,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Access {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "access({:?}, {}) = {}",
            self.pathname.to_string_lossy(),
            self.mode,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `access("path", F_OK) = 0` or `access("path", R_OK) = -1 EACCES`.
impl FromStr for Access {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("access call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("access(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 2 {
            return Err(ParseError::new("access arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        let mode = AccessMode::from_str(parts[1].trim())?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
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
        let access = Access::from_raw("/etc/hostname", AccessMode::R_OK, 0);
        assert_eq!(access.to_string(), "access(\"/etc/hostname\", R_OK) = 0");
        assert!(access.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let access = Access::from_raw("/etc/hostname", AccessMode::W_OK, -13);
        assert_eq!(
            access.to_string(),
            "access(\"/etc/hostname\", W_OK) = -1 EACCES"
        );
    }

    #[test]
    fn parse_success() {
        let access = Access::from_str("access(\"/etc/hostname\", R_OK) = 0").unwrap();
        assert_eq!(
            access,
            Access::from_raw("/etc/hostname", AccessMode::R_OK, 0)
        );
    }

    #[test]
    fn parse_failure() {
        let access = Access::from_str("access(\"/etc/hostname\", W_OK) = -1 EACCES").unwrap();
        assert_eq!(access.mode, AccessMode::W_OK);
        assert_eq!(access.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Access::from_str("access(\"/f\", R_OK) 0").is_err());
        assert!(Access::from_str("access(\"/f\") = 0").is_err());
        assert!(Access::from_str("access(/f, R_OK) = 0").is_err());
        assert!(Access::from_str("faccessat(AT_FDCWD, \"/f\", R_OK) = 0").is_err());
        assert!(Access::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "access(\"/etc/hostname\", R_OK) = 0",
            "access(\"/etc/hostname\", W_OK) = -1 EACCES",
            "access(\"/file, with comma\", F_OK) = -1 ENOENT",
        ] {
            assert_eq!(Access::from_str(line).unwrap().to_string(), line);
        }
    }
}
