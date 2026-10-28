// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`tee(2)`] system call.
//!
//! [`tee(2)`]: https://man7.org/linux/man-pages/man2/tee.2.html

use super::fd::Fd;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use super::splice::SpliceFlags;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `tee(2)` system call.
///
/// ```text
/// ssize_t tee(int fd_in, int fd_out, size_t len, unsigned int flags);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tee {
    /// File descriptor the call reads from, with the path `strace` resolved
    /// for it when run with `-y`.
    pub fd_in: Fd,
    /// File descriptor the call writes to, with the path `strace` resolved
    /// for it when run with `-y`.
    pub fd_out: Fd,
    /// Number of bytes to copy.
    pub len: u64,
    /// Flags; see [`SpliceFlags`](super::splice::SpliceFlags).
    pub flags: SpliceFlags,
    /// Return value: the number of bytes copied, or the errno of the failure.
    pub result: LenResult,
}

impl Tee {
    /// Builds a `tee(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes copied on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        fd_in: i32,
        fd_out: i32,
        len: u64,
        flags: SpliceFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            fd_in: Fd::new(fd_in),
            fd_out: Fd::new(fd_out),
            len,
            flags,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Tee {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tee({}, {}, {}, {}) = {}",
            self.fd_in, self.fd_out, self.len, self.flags, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `tee(3, 4, 64, 0) = 64` or `tee(3, 4, 64, SPLICE_F_NONBLOCK) = -1 EAGAIN`.
impl FromStr for Tee {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("tee call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("tee(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("tee arguments", args));
        }
        let fd_in = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd_in", parts[0]))?;
        let fd_out = parts[1]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd_out", parts[1]))?;
        let len = parts[2]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("len", parts[2]))?;
        let flags = SpliceFlags::from_str(parts[3].trim())?;
        Ok(Self {
            fd_in,
            fd_out,
            len,
            flags,
            result: LenResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_success() {
        let call = Tee::from_raw(3, 4, 64, SpliceFlags(0), 64);
        assert_eq!(call.to_string(), "tee(3, 4, 64, 0) = 64");
        assert_eq!(call.result, LenResult::Len(64));
    }

    #[test]
    fn display_flags() {
        let call = Tee::from_raw(3, 4, 64, SpliceFlags::SPLICE_F_NONBLOCK, -11);
        assert_eq!(
            call.to_string(),
            "tee(3, 4, 64, SPLICE_F_NONBLOCK) = -1 EAGAIN"
        );
        assert_eq!(call.result, LenResult::Errno(Errno::EAGAIN));
    }

    #[test]
    fn parse_success() {
        let line = "tee(3, 4, 64, SPLICE_F_MORE) = 32";
        let call = Tee::from_str(line).unwrap();
        assert_eq!(call.fd_in, Fd::new(3));
        assert_eq!(call.fd_out, Fd::new(4));
        assert_eq!(call.len, 64);
        assert_eq!(call.flags, SpliceFlags::SPLICE_F_MORE);
        assert_eq!(call.result, LenResult::Len(32));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Tee::from_str("tee(3, 4, 64) = 0").is_err());
        assert!(Tee::from_str("tee(3, 4, 64, 0) 0").is_err());
        assert!(Tee::from_str("tee(x, 4, 64, 0) = 0").is_err());
        assert!(Tee::from_str("tee(3, 4, 64, 0, 0) = 0").is_err());
        assert!(Tee::from_str("tee(3, 4, x, 0) = 0").is_err());
        assert!(Tee::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "tee(3, 4, 64, 0) = 64",
            "tee(3, 4, 64, SPLICE_F_NONBLOCK) = -1 EAGAIN",
            "tee(5, 6, 4096, SPLICE_F_MOVE|SPLICE_F_MORE) = 4096",
            "tee(3</tmp/in>, 4</tmp/out>, 1024, 0) = -1 EBADF",
            "tee(3, 4, 0, 0x10 /* SPLICE_F_??? */) = 0",
        ] {
            assert_eq!(Tee::from_str(line).unwrap().to_string(), line);
        }
    }
}
