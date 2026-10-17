// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`write(2)`] system call.
//!
//! [`write(2)`]: https://man7.org/linux/man-pages/man2/write.2.html

use super::buffer::Buffer;
use super::bytes::Bytes;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `write(2)` system call.
///
/// ```text
/// ssize_t write(int fd, const void *buf, size_t count);
/// ```
///
/// `strace` decodes `count` bytes of `buf` on entry, so unlike
/// [`read(2)`](super::read::Read) the buffer it prints keeps its full size even
/// on a short write, and it is decoded even when the call fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Write {
    /// File descriptor the call wrote to.
    pub fd: i32,
    /// The bytes the call wrote, or the raw address of the buffer when
    /// `strace` could not decode it, such as after a failure.
    pub buf: Buffer<Bytes>,
    /// Size of the buffer the bytes were written from.
    pub count: u64,
    /// Return value: the number of bytes written, or the errno of the failure.
    pub result: LenResult,
}

impl Write {
    /// Builds a `write(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes written on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, buf: Buffer<Bytes>, count: u64, raw_result: i64) -> Self {
        Self {
            fd,
            buf,
            count,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Write {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "write({}, {}, {}) = {}",
            self.fd, self.buf, self.count, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `write(3, "data", 64) = 4` or `write(3, 0xADDR, 64) = -1 EFAULT`.
impl FromStr for Write {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("write call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("write(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 3 {
            return Err(ParseError::new("write arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<i32>()
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
        Buffer::Decoded(Bytes(value.to_vec()))
    }

    #[test]
    fn display_success() {
        let call = Write::from_raw(1, decoded(b"hi\n"), 3, 3);
        assert_eq!(call.to_string(), "write(1, \"hi\\n\", 3) = 3");
        assert_eq!(call.result, LenResult::Len(3));
    }

    #[test]
    fn display_binary() {
        let call = Write::from_raw(3, decoded(&[0x01, b'5']), 2, 2);
        assert_eq!(call.to_string(), "write(3, \"\\0015\", 2) = 2");
    }

    #[test]
    fn display_empty() {
        let call = Write::from_raw(3, decoded(b""), 0, 0);
        assert_eq!(call.to_string(), "write(3, \"\", 0) = 0");
    }

    #[test]
    fn display_failure() {
        let call = Write::from_raw(3, Buffer::Address(0x7fff1234), 64, -14);
        assert_eq!(call.to_string(), "write(3, 0x7fff1234, 64) = -1 EFAULT");
        assert_eq!(call.result, LenResult::Errno(Errno::EFAULT));
    }

    #[test]
    fn parse_success() {
        let line = "write(1, \"hi\\n\", 3) = 3";
        let call = Write::from_str(line).unwrap();
        assert_eq!(call.fd, 1);
        assert_eq!(call.buf, decoded(b"hi\n"));
        assert_eq!(call.count, 3);
        assert_eq!(call.result, LenResult::Len(3));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_failure() {
        let line = "write(3, 0x7fff1234, 64) = -1 EFAULT";
        let call = Write::from_str(line).unwrap();
        assert_eq!(call.fd, 3);
        assert_eq!(call.buf, Buffer::<Bytes>::Address(0x7fff1234));
        assert_eq!(call.count, 64);
        assert_eq!(call.result, LenResult::Errno(Errno::EFAULT));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Write::from_str("write(3, \"a\", 64) 0").is_err());
        assert!(Write::from_str("write(3, \"a\") = 0").is_err());
        assert!(Write::from_str("write(x, \"a\", 64) = 0").is_err());
        assert!(Write::from_str("write(3, \"a\", 64, 1) = 0").is_err());
        assert!(Write::from_str("writev(3, \"a\", 64) = 0").is_err());
        assert!(Write::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "write(1, \"hi\\n\", 3) = 3",
            "write(3, \"\", 0) = 0",
            "write(3, \"\\0015\", 2) = 2",
            "write(3, \"\\377\", 1) = 1",
            "write(1, \"a, b\", 5) = 5",
            "write(3, 0x7fff1234, 64) = -1 EFAULT",
        ] {
            assert_eq!(Write::from_str(line).unwrap().to_string(), line);
        }
    }
}
