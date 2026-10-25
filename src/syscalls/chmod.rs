// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`chmod(2)`] system call.
//!
//! [`chmod(2)`]: https://man7.org/linux/man-pages/man2/chmod.2.html

use super::chmodmode::ChmodMode;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `chmod(2)` system call.
///
/// ```text
/// int chmod(const char *pathname, mode_t mode);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chmod {
    /// Pathname of the file to change permissions of, without the terminating NUL byte.
    pub pathname: CString,
    /// File mode bits.
    pub mode: ChmodMode,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Chmod {
    /// Builds a `chmod(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(pathname: P, mode: ChmodMode, raw_result: i64) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            mode,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Chmod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "chmod({:?}, {}) = {}",
            self.pathname.to_string_lossy(),
            self.mode,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `chmod("path", 0644) = 0` or `chmod("path", 0644) = -1 EACCES`.
impl FromStr for Chmod {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("chmod call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("chmod(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 2 {
            return Err(ParseError::new("chmod arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        let mode = ChmodMode::from_str(parts[1].trim())?;
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
        let call = Chmod::from_raw("/tmp/f1", ChmodMode(0o644), 0);
        assert_eq!(call.to_string(), "chmod(\"/tmp/f1\", 0644) = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Chmod::from_raw("/tmp/f1", ChmodMode(0o644), -13);
        assert_eq!(call.to_string(), "chmod(\"/tmp/f1\", 0644) = -1 EACCES");
        assert!(!call.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let call = Chmod::from_str("chmod(\"/tmp/f1\", 0600) = 0").unwrap();
        assert_eq!(call.pathname.to_str().unwrap(), "/tmp/f1");
        assert_eq!(call.mode, ChmodMode(0o600));
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), "chmod(\"/tmp/f1\", 0600) = 0");
    }

    #[test]
    fn parse_failure() {
        let call = Chmod::from_str("chmod(\"/tmp/f1\", 0644) = -1 EACCES").unwrap();
        assert_eq!(call.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Chmod::from_str("chmod(\"/tmp/f1\", 0644) 0").is_err());
        assert!(Chmod::from_str("chmod(/tmp/f1, 0644) = 0").is_err());
        assert!(Chmod::from_str("chmod(\"/tmp/f1\") = 0").is_err());
        assert!(Chmod::from_str("chmod(\"/tmp/f1\", 0644, 1) = 0").is_err());
        assert!(Chmod::from_str("fchmod(3, 0644) = 0").is_err());
        assert!(Chmod::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "chmod(\"/tmp/f1\", 0644) = 0",
            "chmod(\"/tmp/f1\", 0600) = 0",
            "chmod(\"/a, b\", 0755) = -1 ENOENT",
        ] {
            assert_eq!(Chmod::from_str(line).unwrap().to_string(), line);
        }
    }
}
