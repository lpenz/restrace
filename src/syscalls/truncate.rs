// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`truncate(2)`] system call.
//!
//! [`truncate(2)`]: https://man7.org/linux/man-pages/man2/truncate.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `truncate(2)` system call.
///
/// ```text
/// int truncate(const char *path, off_t length);
/// ```
///
/// `strace` prints the length as an unsigned value, so a negative length
/// shows up as a large number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Truncate {
    /// Pathname of the file to truncate, without the terminating NUL byte.
    pub path: CString,
    /// The new length of the file, in bytes.
    pub length: u64,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Truncate {
    /// Builds a `truncate(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `path` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(path: P, length: u64, raw_result: i64) -> Self {
        Self {
            path: CString::new(path.as_ref()).expect("path contains NUL byte"),
            length,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Truncate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "truncate({:?}, {}) = {}",
            self.path.to_string_lossy(),
            self.length,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `truncate("path", length) = 0` or `truncate("path", length) = -1 EINVAL`.
impl FromStr for Truncate {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("truncate call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("truncate(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 2 {
            return Err(ParseError::new("truncate arguments", args));
        }
        let path = parse::unquote(parts[0].trim(), "path")?;
        let length = parts[1]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("length", parts[1]))?;
        Ok(Self {
            path: CString::new(path).map_err(|_| ParseError::new("path", parts[0]))?,
            length,
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
        let call = Truncate::from_raw("/tmp/file", 1024, 0);
        assert_eq!(call.to_string(), "truncate(\"/tmp/file\", 1024) = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Truncate::from_raw("/tmp/file", 1024, -22);
        assert_eq!(
            call.to_string(),
            "truncate(\"/tmp/file\", 1024) = -1 EINVAL"
        );
        assert!(!call.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let call = Truncate::from_str("truncate(\"/tmp/file\", 1024) = 0").unwrap();
        assert_eq!(call, Truncate::from_raw("/tmp/file", 1024, 0));
        assert_eq!(call.path.to_str().unwrap(), "/tmp/file");
        assert_eq!(call.length, 1024);
    }

    #[test]
    fn parse_failure() {
        let call = Truncate::from_str("truncate(\"/tmp/file\", 1024) = -1 EACCES").unwrap();
        assert_eq!(call.path.to_str().unwrap(), "/tmp/file");
        assert_eq!(call.length, 1024);
        assert_eq!(call.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Truncate::from_str("truncate(\"/f\", 1024) 0").is_err());
        assert!(Truncate::from_str("truncate(/f, 1024) = 0").is_err());
        assert!(Truncate::from_str("truncate(\"/f\") = 0").is_err());
        assert!(Truncate::from_str("truncate(\"/f\", 1024, 2) = 0").is_err());
        assert!(Truncate::from_str("truncate(\"/f\", x) = 0").is_err());
        assert!(Truncate::from_str("truncate(\"/f\", 1024) = 1").is_err());
        assert!(Truncate::from_str("truncate(\"/f\", 1024) = -1").is_err());
        assert!(Truncate::from_str("ftruncate(3, 1024) = 0").is_err());
        assert!(Truncate::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "truncate(\"/tmp/file\", 1024) = 0",
            "truncate(\"/tmp/file\", 0) = 0",
            "truncate(\"/tmp/file\", 16405315391733093453) = -1 EBADF",
            "truncate(\"/tmp/file\", 1024) = -1 EINVAL",
            "truncate(\"/a, b\", 4096) = -1 EACCES",
            "truncate(\"quote\\\"here\", 512) = 0",
        ] {
            assert_eq!(Truncate::from_str(line).unwrap().to_string(), line);
        }
    }
}
