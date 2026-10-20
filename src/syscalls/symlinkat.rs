// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`symlinkat(2)`] system call.
//!
//! [`symlinkat(2)`]: https://man7.org/linux/man-pages/man2/symlinkat.2.html

use super::dirfd::DirFd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `symlinkat(2)` system call.
///
/// ```text
/// int symlinkat(const char *target, int newdirfd, const char *linkpath);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symlinkat {
    /// Contents of the symbolic link, without the terminating NUL byte.
    pub target: CString,
    /// Directory against which the link pathname is resolved, or
    /// [`DirFd::AT_FDCWD`].
    pub newdirfd: DirFd,
    /// Pathname of the symbolic link to create, without the terminating NUL
    /// byte.
    ///
    /// Interpreted relative to `newdirfd` unless it is absolute.
    pub linkpath: CString,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Symlinkat {
    /// Builds a `symlinkat(2)` record from its arguments and raw return value.
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
        target: P,
        newdirfd: DirFd,
        linkpath: Q,
        raw_result: i64,
    ) -> Self {
        Self {
            target: CString::new(target.as_ref()).expect("pathname contains NUL byte"),
            newdirfd,
            linkpath: CString::new(linkpath.as_ref()).expect("pathname contains NUL byte"),
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Symlinkat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "symlinkat({:?}, {}, {:?}) = {}",
            self.target.to_string_lossy(),
            self.newdirfd,
            self.linkpath.to_string_lossy(),
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `symlinkat("target", AT_FDCWD, "path") = 0` or `... = -1 EEXIST`.
impl FromStr for Symlinkat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("symlinkat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("symlinkat(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 3 {
            return Err(ParseError::new("symlinkat arguments", args));
        }
        let target = parse::unquote(parts[0].trim(), "target")?;
        let newdirfd = DirFd::from_str(parts[1].trim())?;
        let linkpath = parse::unquote(parts[2].trim(), "linkpath")?;
        Ok(Self {
            target: CString::new(target).map_err(|_| ParseError::new("target", parts[0]))?,
            newdirfd,
            linkpath: CString::new(linkpath).map_err(|_| ParseError::new("linkpath", parts[2]))?,
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
        let symlinkat = Symlinkat::from_raw("target", DirFd::AT_FDCWD, "link", 0);
        assert_eq!(
            symlinkat.to_string(),
            "symlinkat(\"target\", AT_FDCWD, \"link\") = 0"
        );
        assert!(symlinkat.result.is_ok());
    }

    #[test]
    fn display_dirfd_and_failure() {
        let symlinkat = Symlinkat::from_raw("target", DirFd::new(5), "link", -17);
        assert_eq!(
            symlinkat.to_string(),
            "symlinkat(\"target\", 5, \"link\") = -1 EEXIST"
        );
    }

    #[test]
    fn parse_success() {
        let symlinkat =
            Symlinkat::from_str("symlinkat(\"target\", AT_FDCWD, \"link\") = 0").unwrap();
        assert_eq!(
            symlinkat,
            Symlinkat::from_raw("target", DirFd::AT_FDCWD, "link", 0)
        );
    }

    #[test]
    fn parse_dirfd_and_failure() {
        let symlinkat = Symlinkat::from_str("symlinkat(\"t\", 5, \"l\") = -1 EACCES").unwrap();
        assert_eq!(symlinkat.newdirfd, DirFd::new(5));
        assert_eq!(symlinkat.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Symlinkat::from_str("symlinkat(\"t\", AT_FDCWD, \"l\") 0").is_err());
        assert!(Symlinkat::from_str("symlinkat(\"t\", AT_FDCWD) = 0").is_err());
        assert!(Symlinkat::from_str("symlinkat(\"t\", \"l\") = 0").is_err());
        assert!(Symlinkat::from_str("symlink(\"t\", \"l\") = 0").is_err());
        assert!(Symlinkat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "symlinkat(\"target\", AT_FDCWD, \"link\") = 0",
            "symlinkat(\"t\", 5, \"l\") = -1 EEXIST",
            "symlinkat(\"a, b\", 0, \"c, d\") = -1 EACCES",
        ] {
            assert_eq!(Symlinkat::from_str(line).unwrap().to_string(), line);
        }
    }
}
