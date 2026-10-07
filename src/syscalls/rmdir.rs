// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`rmdir(2)`] system call.
//!
//! [`rmdir(2)`]: https://man7.org/linux/man-pages/man2/rmdir.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `rmdir(2)` system call.
///
/// ```text
/// int rmdir(const char *pathname);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rmdir {
    /// Pathname of the directory to remove, without the terminating NUL byte.
    pub pathname: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Rmdir {
    /// Builds an `rmdir(2)` record from its argument and raw return value.
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

impl fmt::Display for Rmdir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rmdir({:?}) = {}",
            self.pathname.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `rmdir("path") = 0` or `rmdir("path") = -1 ENOENT`.
impl FromStr for Rmdir {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("rmdir call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("rmdir(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 1 {
            return Err(ParseError::new("rmdir arguments", args));
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
        let rmdir = Rmdir::from_raw("/tmp/dir", 0);
        assert_eq!(rmdir.to_string(), "rmdir(\"/tmp/dir\") = 0");
        assert!(rmdir.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let rmdir = Rmdir::from_raw("/tmp/dir", -2);
        assert_eq!(rmdir.to_string(), "rmdir(\"/tmp/dir\") = -1 ENOENT");
        assert!(!rmdir.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let rmdir = Rmdir::from_str("rmdir(\"/tmp/dir\") = 0").unwrap();
        assert_eq!(rmdir, Rmdir::from_raw("/tmp/dir", 0));
        assert_eq!(rmdir.pathname.to_str().unwrap(), "/tmp/dir");
    }

    #[test]
    fn parse_failure() {
        let rmdir = Rmdir::from_str("rmdir(\"/tmp/dir\") = -1 EACCES").unwrap();
        assert_eq!(rmdir.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Rmdir::from_str("rmdir(\"/tmp/dir\") 0").is_err());
        assert!(Rmdir::from_str("rmdir(/tmp/dir) = 0").is_err());
        assert!(Rmdir::from_str("rmdir(\"/a\", \"b\") = 0").is_err());
        assert!(Rmdir::from_str("chdir(\"/tmp\") = 0").is_err());
        assert!(Rmdir::from_str("rmdir(\"/tmp/dir\") = 1").is_err());
        assert!(Rmdir::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "rmdir(\"/tmp/dir\") = 0",
            "rmdir(\"relative\") = -1 ENOENT",
            "rmdir(\"/a, b\") = -1 EACCES",
            "rmdir(\"quote\\\"here\") = 0",
            "rmdir(\"\") = -1 ENOENT",
        ] {
            assert_eq!(Rmdir::from_str(line).unwrap().to_string(), line);
        }
    }
}
