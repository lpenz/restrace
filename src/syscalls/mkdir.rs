// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`mkdir(2)`] system call.
//!
//! [`mkdir(2)`]: https://man7.org/linux/man-pages/man2/mkdir.2.html

use super::mode::Mode;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `mkdir(2)` system call.
///
/// ```text
/// int mkdir(const char *pathname, mode_t mode);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mkdir {
    /// Pathname of the directory to create, without the terminating NUL byte.
    pub pathname: CString,
    /// Directory creation mode, the `mode` argument.
    pub mode: Mode,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Mkdir {
    /// Builds a `mkdir(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(pathname: P, mode: Mode, raw_result: i64) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            mode,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Mkdir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "mkdir({:?}, {}) = {}",
            self.pathname.to_string_lossy(),
            self.mode,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `mkdir("path", mode) = 0` or `mkdir("path", mode) = -1 EEXIST`.
impl FromStr for Mkdir {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("mkdir call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("mkdir(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 2 {
            return Err(ParseError::new("mkdir arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        let mode = Mode::from_str(parts[1].trim())?;
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
        let mkdir = Mkdir::from_raw("/tmp/dir", Mode(0o755), 0);
        assert_eq!(mkdir.to_string(), "mkdir(\"/tmp/dir\", 0755) = 0");
        assert!(mkdir.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let mkdir = Mkdir::from_raw("/tmp/dir", Mode::S_IRWXU, -17);
        assert_eq!(mkdir.to_string(), "mkdir(\"/tmp/dir\", 0700) = -1 EEXIST");
        assert!(!mkdir.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let mkdir = Mkdir::from_str("mkdir(\"/tmp/dir\", 0755) = 0").unwrap();
        assert_eq!(mkdir, Mkdir::from_raw("/tmp/dir", Mode(0o755), 0));
        assert_eq!(mkdir.mode, Mode(0o755));
    }

    #[test]
    fn parse_failure() {
        let mkdir = Mkdir::from_str("mkdir(\"/tmp/dir\", 0755) = -1 EACCES").unwrap();
        assert_eq!(mkdir.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Mkdir::from_str("mkdir(\"/d\", 0755) 0").is_err());
        assert!(Mkdir::from_str("mkdir(\"/d\") = 0").is_err());
        assert!(Mkdir::from_str("mkdir(\"/d\", 0755, x) = 0").is_err());
        assert!(Mkdir::from_str("mkdir(/d, 0755) = 0").is_err());
        assert!(Mkdir::from_str("mkdirat(AT_FDCWD, \"/d\", 0755) = 0").is_err());
        assert!(Mkdir::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "mkdir(\"/tmp/dir\", 0755) = 0",
            "mkdir(\"/tmp/dir\", 0700) = -1 EEXIST",
            "mkdir(\"/a, b\", 0000) = -1 EACCES",
            "mkdir(\"quote\\\"here\", 7777) = -1 ENAMETOOLONG",
            "mkdir(\"relative\", 0755) = -1 ENOENT",
        ] {
            assert_eq!(Mkdir::from_str(line).unwrap().to_string(), line);
        }
    }
}
