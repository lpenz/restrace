// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`splice(2)`] system call.
//!
//! [`splice(2)`]: https://man7.org/linux/man-pages/man2/splice.2.html

use super::buffer::Buffer;
use super::fd::Fd;
use super::flags;
use super::indirect::Indirect;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Named bits of [`SpliceFlags`], in [`Display`](fmt::Display) order.
const NAMED_FLAGS: &[(u32, &str)] = &[
    (SpliceFlags::SPLICE_F_MOVE.0, "SPLICE_F_MOVE"),
    (SpliceFlags::SPLICE_F_NONBLOCK.0, "SPLICE_F_NONBLOCK"),
    (SpliceFlags::SPLICE_F_MORE.0, "SPLICE_F_MORE"),
    (SpliceFlags::SPLICE_F_GIFT.0, "SPLICE_F_GIFT"),
];

/// Arguments and return value of the `splice(2)` system call.
///
/// ```text
/// ssize_t splice(int fd_in, loff_t *off_in, int fd_out,
///                loff_t *off_out, size_t len, unsigned int flags);
/// ```
///
/// When `off_in` or `off_out` is not null, `strace` decodes the offset it
/// points to and prints it between square brackets; a null pointer is printed
/// as `NULL` and one it cannot read as a raw address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Splice {
    /// File descriptor the call reads from, with the path `strace` resolved
    /// for it when run with `-y`.
    pub fd_in: Fd,
    /// Offset the call reads from, or `NULL` to read from the file offset.
    pub off_in: Buffer<Indirect<i64>>,
    /// File descriptor the call writes to, with the path `strace` resolved
    /// for it when run with `-y`.
    pub fd_out: Fd,
    /// Offset the call writes to, or `NULL` to write at the file offset.
    pub off_out: Buffer<Indirect<i64>>,
    /// Number of bytes to transfer.
    pub len: u64,
    /// Flags, the `flags` argument; see [`SpliceFlags`].
    pub flags: SpliceFlags,
    /// Return value: the number of bytes transferred, or the errno of the
    /// failure.
    pub result: LenResult,
}

impl Splice {
    /// Builds a `splice(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes transferred on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        fd_in: i32,
        off_in: Buffer<Indirect<i64>>,
        fd_out: i32,
        off_out: Buffer<Indirect<i64>>,
        len: u64,
        flags: SpliceFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            fd_in: Fd::new(fd_in),
            off_in,
            fd_out: Fd::new(fd_out),
            off_out,
            len,
            flags,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Splice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "splice({}, {}, {}, {}, {}, {}) = {}",
            self.fd_in, self.off_in, self.fd_out, self.off_out, self.len, self.flags, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `splice(5, NULL, 4, [10], 64, SPLICE_F_MOVE) = 64` or
/// `splice(5, [5], 999, NULL, 64, 0) = -1 EBADF`.
impl FromStr for Splice {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("splice call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("splice(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 6 {
            return Err(ParseError::new("splice arguments", args));
        }
        let fd_in = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd_in", parts[0]))?;
        let off_in = super::buffer::parse(parts[1].trim())?;
        let fd_out = parts[2]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd_out", parts[2]))?;
        let off_out = super::buffer::parse(parts[3].trim())?;
        let len = parts[4]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("len", parts[4]))?;
        let flags = SpliceFlags::from_str(parts[5].trim())?;
        Ok(Self {
            fd_in,
            off_in,
            fd_out,
            off_out,
            len,
            flags,
            result: LenResult::from_str(result)?,
        })
    }
}

/// The `flags` argument of the `splice(2)` and `vmsplice(2)` system calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct SpliceFlags(pub u32);

impl SpliceFlags {
    /// Move pages instead of copying them.
    pub const SPLICE_F_MOVE: Self = Self(0x1);
    /// Do not block on the pipe.
    pub const SPLICE_F_NONBLOCK: Self = Self(0x2);
    /// Expect more data to follow.
    pub const SPLICE_F_MORE: Self = Self(0x4);
    /// The pages passed in are a gift.
    pub const SPLICE_F_GIFT: Self = Self(0x8);
}

impl fmt::Display for SpliceFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_FLAGS, "SPLICE_F_???")
    }
}

impl FromStr for SpliceFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        flags::parse(s, NAMED_FLAGS, "flags").map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    fn value(offset: i64) -> Buffer<Indirect<i64>> {
        Buffer::Decoded(Indirect::new(offset))
    }

    #[test]
    fn display_success() {
        let call = Splice::from_raw(
            5,
            value(5),
            4,
            Buffer::Address(0),
            64,
            SpliceFlags::SPLICE_F_MOVE,
            64,
        );
        assert_eq!(
            call.to_string(),
            "splice(5, [5], 4, NULL, 64, SPLICE_F_MOVE) = 64"
        );
        assert_eq!(call.result, LenResult::Len(64));
    }

    #[test]
    fn display_failure() {
        let call = Splice::from_raw(5, value(5), 999, Buffer::Address(0), 64, SpliceFlags(0), -9);
        assert_eq!(
            call.to_string(),
            "splice(5, [5], 999, NULL, 64, 0) = -1 EBADF"
        );
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
    }

    #[test]
    fn display_unknown_flags() {
        let call = Splice::from_raw(
            5,
            Buffer::Address(0x1),
            4,
            Buffer::Address(0),
            0,
            SpliceFlags(0x10),
            0,
        );
        assert_eq!(
            call.to_string(),
            "splice(5, 0x1, 4, NULL, 0, 0x10 /* SPLICE_F_??? */) = 0"
        );
    }

    #[test]
    fn parse_success() {
        let line = "splice(5, [5], 4, [10], 64, SPLICE_F_MOVE|SPLICE_F_MORE) = 64";
        let call = Splice::from_str(line).unwrap();
        assert_eq!(call.fd_in, Fd::new(5));
        assert_eq!(call.off_in, value(5));
        assert_eq!(call.fd_out, Fd::new(4));
        assert_eq!(call.off_out, value(10));
        assert_eq!(call.len, 64);
        assert_eq!(call.flags, SpliceFlags(0x5));
        assert_eq!(call.result, LenResult::Len(64));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Splice::from_str("splice(5, NULL, 4, NULL, 64, 0) 64").is_err());
        assert!(Splice::from_str("splice(5, NULL, 4, NULL, 64) = 0").is_err());
        assert!(Splice::from_str("splice(x, NULL, 4, NULL, 64, 0) = 0").is_err());
        assert!(Splice::from_str("splice(5, NULL, 4, NULL, 64, 0, 0) = 0").is_err());
        assert!(Splice::from_str("splice(5, NULL, 4, NULL, 64, BOGUS) = 0").is_err());
        assert!(Splice::from_str("splice(5, 5, 4, NULL, 64, 0) = 0").is_err());
        assert!(Splice::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "splice(5, NULL, 4, NULL, 64, 0) = 64",
            "splice(5, [5], 4, NULL, 64, SPLICE_F_MOVE) = 64",
            "splice(5, [5], 4, [10], 64, SPLICE_F_MOVE|SPLICE_F_NONBLOCK) = 64",
            "splice(5, [5], 999, NULL, 64, 0) = -1 EBADF",
            "splice(999, 0x12345678, 998, 0x87654321, 64, 0) = -1 EBADF",
            "splice(5, NULL, 4, NULL, 0, 0x10 /* SPLICE_F_??? */) = 0",
        ] {
            assert_eq!(Splice::from_str(line).unwrap().to_string(), line);
        }
    }
}
