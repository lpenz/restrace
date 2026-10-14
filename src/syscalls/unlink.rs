// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`unlink(2)`] system call.
//!
//! [`unlink(2)`]: https://man7.org/linux/man-pages/man2/unlink.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `unlink(2)` system call.
///
/// ```text
/// int unlink(const char *pathname);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unlink {
    /// Pathname of the file to remove, without the terminating NUL byte.
    pub pathname: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Unlink {
    /// Builds an `unlink(2)` record from its argument and raw return value.
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

impl fmt::Display for Unlink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unlink({:?}) = {}",
            self.pathname.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `unlink("path") = 0` or `unlink("path") = -1 ENOENT`.
impl FromStr for Unlink {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("unlink call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("unlink(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 1 {
            return Err(ParseError::new("unlink arguments", args));
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
        let unlink = Unlink::from_raw("/tmp/file", 0);
        assert_eq!(unlink.to_string(), "unlink(\"/tmp/file\") = 0");
        assert!(unlink.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let unlink = Unlink::from_raw("/tmp/file", -2);
        assert_eq!(unlink.to_string(), "unlink(\"/tmp/file\") = -1 ENOENT");
    }

    #[test]
    fn parse_success() {
        let unlink = Unlink::from_str("unlink(\"/tmp/file\") = 0").unwrap();
        assert_eq!(unlink, Unlink::from_raw("/tmp/file", 0));
    }

    #[test]
    fn parse_failure() {
        let unlink = Unlink::from_str("unlink(\"/tmp/file\") = -1 EACCES").unwrap();
        assert_eq!(unlink.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Unlink::from_str("unlink(\"/f\") 0").is_err());
        assert!(Unlink::from_str("unlink(/f) = 0").is_err());
        assert!(Unlink::from_str("unlink(\"a\", \"b\") = 0").is_err());
        assert!(Unlink::from_str("unlinkat(AT_FDCWD, \"f\", 0) = 0").is_err());
        assert!(Unlink::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "unlink(\"/tmp/file\") = 0",
            "unlink(\"relative\") = -1 ENOENT",
            "unlink(\"/a, b\") = -1 EACCES",
            "unlink(\"quote\\\"here\") = 0",
        ] {
            assert_eq!(Unlink::from_str(line).unwrap().to_string(), line);
        }
    }
}
