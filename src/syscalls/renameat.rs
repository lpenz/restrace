// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`renameat(2)`] system call.
//!
//! [`renameat(2)`]: https://man7.org/linux/man-pages/man2/renameat.2.html

use super::dirfd::DirFd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `renameat(2)` system call.
///
/// ```text
/// int renameat(int olddirfd, const char *oldpath, int newdirfd, const char *newpath);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renameat {
    /// Directory against which the old pathname is resolved.
    pub olddirfd: DirFd,
    /// Old pathname, without the terminating NUL byte.
    ///
    /// Interpreted relative to `olddirfd` unless it is absolute.
    pub oldpath: CString,
    /// Directory against which the new pathname is resolved.
    pub newdirfd: DirFd,
    /// New pathname, without the terminating NUL byte.
    ///
    /// Interpreted relative to `newdirfd` unless it is absolute.
    pub newpath: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Renameat {
    /// Builds a `renameat(2)` record from its arguments and raw return value.
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
        olddirfd: DirFd,
        oldpath: P,
        newdirfd: DirFd,
        newpath: Q,
        raw_result: i64,
    ) -> Self {
        Self {
            olddirfd,
            oldpath: CString::new(oldpath.as_ref()).expect("pathname contains NUL byte"),
            newdirfd,
            newpath: CString::new(newpath.as_ref()).expect("pathname contains NUL byte"),
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Renameat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "renameat({}, {:?}, {}, {:?}) = {}",
            self.olddirfd,
            self.oldpath.to_string_lossy(),
            self.newdirfd,
            self.newpath.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `renameat(AT_FDCWD, "old", AT_FDCWD, "new") = 0`.
impl FromStr for Renameat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("renameat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("renameat(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 4 {
            return Err(ParseError::new("renameat arguments", args));
        }
        let olddirfd = DirFd::from_str(parts[0].trim())?;
        let oldpath = parse::unquote(parts[1].trim(), "pathname")?;
        let newdirfd = DirFd::from_str(parts[2].trim())?;
        let newpath = parse::unquote(parts[3].trim(), "pathname")?;
        Ok(Self {
            olddirfd,
            oldpath: CString::new(oldpath).map_err(|_| ParseError::new("pathname", parts[1]))?,
            newdirfd,
            newpath: CString::new(newpath).map_err(|_| ParseError::new("pathname", parts[3]))?,
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
        let renameat = Renameat::from_raw(DirFd::AT_FDCWD, "/a", DirFd::AT_FDCWD, "/b", 0);
        assert_eq!(
            renameat.to_string(),
            "renameat(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\") = 0"
        );
        assert!(renameat.result.is_ok());
    }

    #[test]
    fn display_mixed_dirs() {
        let renameat = Renameat::from_raw(DirFd(3), "a", DirFd(4), "b", -21);
        assert_eq!(
            renameat.to_string(),
            "renameat(3, \"a\", 4, \"b\") = -1 EISDIR"
        );
    }

    #[test]
    fn parse_success() {
        let renameat =
            Renameat::from_str("renameat(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\") = 0").unwrap();
        assert_eq!(
            renameat,
            Renameat::from_raw(DirFd::AT_FDCWD, "/a", DirFd::AT_FDCWD, "/b", 0)
        );
        assert!(renameat.olddirfd.is_cwd());
        assert!(renameat.newdirfd.is_cwd());
    }

    #[test]
    fn parse_failure() {
        let renameat = Renameat::from_str("renameat(3, \"a\", 4, \"b\") = -1 EACCES").unwrap();
        assert_eq!(renameat.olddirfd, DirFd(3));
        assert_eq!(renameat.newdirfd, DirFd(4));
        assert_eq!(renameat.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Renameat::from_str("renameat(AT_FDCWD, \"a\", AT_FDCWD, \"b\") 0").is_err());
        assert!(Renameat::from_str("renameat(AT_FDCWD, \"a\", AT_FDCWD) = 0").is_err());
        assert!(Renameat::from_str("renameat(\"a\", AT_FDCWD, \"b\") = 0").is_err());
        assert!(Renameat::from_str("rename(\"a\", \"b\") = 0").is_err());
        assert!(Renameat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "renameat(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\") = 0",
            "renameat(3, \"a\", 4, \"b\") = -1 EISDIR",
            "renameat(AT_FDCWD, \"a, b\", 0, \"c, d\") = -1 ENOENT",
        ] {
            assert_eq!(Renameat::from_str(line).unwrap().to_string(), line);
        }
    }
}
