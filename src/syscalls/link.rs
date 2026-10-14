// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`link(2)`] system call.
//!
//! [`link(2)`]: https://man7.org/linux/man-pages/man2/link.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `link(2)` system call.
///
/// ```text
/// int link(const char *oldpath, const char *newpath);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Pathname of the existing file, without the terminating NUL byte.
    pub oldpath: CString,
    /// Pathname of the new link, without the terminating NUL byte.
    pub newpath: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Link {
    /// Builds a `link(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if a pathname contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>, Q: AsRef<[u8]>>(
        oldpath: P,
        newpath: Q,
        raw_result: i64,
    ) -> Self {
        Self {
            oldpath: CString::new(oldpath.as_ref()).expect("pathname contains NUL byte"),
            newpath: CString::new(newpath.as_ref()).expect("pathname contains NUL byte"),
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "link({:?}, {:?}) = {}",
            self.oldpath.to_string_lossy(),
            self.newpath.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `link("old", "new") = 0` or `... = -1 EEXIST`.
impl FromStr for Link {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("link call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("link(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 2 {
            return Err(ParseError::new("link arguments", args));
        }
        let oldpath = parse::unquote(parts[0].trim(), "oldpath")?;
        let newpath = parse::unquote(parts[1].trim(), "newpath")?;
        Ok(Self {
            oldpath: CString::new(oldpath).map_err(|_| ParseError::new("oldpath", parts[0]))?,
            newpath: CString::new(newpath).map_err(|_| ParseError::new("newpath", parts[1]))?,
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
        let link = Link::from_raw("/a", "/b", 0);
        assert_eq!(link.to_string(), "link(\"/a\", \"/b\") = 0");
        assert!(link.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let link = Link::from_raw("/a", "/b", -17);
        assert_eq!(link.to_string(), "link(\"/a\", \"/b\") = -1 EEXIST");
    }

    #[test]
    fn parse_success() {
        let link = Link::from_str("link(\"/a\", \"/b\") = 0").unwrap();
        assert_eq!(link, Link::from_raw("/a", "/b", 0));
    }

    #[test]
    fn parse_failure() {
        let link = Link::from_str("link(\"/a\", \"/b\") = -1 EACCES").unwrap();
        assert_eq!(link.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Link::from_str("link(\"/a\", \"/b\") 0").is_err());
        assert!(Link::from_str("link(\"/a\") = 0").is_err());
        assert!(Link::from_str("link(a, b) = 0").is_err());
        assert!(Link::from_str("linkat(AT_FDCWD, \"a\", AT_FDCWD, \"b\", 0) = 0").is_err());
        assert!(Link::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "link(\"/a\", \"/b\") = 0",
            "link(\"a, b\", \"c, d\") = -1 EEXIST",
            "link(\"quote\\\"here\", \"x\") = -1 EACCES",
        ] {
            assert_eq!(Link::from_str(line).unwrap().to_string(), line);
        }
    }
}
