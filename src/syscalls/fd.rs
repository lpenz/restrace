// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! File descriptor arguments and return values.

use super::errno::Errno;
use super::parse::{self, ParseError};
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// A file descriptor argument of a system call, such as the `fd` of
/// [`read(2)`](super::read::Read) or the `dirfd` of
/// [`openat(2)`](super::openat::Openat).
///
/// When the trace was produced with `-y`, `strace` resolves the descriptor
/// against `/proc/<pid>/fd` and appends the path as associated information,
/// which is kept in [`path`](Self::path); without `-y` the path is `None` and
/// only the descriptor number is present.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fd {
    /// The descriptor number.
    pub fd: i32,
    /// The path `strace` resolved for the descriptor with `-y`, if any.
    pub path: Option<CString>,
}

impl Fd {
    /// Builds an [`Fd`] from a descriptor number, without a resolved path.
    #[must_use]
    pub const fn new(fd: i32) -> Self {
        Self { fd, path: None }
    }

    /// Returns the descriptor number.
    #[must_use]
    pub const fn fd(&self) -> i32 {
        self.fd
    }

    /// Returns the path `strace` resolved for the descriptor, if any.
    #[must_use]
    pub fn path(&self) -> Option<&CString> {
        self.path.as_ref()
    }
}

impl From<i32> for Fd {
    fn from(fd: i32) -> Self {
        Self::new(fd)
    }
}

impl fmt::Display for Fd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.fd)?;
        if let Some(path) = &self.path {
            write!(f, "<{}>", path.to_string_lossy())?;
        }
        Ok(())
    }
}

/// Parses a descriptor argument as [`Display`](fmt::Display) writes it: a
/// decimal descriptor, optionally followed by the `<...>` annotation `strace`
/// appends with `-y`.
impl FromStr for Fd {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let (value, annotation) = parse::split_annotation(s);
        let fd = value.parse::<i32>().map_err(|_| ParseError::new("fd", s))?;
        let path = annotation
            .map(|text| CString::new(text).map_err(|_| ParseError::new("fd", s)))
            .transpose()?;
        Ok(Self { fd, path })
    }
}

/// The return value of a system call that reports a file descriptor,
/// such as [`open(2)`](super::open::Open), [`creat(2)`](super::creat::Creat)
/// or [`dup(2)`](super::dup::Dup).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FdResult {
    /// The file descriptor that was opened, with the path `strace` resolved
    /// for it when run with `-y`.
    Fd(Fd),
    /// The error that prevented the file from being opened.
    Errno(Errno),
}

impl FdResult {
    /// Converts a raw return value into an [`FdResult`].
    ///
    /// The kernel reports errors as `-errno` in the range -1..=-4095.
    #[must_use]
    pub fn from_raw(raw: i64) -> Self {
        if (-4095..=-1).contains(&raw) {
            Self::Errno(Errno(-raw as i32))
        } else {
            Self::Fd(Fd::new(raw as i32))
        }
    }

    /// Returns the file descriptor number, if the call succeeded.
    #[must_use]
    pub fn fd(&self) -> Option<i32> {
        match self {
            Self::Fd(fd) => Some(fd.fd),
            Self::Errno(_) => None,
        }
    }
}

impl fmt::Display for FdResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fd(fd) => write!(f, "{fd}"),
            Self::Errno(errno) => write!(f, "-1 {errno}"),
        }
    }
}

/// Parses a return value as [`Display`](fmt::Display) writes it: a file
/// descriptor, or `-1` followed by the errno.
impl FromStr for FdResult {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("return value", s);
        if let Some(errno) = s.strip_prefix("-1 ") {
            return Ok(Self::Errno(
                Errno::from_str(parse::strip_description(errno.trim())).map_err(|_| error())?,
            ));
        }
        if s == "-1" {
            return Err(error());
        }
        Fd::from_str(s).map(Self::Fd).map_err(|_| error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fd(n: i32) -> Fd {
        Fd::new(n)
    }

    fn annotated(n: i32, path: &str) -> Fd {
        Fd {
            fd: n,
            path: Some(CString::new(path).unwrap()),
        }
    }

    #[test]
    fn from_raw_fd() {
        assert_eq!(FdResult::from_raw(3), FdResult::Fd(fd(3)));
        assert_eq!(FdResult::from_raw(0), FdResult::Fd(fd(0)));
        assert_eq!(FdResult::from_raw(4096), FdResult::Fd(fd(4096)));
    }

    #[test]
    fn from_raw_errno() {
        assert_eq!(FdResult::from_raw(-2), FdResult::Errno(Errno::ENOENT));
        assert_eq!(FdResult::from_raw(-1), FdResult::Errno(Errno(1)));
    }

    #[test]
    fn fd_accessor() {
        assert_eq!(FdResult::Fd(fd(3)).fd(), Some(3));
        assert_eq!(FdResult::Errno(Errno::ENOENT).fd(), None);
    }

    #[test]
    fn fd_display() {
        assert_eq!(fd(3).to_string(), "3");
        assert_eq!(annotated(3, "/tmp/file").to_string(), "3</tmp/file>");
        assert_eq!(fd(3).path(), None);
        assert_eq!(
            annotated(3, "/tmp/file")
                .path()
                .map(|p| p.to_str().unwrap()),
            Some("/tmp/file")
        );
    }

    #[test]
    fn fd_parse() {
        assert_eq!(Fd::from_str("3"), Ok(fd(3)));
        assert_eq!(Fd::from_str("3</tmp/file>"), Ok(annotated(3, "/tmp/file")));
        assert_eq!(
            Fd::from_str("3</dev/null<char 1:3>>"),
            Ok(annotated(3, "/dev/null<char 1:3>"))
        );
        assert_eq!(
            Fd::from_str("3</tmp/x>(deleted)"),
            Ok(annotated(3, "/tmp/x"))
        );
        assert!(Fd::from_str("x</tmp>").is_err());
    }

    #[test]
    fn parse() {
        assert_eq!(FdResult::from_str("3"), Ok(FdResult::Fd(fd(3))));
        assert_eq!(
            FdResult::from_str("3</tmp/file>"),
            Ok(FdResult::Fd(annotated(3, "/tmp/file")))
        );
        assert_eq!(
            FdResult::from_str("-1 ENOENT"),
            Ok(FdResult::Errno(Errno::ENOENT))
        );
        assert_eq!(
            FdResult::from_str("-1 9999"),
            Ok(FdResult::Errno(Errno(9999)))
        );
        assert_eq!(
            FdResult::from_str("-1 ENOENT (No such file or directory)"),
            Ok(FdResult::Errno(Errno::ENOENT))
        );
        assert!(FdResult::from_str("-1").is_err());
        assert!(FdResult::from_str("x").is_err());
    }

    #[test]
    fn round_trip() {
        for result in [
            FdResult::Fd(fd(0)),
            FdResult::Fd(fd(4096)),
            FdResult::Fd(annotated(3, "/tmp/file")),
            FdResult::Fd(annotated(3, "/dev/null<char 1:3>")),
            FdResult::Errno(Errno::ENOENT),
            FdResult::Errno(Errno(9999)),
        ] {
            assert_eq!(FdResult::from_str(&result.to_string()).unwrap(), result);
        }
    }
}
