// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`close(2)`] system call.
//!
//! [`close(2)`]: https://man7.org/linux/man-pages/man2/close.2.html

use super::errno::Errno;
use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `close(2)` system call.
///
/// ```text
/// int close(int fd);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Close {
    /// The file descriptor to close.
    pub fd: i32,
    /// Return value: zero on success, or the errno of the failure.
    pub result: CloseResult,
}

impl Close {
    /// Builds a `close(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, raw_result: i64) -> Self {
        Self {
            fd,
            result: CloseResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Close {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "close({}) = {}", self.fd, self.result)
    }
}

/// The return value of `close(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseResult {
    /// The descriptor was closed; `close(2)` returned 0.
    Closed,
    /// The descriptor was left open; `close(2)` failed with this errno.
    Errno(Errno),
}

impl CloseResult {
    /// Converts a raw `close(2)` return value into a [`CloseResult`].
    ///
    /// The kernel reports errors as `-errno` in the range -1..=-4095;
    /// anything else counts as success, as `close(2)` only reports 0.
    #[must_use]
    pub fn from_raw(raw: i64) -> Self {
        if (-4095..=-1).contains(&raw) {
            Self::Errno(Errno(-raw as i32))
        } else {
            Self::Closed
        }
    }

    /// Returns true if the descriptor was closed.
    #[must_use]
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }
}

impl fmt::Display for CloseResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => f.write_str("0"),
            Self::Errno(errno) => write!(f, "-1 {errno}"),
        }
    }
}

/// Parses a return value as [`Display`](fmt::Display) writes it: 0, or `-1`
/// followed by the errno.
impl FromStr for CloseResult {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("return value", s);
        if let Some(errno) = s.strip_prefix("-1 ") {
            return Ok(Self::Errno(
                Errno::from_str(errno.trim()).map_err(|_| error())?,
            ));
        }
        if s == "0" {
            return Ok(Self::Closed);
        }
        Err(error())
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `close(fd) = 0` or `close(fd) = -1 EBADF`.
impl FromStr for Close {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("close call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let fd = args
            .strip_prefix("close(")
            .ok_or_else(syntax)?
            .trim()
            .parse::<i32>()
            .map_err(|_| syntax())?;
        Ok(Self {
            fd,
            result: CloseResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_success() {
        let close = Close::from_raw(3, 0);
        assert_eq!(close.to_string(), "close(3) = 0");
        assert!(close.result.is_closed());
    }

    #[test]
    fn display_failure() {
        let close = Close::from_raw(3, -9);
        assert_eq!(close.to_string(), "close(3) = -1 EBADF");
        assert!(!close.result.is_closed());
    }

    #[test]
    fn from_raw() {
        assert_eq!(CloseResult::from_raw(0), CloseResult::Closed);
        assert_eq!(CloseResult::from_raw(-9), CloseResult::Errno(Errno::EBADF));
        assert_eq!(CloseResult::from_raw(4096), CloseResult::Closed);
    }

    #[test]
    fn parse_success() {
        assert_eq!(
            Close::from_str("close(3) = 0").unwrap(),
            Close::from_raw(3, 0)
        );
        assert_eq!(CloseResult::from_str("0"), Ok(CloseResult::Closed));
    }

    #[test]
    fn parse_failure() {
        let close = Close::from_str("close(3) = -1 EBADF").unwrap();
        assert_eq!(close.fd, 3);
        assert_eq!(close.result, CloseResult::Errno(Errno::EBADF));
        assert_eq!(
            CloseResult::from_str("-1 9999"),
            Ok(CloseResult::Errno(Errno(9999)))
        );
    }

    #[test]
    fn parse_invalid() {
        assert!(Close::from_str("close(3) 0").is_err());
        assert!(Close::from_str("close(x) = 0").is_err());
        assert!(Close::from_str("close(3) = 1").is_err());
        assert!(Close::from_str("close(3) = -1").is_err());
        assert!(Close::from_str("open(\"/f\", O_RDONLY) = 3").is_err());
        assert!(CloseResult::from_str("3").is_err());
        assert!(CloseResult::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in ["close(0) = 0", "close(3) = -1 EBADF", "close(42) = -1 9999"] {
            assert_eq!(Close::from_str(line).unwrap().to_string(), line);
        }
    }
}
