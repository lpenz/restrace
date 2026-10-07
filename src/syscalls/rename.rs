// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`rename(2)`] system call.
//!
//! [`rename(2)`]: https://man7.org/linux/man-pages/man2/rename.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `rename(2)` system call.
///
/// ```text
/// int rename(const char *oldpath, const char *newpath);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    /// Old pathname, without the terminating NUL byte.
    pub oldpath: CString,
    /// New pathname, without the terminating NUL byte.
    pub newpath: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Rename {
    /// Builds a `rename(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if pathnames contain an interior NUL byte, which cannot
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

impl fmt::Display for Rename {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rename({:?}, {:?}) = {}",
            self.oldpath.to_string_lossy(),
            self.newpath.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `rename("old", "new") = 0` or `rename("old", "new") = -1 ENOENT`.
impl FromStr for Rename {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("rename call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("rename(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 2 {
            return Err(ParseError::new("rename arguments", args));
        }
        let oldpath = parse::unquote(parts[0].trim(), "pathname")?;
        let newpath = parse::unquote(parts[1].trim(), "pathname")?;
        Ok(Self {
            oldpath: CString::new(oldpath).map_err(|_| ParseError::new("pathname", parts[0]))?,
            newpath: CString::new(newpath).map_err(|_| ParseError::new("pathname", parts[1]))?,
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
        let rename = Rename::from_raw("/a", "/b", 0);
        assert_eq!(rename.to_string(), "rename(\"/a\", \"/b\") = 0");
        assert!(rename.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let rename = Rename::from_raw("/a", "/b", -2);
        assert_eq!(rename.to_string(), "rename(\"/a\", \"/b\") = -1 ENOENT");
        assert!(!rename.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let rename = Rename::from_str("rename(\"/a\", \"/b\") = 0").unwrap();
        assert_eq!(rename, Rename::from_raw("/a", "/b", 0));
        assert_eq!(rename.oldpath.to_str().unwrap(), "/a");
        assert_eq!(rename.newpath.to_str().unwrap(), "/b");
    }

    #[test]
    fn parse_failure() {
        let rename = Rename::from_str("rename(\"/a\", \"/b\") = -1 EACCES").unwrap();
        assert_eq!(rename.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_quoted_comma() {
        let rename = Rename::from_str("rename(\"a, b\", \"c, d\") = 0").unwrap();
        assert_eq!(rename.oldpath.to_str().unwrap(), "a, b");
        assert_eq!(rename.newpath.to_str().unwrap(), "c, d");
    }

    #[test]
    fn parse_invalid() {
        assert!(Rename::from_str("rename(\"/a\", \"/b\") 0").is_err());
        assert!(Rename::from_str("rename(\"/a\") = 0").is_err());
        assert!(Rename::from_str("rename(\"/a\", \"/b\", x) = 0").is_err());
        assert!(Rename::from_str("renameat(AT_FDCWD, \"a\", AT_FDCWD, \"b\") = 0").is_err());
        assert!(Rename::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "rename(\"/a\", \"/b\") = 0",
            "rename(\"old\", \"new\") = -1 ENOENT",
            "rename(\"a, b\", \"c, d\") = -1 EISDIR",
            "rename(\"quote\\\"o\", \"n\") = 0",
        ] {
            assert_eq!(Rename::from_str(line).unwrap().to_string(), line);
        }
    }
}
