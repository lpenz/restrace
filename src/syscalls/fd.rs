// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Return values of the system calls that open a file descriptor.

use super::errno::Errno;
use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// The return value of a system call that opens a file descriptor,
/// such as [`open(2)`](super::open::Open) or [`creat(2)`](super::creat::Creat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdResult {
    /// The file descriptor that was opened.
    Fd(i32),
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
            Self::Fd(raw as i32)
        }
    }

    /// Returns the file descriptor, if the call succeeded.
    #[must_use]
    pub fn fd(self) -> Option<i32> {
        match self {
            Self::Fd(fd) => Some(fd),
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
                Errno::from_str(errno.trim()).map_err(|_| error())?,
            ));
        }
        if s == "-1" {
            return Err(error());
        }
        s.parse::<i32>().map(Self::Fd).map_err(|_| error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_fd() {
        assert_eq!(FdResult::from_raw(3), FdResult::Fd(3));
        assert_eq!(FdResult::from_raw(0), FdResult::Fd(0));
        assert_eq!(FdResult::from_raw(4096), FdResult::Fd(4096));
    }

    #[test]
    fn from_raw_errno() {
        assert_eq!(FdResult::from_raw(-2), FdResult::Errno(Errno::ENOENT));
        assert_eq!(FdResult::from_raw(-1), FdResult::Errno(Errno(1)));
    }

    #[test]
    fn fd() {
        assert_eq!(FdResult::Fd(3).fd(), Some(3));
        assert_eq!(FdResult::Errno(Errno::ENOENT).fd(), None);
    }

    #[test]
    fn parse() {
        assert_eq!(FdResult::from_str("3"), Ok(FdResult::Fd(3)));
        assert_eq!(
            FdResult::from_str("-1 ENOENT"),
            Ok(FdResult::Errno(Errno::ENOENT))
        );
        assert_eq!(
            FdResult::from_str("-1 9999"),
            Ok(FdResult::Errno(Errno(9999)))
        );
        assert!(FdResult::from_str("-1").is_err());
        assert!(FdResult::from_str("x").is_err());
    }

    #[test]
    fn round_trip() {
        for result in [
            FdResult::Fd(0),
            FdResult::Fd(4096),
            FdResult::Errno(Errno::ENOENT),
            FdResult::Errno(Errno(9999)),
        ] {
            assert_eq!(FdResult::from_str(&result.to_string()).unwrap(), result);
        }
    }
}
