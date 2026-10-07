// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`mkdirat(2)`] system call.
//!
//! [`mkdirat(2)`]: https://man7.org/linux/man-pages/man2/mkdirat.2.html

use super::dirfd::DirFd;
use super::mode::Mode;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `mkdirat(2)` system call.
///
/// ```text
/// int mkdirat(int dirfd, const char *pathname, mode_t mode);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mkdirat {
    /// Directory against which the pathname is resolved, or [`DirFd::AT_FDCWD`].
    pub dirfd: DirFd,
    /// Pathname of the directory to create, without the terminating NUL byte.
    ///
    /// Interpreted relative to `dirfd` unless it is absolute.
    pub pathname: CString,
    /// Directory creation mode, the `mode` argument.
    pub mode: Mode,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Mkdirat {
    /// Builds a `mkdirat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(
        dirfd: DirFd,
        pathname: P,
        mode: Mode,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            mode,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Mkdirat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "mkdirat({}, {:?}, {}) = {}",
            self.dirfd,
            self.pathname.to_string_lossy(),
            self.mode,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `mkdirat(dirfd, "path", mode) = 0` or `... = -1 EEXIST`.
impl FromStr for Mkdirat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("mkdirat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("mkdirat(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 3 {
            return Err(ParseError::new("mkdirat arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        let mode = Mode::from_str(parts[2].trim())?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
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
        let mkdirat = Mkdirat::from_raw(DirFd::AT_FDCWD, "/tmp/dir", Mode(0o755), 0);
        assert_eq!(
            mkdirat.to_string(),
            "mkdirat(AT_FDCWD, \"/tmp/dir\", 0755) = 0"
        );
        assert!(mkdirat.result.is_ok());
    }

    #[test]
    fn display_dirfd_and_failure() {
        let mkdirat = Mkdirat::from_raw(DirFd(4), "sub", Mode(0o700), -13);
        assert_eq!(mkdirat.to_string(), "mkdirat(4, \"sub\", 0700) = -1 EACCES");
        assert!(!mkdirat.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let mkdirat = Mkdirat::from_str("mkdirat(AT_FDCWD, \"/tmp/dir\", 0755) = 0").unwrap();
        assert_eq!(
            mkdirat,
            Mkdirat::from_raw(DirFd::AT_FDCWD, "/tmp/dir", Mode(0o755), 0)
        );
        assert!(mkdirat.dirfd.is_cwd());
    }

    #[test]
    fn parse_failure() {
        let mkdirat = Mkdirat::from_str("mkdirat(4, \"sub\", 0700) = -1 ENOENT").unwrap();
        assert_eq!(mkdirat.dirfd, DirFd(4));
        assert_eq!(mkdirat.mode, Mode(0o700));
        assert_eq!(mkdirat.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Mkdirat::from_str("mkdirat(AT_FDCWD, \"/d\", 0755) 0").is_err());
        assert!(Mkdirat::from_str("mkdirat(\"/d\", 0755) = 0").is_err());
        assert!(Mkdirat::from_str("mkdirat(AT_FDCWD, \"/d\") = 0").is_err());
        assert!(Mkdirat::from_str("mkdirat(/d, \"/d\", 0755) = 0").is_err());
        assert!(Mkdirat::from_str("mkdir(\"/d\", 0755) = 0").is_err());
        assert!(Mkdirat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "mkdirat(AT_FDCWD, \"/tmp/dir\", 0755) = 0",
            "mkdirat(4, \"sub\", 0700) = -1 EACCES",
            "mkdirat(AT_FDCWD, \"a, b\", 0755) = -1 ENOENT",
            "mkdirat(0, \"quote\\\"here\", 1777) = -1 ENAMETOOLONG",
        ] {
            assert_eq!(Mkdirat::from_str(line).unwrap().to_string(), line);
        }
    }
}
