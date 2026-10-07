// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`creat(2)`] system call.
//!
//! [`creat(2)`]: https://man7.org/linux/man-pages/man2/creat.2.html

use super::fd::FdResult;
use super::mode::Mode;
use super::parse::{self, ParseError};
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `creat(2)` system call.
///
/// ```text
/// int creat(const char *pathname, mode_t mode);
/// ```
///
/// Equivalent to `open(pathname, O_CREAT|O_WRONLY|O_TRUNC, mode)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Creat {
    /// Pathname of the file to create, without the terminating NUL byte.
    pub pathname: CString,
    /// File creation mode, the `mode` argument.
    pub mode: Mode,
    /// Return value: a file descriptor, or the errno that caused the failure.
    pub result: FdResult,
}

impl Creat {
    /// Builds a `creat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: a file
    /// descriptor (>= 0) on success, or `-errno` on failure.
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
            result: FdResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Creat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "creat({:?}, {}) = {}",
            self.pathname.to_string_lossy(),
            self.mode,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `creat("path", mode) = fd` or `creat("path", mode) = -1 EACCES`.
impl FromStr for Creat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("creat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("creat(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 2 {
            return Err(ParseError::new("creat arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        let mode = Mode::from_str(parts[1].trim())?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
            mode,
            result: FdResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_success() {
        let creat = Creat::from_raw("/tmp/file", Mode(0o644), 3);
        assert_eq!(creat.to_string(), "creat(\"/tmp/file\", 0644) = 3");
        assert_eq!(creat.result.fd(), Some(3));
    }

    #[test]
    fn display_failure() {
        let creat = Creat::from_raw("/read-only/fs", Mode::S_IRUSR, -13);
        assert_eq!(
            creat.to_string(),
            "creat(\"/read-only/fs\", 0400) = -1 EACCES"
        );
        assert_eq!(creat.result.fd(), None);
    }

    #[test]
    fn parse_success() {
        let creat = Creat::from_str("creat(\"/tmp/file\", 0644) = 3").unwrap();
        assert_eq!(creat, Creat::from_raw("/tmp/file", Mode(0o644), 3));
        assert_eq!(creat.mode, Mode(0o644));
    }

    #[test]
    fn parse_failure() {
        let creat = Creat::from_str("creat(\"/f\", 0644) = -1 ENOENT").unwrap();
        assert_eq!(creat.pathname.to_str().unwrap(), "/f");
        assert_eq!(creat.result, FdResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Creat::from_str("creat(\"/f\", 0644) 3").is_err());
        assert!(Creat::from_str("creat(\"/f\") = 3").is_err());
        assert!(Creat::from_str("creat(\"/f\", 0644, x) = 3").is_err());
        assert!(Creat::from_str("creat(/f, 0644) = 3").is_err());
        assert!(Creat::from_str("open(\"/f\", O_RDONLY) = 3").is_err());
        assert!(Creat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "creat(\"/tmp/file\", 0644) = 3",
            "creat(\"/read-only/fs\", 0400) = -1 EACCES",
            "creat(\"/a, b\", 0000) = 0",
            "creat(\"quote\\\"here\", 7777) = -1 ENAMETOOLONG",
        ] {
            assert_eq!(Creat::from_str(line).unwrap().to_string(), line);
        }
    }
}
