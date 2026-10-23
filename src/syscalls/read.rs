// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`read(2)`] system call.
//!
//! [`read(2)`]: https://man7.org/linux/man-pages/man2/read.2.html

use super::buffer::Buffer;
use super::bytes::Bytes;
use super::fd::Fd;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `read(2)` system call.
///
/// ```text
/// ssize_t read(int fd, void *buf, size_t count);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// File descriptor the call read from, with the path `strace` resolved for
    /// it when run with `-y`.
    pub fd: Fd,
    /// The bytes the call read, or the raw address of the buffer when
    /// `strace` could not decode it, such as after a failure.
    pub buf: Buffer<Bytes>,
    /// Size of the buffer the bytes were read into.
    pub count: u64,
    /// Return value: the number of bytes read, or the errno of the failure.
    pub result: LenResult,
}

impl Read {
    /// Builds a `read(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes read on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, buf: Buffer<Bytes>, count: u64, raw_result: i64) -> Self {
        Self {
            fd: Fd::new(fd),
            buf,
            count,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Read {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "read({}, {}, {}) = {}",
            self.fd, self.buf, self.count, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `read(3, "data", 64) = 4` or `read(3, 0xADDR, 64) = -1 EBADF`.
impl FromStr for Read {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("read call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("read(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 3 {
            return Err(ParseError::new("read arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[0]))?;
        let buf = super::buffer::parse(parts[1].trim())?;
        let count = parts[2]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("count", parts[2]))?;
        Ok(Self {
            fd,
            buf,
            count,
            result: LenResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    fn decoded(value: &[u8]) -> Buffer<Bytes> {
        Buffer::Decoded(Bytes::new(value.to_vec()))
    }

    #[test]
    fn display_success() {
        let call = Read::from_raw(0, decoded(b"hi\n"), 131072, 3);
        assert_eq!(call.to_string(), "read(0, \"hi\\n\", 131072) = 3");
        assert_eq!(call.result, LenResult::Len(3));
    }

    #[test]
    fn display_binary() {
        let call = Read::from_raw(3, decoded(&[0x01, b'5']), 8, 2);
        assert_eq!(call.to_string(), "read(3, \"\\0015\", 8) = 2");
    }

    #[test]
    fn display_empty() {
        let call = Read::from_raw(3, decoded(b""), 64, 0);
        assert_eq!(call.to_string(), "read(3, \"\", 64) = 0");
    }

    #[test]
    fn display_failure() {
        let call = Read::from_raw(3, Buffer::Address(0x7ffc1234), 64, -9);
        assert_eq!(call.to_string(), "read(3, 0x7ffc1234, 64) = -1 EBADF");
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_success() {
        let line = "read(0, \"hi\\n\", 131072) = 3";
        let call = Read::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(0));
        assert_eq!(call.buf, decoded(b"hi\n"));
        assert_eq!(call.count, 131072);
        assert_eq!(call.result, LenResult::Len(3));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_failure() {
        let line = "read(3, 0x7ffc1234, 64) = -1 EBADF";
        let call = Read::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.buf, Buffer::<Bytes>::Address(0x7ffc1234));
        assert_eq!(call.count, 64);
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Read::from_str("read(3, \"a\", 64) 0").is_err());
        assert!(Read::from_str("read(3, \"a\") = 0").is_err());
        assert!(Read::from_str("read(x, \"a\", 64) = 0").is_err());
        assert!(Read::from_str("read(3, \"a\", 64, 1) = 0").is_err());
        assert!(Read::from_str("readlink(3, \"a\", 64) = 0").is_err());
        assert!(Read::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "read(0, \"hi\\n\", 131072) = 3",
            "read(3, \"\", 64) = 0",
            "read(3, \"\\0015\", 8) = 2",
            "read(3, \"\\377\", 1) = 1",
            "read(0, \"a, b\", 5) = 5",
            "read(3</tmp/in>, \"hi\\n\", 131072) = 3",
            "read(3, 0x7ffc1234, 64) = -1 EBADF",
        ] {
            assert_eq!(Read::from_str(line).unwrap().to_string(), line);
        }
    }
}
