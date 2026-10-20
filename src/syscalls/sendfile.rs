// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`sendfile(2)`] system call.
//!
//! [`sendfile(2)`]: https://man7.org/linux/man-pages/man2/sendfile.2.html

use super::buffer::Buffer;
use super::fd::Fd;
use super::indirect::Indirect;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `sendfile(2)` system call.
///
/// ```text
/// ssize_t sendfile(int out_fd, int in_fd, off_t *offset, size_t count);
/// ```
///
/// `strace` reads the offset `offset` points to and prints it unsigned
/// between square brackets, together with the value the call moved it to,
/// such as `[0 => 10]` (older `strace` releases printed `[0] => [10]`);
/// a null pointer is printed as `NULL` and one it cannot read as a raw
/// address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sendfile {
    /// File descriptor the call writes to, with the path `strace` resolved
    /// for it when run with `-y`.
    pub out_fd: Fd,
    /// File descriptor the call reads from, with the path `strace` resolved
    /// for it when run with `-y`.
    pub in_fd: Fd,
    /// Offset the call reads from and advances, or `NULL` to read from and
    /// advance the file offset of `in_fd`.
    pub offset: Buffer<Indirect<u64>>,
    /// Number of bytes to transfer.
    pub count: u64,
    /// Return value: the number of bytes transferred, or the errno of the
    /// failure.
    pub result: LenResult,
}

impl Sendfile {
    /// Builds a `sendfile(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes transferred on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        out_fd: i32,
        in_fd: i32,
        offset: Buffer<Indirect<u64>>,
        count: u64,
        raw_result: i64,
    ) -> Self {
        Self {
            out_fd: Fd::new(out_fd),
            in_fd: Fd::new(in_fd),
            offset,
            count,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Sendfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sendfile({}, {}, {}, {}) = {}",
            self.out_fd, self.in_fd, self.offset, self.count, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `sendfile(4, 3, [0 => 10], 100) = 10` or
/// `sendfile(4, 3, NULL, 100) = -1 EBADF`.
impl FromStr for Sendfile {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("sendfile call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("sendfile(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("sendfile arguments", args));
        }
        let out_fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("out_fd", parts[0]))?;
        let in_fd = parts[1]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("in_fd", parts[1]))?;
        let offset = super::buffer::parse(parts[2].trim())?;
        let count = parts[3]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("count", parts[3]))?;
        Ok(Self {
            out_fd,
            in_fd,
            offset,
            count,
            result: LenResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    fn unchanged(offset: u64) -> Buffer<Indirect<u64>> {
        Buffer::Decoded(Indirect::new(offset))
    }

    fn moved(offset: u64, to: u64) -> Buffer<Indirect<u64>> {
        Buffer::Decoded(Indirect::moved(offset, to))
    }

    #[test]
    fn display_success() {
        let call = Sendfile::from_raw(4, 3, unchanged(0), 100, 10);
        assert_eq!(call.to_string(), "sendfile(4, 3, [0], 100) = 10");
        assert_eq!(call.result, LenResult::Len(10));
    }

    #[test]
    fn display_moved_offset() {
        let call = Sendfile::from_raw(4, 3, moved(0, 10), 100, 10);
        assert_eq!(call.to_string(), "sendfile(4, 3, [0 => 10], 100) = 10");
    }

    #[test]
    fn display_null_offset() {
        let call = Sendfile::from_raw(4, 3, Buffer::Address(0), 100, 10);
        assert_eq!(call.to_string(), "sendfile(4, 3, NULL, 100) = 10");
    }

    #[test]
    fn display_failure() {
        let call = Sendfile::from_raw(4, 3, Buffer::Address(0x7fff1234), 100, -14);
        assert_eq!(
            call.to_string(),
            "sendfile(4, 3, 0x7fff1234, 100) = -1 EFAULT"
        );
        assert_eq!(call.result, LenResult::Errno(Errno::EFAULT));
    }

    #[test]
    fn parse_success() {
        let line = "sendfile(4, 3, [0 => 10], 100) = 10";
        let call = Sendfile::from_str(line).unwrap();
        assert_eq!(call.out_fd, Fd::new(4));
        assert_eq!(call.in_fd, Fd::new(3));
        assert_eq!(call.offset, moved(0, 10));
        assert_eq!(call.count, 100);
        assert_eq!(call.result, LenResult::Len(10));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_success_old_strace_format() {
        let call = Sendfile::from_str("sendfile(4, 3, [0] => [10], 100) = 10").unwrap();
        assert_eq!(call.out_fd, Fd::new(4));
        assert_eq!(call.in_fd, Fd::new(3));
        assert_eq!(call.offset, moved(0, 10));
        assert_eq!(call.count, 100);
        assert_eq!(call.result, LenResult::Len(10));
        assert_eq!(call.to_string(), "sendfile(4, 3, [0 => 10], 100) = 10");
    }

    #[test]
    fn parse_unchanged_offset() {
        let line = "sendfile(4, 3, [0], 100) = 10";
        let call = Sendfile::from_str(line).unwrap();
        assert_eq!(call.offset, unchanged(0));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_null_offset() {
        let line = "sendfile(4, 3, NULL, 100) = -1 EBADF";
        let call = Sendfile::from_str(line).unwrap();
        assert_eq!(call.offset, Buffer::<Indirect<u64>>::Address(0));
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Sendfile::from_str("sendfile(4, 3, [0], 100) 10").is_err());
        assert!(Sendfile::from_str("sendfile(4, 3, [0]) = 0").is_err());
        assert!(Sendfile::from_str("sendfile(x, 3, [0], 100) = 0").is_err());
        assert!(Sendfile::from_str("sendfile(4, 3, [0], 100, 1) = 0").is_err());
        assert!(Sendfile::from_str("sendfile64(4, 3, [0], 100) = 0").is_err());
        assert!(Sendfile::from_str("sendfile(4, 3, 5, 100) = 0").is_err());
        assert!(Sendfile::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "sendfile(4, 3, [0], 100) = 10",
            "sendfile(4, 3, [0 => 10], 100) = 10",
            "sendfile(1, 3, NULL, 100) = 100",
            "sendfile(4, 3, 0x7fff1234, 100) = -1 EFAULT",
            "sendfile(4, 3, [18446744073709551615], 100) = -1 EINVAL",
        ] {
            assert_eq!(Sendfile::from_str(line).unwrap().to_string(), line);
        }
    }
}
