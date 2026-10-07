// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`chdir(2)`] system call.
//!
//! [`chdir(2)`]: https://man7.org/linux/man-pages/man2/chdir.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `chdir(2)` system call.
///
/// ```text
/// int chdir(const char *path);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chdir {
    /// Pathname of the directory to change to, without the NUL byte.
    pub pathname: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Chdir {
    /// Builds a `chdir(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(pathname: P, raw_result: i64) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Chdir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "chdir({:?}) = {}",
            self.pathname.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `chdir("path") = 0` or `chdir("path") = -1 ENOENT`.
impl FromStr for Chdir {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("chdir call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("chdir(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 1 {
            return Err(ParseError::new("chdir arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
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
        let chdir = Chdir::from_raw("/tmp", 0);
        assert_eq!(chdir.to_string(), "chdir(\"/tmp\") = 0");
        assert!(chdir.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let chdir = Chdir::from_raw("/nonexistent", -2);
        assert_eq!(chdir.to_string(), "chdir(\"/nonexistent\") = -1 ENOENT");
        assert!(!chdir.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let chdir = Chdir::from_str("chdir(\"/tmp\") = 0").unwrap();
        assert_eq!(chdir, Chdir::from_raw("/tmp", 0));
        assert_eq!(chdir.pathname.to_str().unwrap(), "/tmp");
    }

    #[test]
    fn parse_failure() {
        let chdir = Chdir::from_str("chdir(\"/nonexistent\") = -1 ENOENT").unwrap();
        assert_eq!(chdir.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Chdir::from_str("chdir(\"/tmp\") 0").is_err());
        assert!(Chdir::from_str("chdir(/tmp) = 0").is_err());
        assert!(Chdir::from_str("chdir(\"/a\", \"b\") = 0").is_err());
        assert!(Chdir::from_str("fchdir(3) = 0").is_err());
        assert!(Chdir::from_str("chdir(\"/tmp\") = 1").is_err());
        assert!(Chdir::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "chdir(\"/tmp\") = 0",
            "chdir(\"relative\") = -1 ENOENT",
            "chdir(\"/a, b\") = -1 ENOTDIR",
            "chdir(\"quote\\\"here\") = 0",
            "chdir(\"\") = -1 ENOENT",
        ] {
            assert_eq!(Chdir::from_str(line).unwrap().to_string(), line);
        }
    }
}
