// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`dup3(2)`] system call.
//!
//! [`dup3(2)`]: https://man7.org/linux/man-pages/man2/dup3.2.html

use super::fd::FdResult;
use super::flags;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Named bits of [`Dup3Flags`], in [`Display`](fmt::Display) order.
const NAMED_FLAGS: &[(u32, &str)] = &[(Dup3Flags::O_CLOEXEC.0, "O_CLOEXEC")];

/// Arguments and return value of the `dup3(2)` system call.
///
/// ```text
/// int dup3(int oldfd, int newfd, int flags);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dup3 {
    /// The file descriptor to duplicate.
    pub oldfd: i32,
    /// The file descriptor to make a copy of `oldfd`.
    pub newfd: i32,
    /// Flags, the `flags` argument; see [`Dup3Flags`].
    pub flags: Dup3Flags,
    /// Return value: the new file descriptor, or the errno that caused the
    /// failure.
    pub result: FdResult,
}

impl Dup3 {
    /// Builds a `dup3(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the new
    /// file descriptor (>= 0) on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(oldfd: i32, newfd: i32, flags: Dup3Flags, raw_result: i64) -> Self {
        Self {
            oldfd,
            newfd,
            flags,
            result: FdResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Dup3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "dup3({}, {}, {}) = {}",
            self.oldfd, self.newfd, self.flags, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `dup3(oldfd, newfd, flags) = fd` or `... = -1 EINVAL`.
impl FromStr for Dup3 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("dup3 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("dup3(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 3 {
            return Err(ParseError::new("dup3 arguments", args));
        }
        let oldfd = parts[0].trim().parse::<i32>().map_err(|_| syntax())?;
        let newfd = parts[1].trim().parse::<i32>().map_err(|_| syntax())?;
        let flags = Dup3Flags::from_str(parts[2].trim())?;
        Ok(Self {
            oldfd,
            newfd,
            flags,
            result: FdResult::from_str(result)?,
        })
    }
}

/// The `flags` argument of `dup3(2)`.
///
/// [`Dup3Flags::O_CLOEXEC`] is the only flag the kernel accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct Dup3Flags(pub u32);

impl Dup3Flags {
    /// Close the new file descriptor on `exec`.
    pub const O_CLOEXEC: Self = Self(0o2000000);
}

impl fmt::Display for Dup3Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_FLAGS, "O_???")
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them.
impl FromStr for Dup3Flags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        flags::parse(s, NAMED_FLAGS, "flags").map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_success() {
        let dup3 = Dup3::from_raw(0, 4, Dup3Flags(0), 4);
        assert_eq!(dup3.to_string(), "dup3(0, 4, 0) = 4");
        assert_eq!(dup3.result.fd(), Some(4));
    }

    #[test]
    fn display_flags() {
        let dup3 = Dup3::from_raw(0, 4, Dup3Flags::O_CLOEXEC, 4);
        assert_eq!(dup3.to_string(), "dup3(0, 4, O_CLOEXEC) = 4");
    }

    #[test]
    fn display_failure() {
        let dup3 = Dup3::from_raw(3, 3, Dup3Flags::O_CLOEXEC, -22);
        assert_eq!(dup3.to_string(), "dup3(3, 3, O_CLOEXEC) = -1 EINVAL");
        assert_eq!(dup3.result.fd(), None);
    }

    #[test]
    fn parse_success() {
        let dup3 = Dup3::from_str("dup3(0, 4, O_CLOEXEC) = 4").unwrap();
        assert_eq!(dup3, Dup3::from_raw(0, 4, Dup3Flags::O_CLOEXEC, 4));
        assert_eq!(dup3.oldfd, 0);
        assert_eq!(dup3.newfd, 4);
        assert_eq!(dup3.flags, Dup3Flags::O_CLOEXEC);
    }

    #[test]
    fn parse_failure() {
        let dup3 = Dup3::from_str("dup3(1, 3, 0) = -1 EBADF").unwrap();
        assert_eq!(dup3.oldfd, 1);
        assert_eq!(dup3.newfd, 3);
        assert_eq!(dup3.flags, Dup3Flags(0));
        assert_eq!(dup3.result, FdResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_invalid() {
        assert!(Dup3::from_str("dup3(0, 4, 0) 4").is_err());
        assert!(Dup3::from_str("dup3(0, 4) = 4").is_err());
        assert!(Dup3::from_str("dup3(0, 4, 0, 1) = 4").is_err());
        assert!(Dup3::from_str("dup3(x, 4, 0) = 4").is_err());
        assert!(Dup3::from_str("dup3(0, 4, BOGUS) = 4").is_err());
        assert!(Dup3::from_str("dup2(0, 4) = 4").is_err());
        assert!(Dup3::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "dup3(0, 4, 0) = 4",
            "dup3(0, 4, O_CLOEXEC) = 4",
            "dup3(1, 3, 0) = -1 EBADF",
            "dup3(2, 5, O_CLOEXEC|0x1) = -1 9999",
        ] {
            assert_eq!(Dup3::from_str(line).unwrap().to_string(), line);
        }
    }
}
