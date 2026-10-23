// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`pread64(2)`] system call.
//!
//! [`pread64(2)`]: https://man7.org/linux/man-pages/man2/pread.2.html

use super::buffer::Buffer;
use super::bytes::Bytes;
use super::fd::Fd;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `pread64(2)` system call.
///
/// ```text
/// ssize_t pread64(int fd, void *buf, size_t count, off_t offset);
/// ```
///
/// Unlike [`read(2)`](super::read::Read), the call leaves the file offset of
/// `fd` unchanged; `offset` names the position the read starts at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pread64 {
    /// File descriptor the call read from, with the path `strace` resolved
    /// for it when run with `-y`.
    pub fd: Fd,
    /// The bytes the call read, or the raw address of the buffer when
    /// `strace` could not decode it, such as after a failure.
    pub buf: Buffer<Bytes>,
    /// Size of the buffer the bytes were read into.
    pub count: u64,
    /// Offset, in bytes, the call read from.
    pub offset: i64,
    /// Return value: the number of bytes read, or the errno of the failure.
    pub result: LenResult,
}

impl Pread64 {
    /// Builds a `pread64(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes read on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, buf: Buffer<Bytes>, count: u64, offset: i64, raw_result: i64) -> Self {
        Self {
            fd: Fd::new(fd),
            buf,
            count,
            offset,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Pread64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pread64({}, {}, {}, {}) = {}",
            self.fd, self.buf, self.count, self.offset, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `pread64(3, "data", 64, 1024) = 4` or `pread64(3, 0xADDR, 64, 1024) = -1 EBADF`.
impl FromStr for Pread64 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("pread64 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("pread64(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("pread64 arguments", args));
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
        let offset = parts[3]
            .trim()
            .parse::<i64>()
            .map_err(|_| ParseError::new("offset", parts[3]))?;
        Ok(Self {
            fd,
            buf,
            count,
            offset,
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
        let call = Pread64::from_raw(0, decoded(b"hi\n"), 131072, 1024, 3);
        assert_eq!(call.to_string(), "pread64(0, \"hi\\n\", 131072, 1024) = 3");
        assert_eq!(call.result, LenResult::Len(3));
    }

    #[test]
    fn display_empty() {
        let call = Pread64::from_raw(3, decoded(b""), 64, 0, 0);
        assert_eq!(call.to_string(), "pread64(3, \"\", 64, 0) = 0");
    }

    #[test]
    fn display_negative_offset() {
        let call = Pread64::from_raw(3, decoded(b""), 64, -1, -22);
        assert_eq!(call.to_string(), "pread64(3, \"\", 64, -1) = -1 EINVAL");
    }

    #[test]
    fn display_failure() {
        let call = Pread64::from_raw(3, Buffer::Address(0x7ffc1234), 64, 512, -9);
        assert_eq!(
            call.to_string(),
            "pread64(3, 0x7ffc1234, 64, 512) = -1 EBADF"
        );
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_success() {
        let line = "pread64(3, \"carb\", 4, 0) = 4";
        let call = Pread64::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.buf, decoded(b"carb"));
        assert_eq!(call.count, 4);
        assert_eq!(call.offset, 0);
        assert_eq!(call.result, LenResult::Len(4));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_failure() {
        let line = "pread64(3, 0x7ffc1234, 64, 512) = -1 EBADF";
        let call = Pread64::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.buf, Buffer::<Bytes>::Address(0x7ffc1234));
        assert_eq!(call.count, 64);
        assert_eq!(call.offset, 512);
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Pread64::from_str("pread64(3, \"a\", 64, 0) 0").is_err());
        assert!(Pread64::from_str("pread64(3, \"a\", 64) = 0").is_err());
        assert!(Pread64::from_str("pread64(3, \"a\", 64, 0, 1) = 0").is_err());
        assert!(Pread64::from_str("pread64(x, \"a\", 64, 0) = 0").is_err());
        assert!(Pread64::from_str("pread64(3, \"a\", x, 0) = 0").is_err());
        assert!(Pread64::from_str("pread64(3, \"a\", 64, x) = 0").is_err());
        assert!(Pread64::from_str("read(3, \"a\", 64) = 0").is_err());
        assert!(Pread64::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "pread64(3, \"carb\", 4, 0) = 4",
            "pread64(0, \"hi\\n\", 131072, 1024) = 3",
            "pread64(3, \"\", 64, 0) = 0",
            "pread64(3, \"a, b\", 5, 8) = 5",
            "pread64(3, 0x7ffc1234, 64, 512) = -1 EBADF",
            "pread64(3, \"\", 64, -1) = -1 EINVAL",
        ] {
            assert_eq!(Pread64::from_str(line).unwrap().to_string(), line);
        }
    }
}
