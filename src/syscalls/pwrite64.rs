// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`pwrite64(2)`] system call.
//!
//! [`pwrite64(2)`]: https://man7.org/linux/man-pages/man2/pwrite.2.html

use super::buffer::Buffer;
use super::bytes::Bytes;
use super::fd::Fd;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `pwrite64(2)` system call.
///
/// ```text
/// ssize_t pwrite64(int fd, const void *buf, size_t count, off_t offset);
/// ```
///
/// `strace` decodes `count` bytes of `buf` on entry, so unlike
/// [`pread64(2)`](super::pread64::Pread64) the buffer it prints keeps its
/// full size even on a short write, and it is decoded even when the call
/// fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pwrite64 {
    /// File descriptor the call wrote to, with the path `strace` resolved for
    /// it when run with `-y`.
    pub fd: Fd,
    /// The bytes the call wrote, or the raw address of the buffer when
    /// `strace` could not decode it, such as after a failure.
    pub buf: Buffer<Bytes>,
    /// Size of the buffer the bytes were written from.
    pub count: u64,
    /// Offset, in bytes, the call wrote at.
    pub offset: i64,
    /// Return value: the number of bytes written, or the errno of the failure.
    pub result: LenResult,
}

impl Pwrite64 {
    /// Builds a `pwrite64(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes written on success, or `-errno` on failure.
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

impl fmt::Display for Pwrite64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pwrite64({}, {}, {}, {}) = {}",
            self.fd, self.buf, self.count, self.offset, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `pwrite64(3, "data", 64, 1024) = 4` or `pwrite64(3, 0xADDR, 64, 1024) = -1 EFAULT`.
impl FromStr for Pwrite64 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("pwrite64 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("pwrite64(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("pwrite64 arguments", args));
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
        let call = Pwrite64::from_raw(1, decoded(b"hi\n"), 3, 1024, 3);
        assert_eq!(call.to_string(), "pwrite64(1, \"hi\\n\", 3, 1024) = 3");
        assert_eq!(call.result, LenResult::Len(3));
    }

    #[test]
    fn display_binary() {
        let call = Pwrite64::from_raw(3, decoded(&[0x01, b'5']), 2, 0, 2);
        assert_eq!(call.to_string(), "pwrite64(3, \"\\0015\", 2, 0) = 2");
    }

    #[test]
    fn display_empty() {
        let call = Pwrite64::from_raw(3, decoded(b""), 0, 5, 0);
        assert_eq!(call.to_string(), "pwrite64(3, \"\", 0, 5) = 0");
    }

    #[test]
    fn display_failure() {
        let call = Pwrite64::from_raw(3, Buffer::Address(0x7fff1234), 64, 512, -14);
        assert_eq!(
            call.to_string(),
            "pwrite64(3, 0x7fff1234, 64, 512) = -1 EFAULT"
        );
        assert_eq!(call.result, LenResult::Errno(Errno::EFAULT));
    }
}
