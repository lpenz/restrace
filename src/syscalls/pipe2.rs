// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`pipe2(2)`] system call.
//!
//! [`pipe2(2)`]: https://man7.org/linux/man-pages/man2/pipe.2.html

use super::flags;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::fmt;
use std::str::FromStr;

/// Named bits of [`Pipe2Flags`], in [`Display`](fmt::Display) order.
const NAMED_FLAGS: &[(u32, &str)] = &[
    (Pipe2Flags::O_NONBLOCK.0, "O_NONBLOCK"),
    (Pipe2Flags::O_DIRECT.0, "O_DIRECT"),
    (Pipe2Flags::O_CLOEXEC.0, "O_CLOEXEC"),
];

/// Arguments and return value of the `pipe2(2)` system call.
///
/// ```text
/// int pipe2(int pipefd[2], int flags);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pipe2 {
    /// The two file descriptors of the pipe: the read end first, then the
    /// write end.
    ///
    /// Only present with the values the kernel wrote on success; a failure
    /// leaves the array untouched, so the parser only reports the successful
    /// form.
    pub pipefd: [i32; 2],
    /// Flags, the `flags` argument; see [`Pipe2Flags`].
    pub flags: Pipe2Flags,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Pipe2 {
    /// Builds a `pipe2(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(pipefd: [i32; 2], flags: Pipe2Flags, raw_result: i64) -> Self {
        Self {
            pipefd,
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Pipe2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pipe2([{}, {}], {}) = {}",
            self.pipefd[0], self.pipefd[1], self.flags, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `pipe2([fd0, fd1], flags) = 0` or `... = -1 EMFILE`.
impl FromStr for Pipe2 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("pipe2 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("pipe2(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 2 {
            return Err(ParseError::new("pipe2 arguments", args));
        }
        let pipefd = parse_pipefd(parts[0])?;
        let flags = Pipe2Flags::from_str(parts[1].trim())?;
        Ok(Self {
            pipefd,
            flags,
            result: OkResult::from_str(result)?,
        })
    }
}

/// Parses the `[fd0, fd1]` array argument of `pipe2(2)`.
fn parse_pipefd(input: &str) -> Result<[i32; 2], ParseError> {
    let error = || ParseError::new("pipefd", input);
    let body = input
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(error)?;
    let mut fds = body.split(',');
    let first = fds.next().ok_or_else(error)?.trim();
    let second = fds.next().ok_or_else(error)?.trim();
    if fds.next().is_some() {
        return Err(error());
    }
    Ok([
        first.parse::<i32>().map_err(|_| error())?,
        second.parse::<i32>().map_err(|_| error())?,
    ])
}

/// The `flags` argument of `pipe2(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct Pipe2Flags(pub u32);

impl Pipe2Flags {
    /// Open the read and write ends in nonblocking mode.
    pub const O_NONBLOCK: Self = Self(0o4000);
    /// Perform I/O in packet mode.
    pub const O_DIRECT: Self = Self(0o40000);
    /// Close both file descriptors on `exec`.
    pub const O_CLOEXEC: Self = Self(0o2000000);
}

impl fmt::Display for Pipe2Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_FLAGS)
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them.
impl FromStr for Pipe2Flags {
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
        let pipe2 = Pipe2::from_raw([3, 4], Pipe2Flags(0), 0);
        assert_eq!(pipe2.to_string(), "pipe2([3, 4], 0) = 0");
        assert!(pipe2.result.is_ok());
    }

    #[test]
    fn display_flags() {
        let pipe2 = Pipe2::from_raw([5, 6], Pipe2Flags::O_CLOEXEC, 0);
        assert_eq!(pipe2.to_string(), "pipe2([5, 6], O_CLOEXEC) = 0");

        let all = Pipe2Flags(0o4000 | 0o40000 | 0o2000000);
        let pipe2 = Pipe2::from_raw([7, 8], all, 0);
        assert_eq!(
            pipe2.to_string(),
            "pipe2([7, 8], O_NONBLOCK|O_DIRECT|O_CLOEXEC) = 0"
        );
    }

    #[test]
    fn display_failure() {
        let pipe2 = Pipe2::from_raw([3, 4], Pipe2Flags(0), -24);
        assert_eq!(pipe2.to_string(), "pipe2([3, 4], 0) = -1 EMFILE");
        assert!(!pipe2.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let pipe2 = Pipe2::from_str("pipe2([3, 4], O_NONBLOCK|O_CLOEXEC) = 0").unwrap();
        assert_eq!(
            pipe2,
            Pipe2::from_raw([3, 4], Pipe2Flags(0o4000 | 0o2000000), 0)
        );
        assert_eq!(pipe2.pipefd, [3, 4]);
        assert_eq!(pipe2.flags, Pipe2Flags(0o4000 | 0o2000000));
    }

    #[test]
    fn parse_failure() {
        let pipe2 = Pipe2::from_str("pipe2([9, 10], 0) = -1 EMFILE").unwrap();
        assert_eq!(pipe2.pipefd, [9, 10]);
        assert_eq!(pipe2.flags, Pipe2Flags(0));
        assert_eq!(pipe2.result, OkResult::Errno(Errno::EMFILE));
    }

    #[test]
    fn parse_invalid() {
        assert!(Pipe2::from_str("pipe2([3, 4], 0) 0").is_err());
        assert!(Pipe2::from_str("pipe2([3, 4]) = 0").is_err());
        assert!(Pipe2::from_str("pipe2(3, 4, 0) = 0").is_err());
        assert!(Pipe2::from_str("pipe2([3], 0) = 0").is_err());
        assert!(Pipe2::from_str("pipe2([3, 4, 5], 0) = 0").is_err());
        assert!(Pipe2::from_str("pipe2([x, 4], 0) = 0").is_err());
        assert!(Pipe2::from_str("pipe2([3, 4], BOGUS) = 0").is_err());
        assert!(Pipe2::from_str("pipe([3, 4]) = 0").is_err());
        assert!(Pipe2::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "pipe2([3, 4], 0) = 0",
            "pipe2([5, 6], O_CLOEXEC) = 0",
            "pipe2([7, 8], O_NONBLOCK|O_DIRECT|O_CLOEXEC) = 0",
            "pipe2([3, 4], 0) = -1 EMFILE",
            "pipe2([3, 4], O_NONBLOCK|0x1) = -1 9999",
        ] {
            assert_eq!(Pipe2::from_str(line).unwrap().to_string(), line);
        }
    }
}
