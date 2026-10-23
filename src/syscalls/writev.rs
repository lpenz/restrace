// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`writev(2)`] system call.
//!
//! [`writev(2)`]: https://man7.org/linux/man-pages/man2/writev.2.html

use super::buffer::Buffer;
use super::fd::Fd;
use super::iovec::IovecList;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `writev(2)` system call.
///
/// ```text
/// ssize_t writev(int fd, const struct iovec *iov, int iovcnt);
/// ```
///
/// `strace` decodes the buffers on entry before the call executes, so even
/// on a short write or a failure the base buffers keep the bytes it printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Writev {
    /// File descriptor the call wrote to, with the path `strace` resolved for
    /// it when run with `-y`.
    pub fd: Fd,
    /// The buffers the call wrote from, or the raw address of the iov array
    /// when `strace` could not read it.
    pub iov: Buffer<IovecList>,
    /// Number of entries in the iov array.
    pub iovcnt: u64,
    /// Return value: the number of bytes written, or the errno of the failure.
    pub result: LenResult,
}

impl Writev {
    /// Builds a `writev(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes written on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, iov: Buffer<IovecList>, iovcnt: u64, raw_result: i64) -> Self {
        Self {
            fd: Fd::new(fd),
            iov,
            iovcnt,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Writev {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "writev({}, {}, {}) = {}",
            self.fd, self.iov, self.iovcnt, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `writev(3, [{iov_base="data", iov_len=4}], 1) = 4` or
/// `writev(3, 0xADDR, 1) = -1 EFAULT`.
impl FromStr for Writev {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("writev call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("writev(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 3 {
            return Err(ParseError::new("writev arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[0]))?;
        let iov = super::buffer::parse(parts[1].trim())?;
        let iovcnt = parts[2]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("iovcnt", parts[2]))?;
        Ok(Self {
            fd,
            iov,
            iovcnt,
            result: LenResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::bytes::Bytes;
    use crate::syscalls::errno::Errno;
    use crate::syscalls::iovec::Iovec;

    fn decoded(value: &[u8]) -> Buffer<Bytes> {
        Buffer::Decoded(Bytes::new(value.to_vec()))
    }

    fn list(entries: Vec<(&[u8], u64)>) -> Buffer<IovecList> {
        Buffer::Decoded(IovecList(
            entries
                .into_iter()
                .map(|(value, len)| Iovec {
                    base: decoded(value),
                    len,
                })
                .collect(),
        ))
    }

    const SUCCESS: &str = "writev(3, [{iov_base=\"abcd\", iov_len=4}, \
                           {iov_base=\"efghij\", iov_len=16}, \
                           {iov_base=\"\", iov_len=8}], 3) = 10";

    #[test]
    fn display_success() {
        let call = Writev::from_raw(
            3,
            list(vec![(b"abcd", 4), (b"efghij", 16), (b"", 8)]),
            3,
            10,
        );
        assert_eq!(call.to_string(), SUCCESS);
        assert_eq!(call.result, LenResult::Len(10));
    }

    #[test]
    fn display_empty() {
        let call = Writev::from_raw(3, Buffer::Decoded(IovecList(Vec::new())), 0, 0);
        assert_eq!(call.to_string(), "writev(3, [], 0) = 0");
    }

    #[test]
    fn display_failure_addresses() {
        let call = Writev::from_raw(
            99,
            Buffer::Decoded(IovecList(vec![Iovec {
                base: Buffer::Address(0x7f8cc17fc210),
                len: 8,
            }])),
            1,
            -9,
        );
        assert_eq!(
            call.to_string(),
            "writev(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1) = -1 EBADF"
        );
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
    }

    #[test]
    fn display_unreadable_array() {
        let call = Writev::from_raw(3, Buffer::Address(0x1), 2, -14);
        assert_eq!(call.to_string(), "writev(3, 0x1, 2) = -1 EFAULT");
    }

    #[test]
    fn parse_success() {
        let call = Writev::from_str(SUCCESS).unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(
            call.iov,
            list(vec![(b"abcd", 4), (b"efghij", 16), (b"", 8)])
        );
        assert_eq!(call.iovcnt, 3);
        assert_eq!(call.result, LenResult::Len(10));
        assert_eq!(call.to_string(), SUCCESS);
    }

    #[test]
    fn parse_empty() {
        let call = Writev::from_str("writev(3, [], 0) = 0").unwrap();
        assert_eq!(call.iov, Buffer::Decoded(IovecList(Vec::new())));
        assert_eq!(call.to_string(), "writev(3, [], 0) = 0");
    }

    #[test]
    fn parse_failure() {
        let line = "writev(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1) = -1 EBADF \
                    (Bad file descriptor)";
        let call = Writev::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(99));
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
        assert_eq!(
            call.to_string(),
            "writev(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1) = -1 EBADF"
        );
    }

    #[test]
    fn parse_unreadable_array() {
        let line = "writev(3, 0x1, 2) = -1 EFAULT (Bad address)";
        let call = Writev::from_str(line).unwrap();
        assert_eq!(call.iov, Buffer::Address(0x1));
        assert_eq!(call.to_string(), "writev(3, 0x1, 2) = -1 EFAULT");
    }

    #[test]
    fn parse_invalid() {
        assert!(Writev::from_str("writev(3, [], 0) 0").is_err());
        assert!(Writev::from_str("writev(3, []) = 0").is_err());
        assert!(Writev::from_str("writev(x, [], 0) = 0").is_err());
        assert!(Writev::from_str("writev(3, [], 0, 1) = 0").is_err());
        assert!(Writev::from_str("write(3, \"a\", 1) = 1").is_err());
        assert!(Writev::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "writev(3, [], 0) = 0",
            "writev(3, [{iov_base=\"\", iov_len=8}], 1) = 0",
            "writev(3, [{iov_base=\"abcd\", iov_len=4}, {iov_base=\"efghij\", iov_len=16}], 2) = 10",
            "writev(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1) = -1 EBADF",
            "writev(3, 0x1, 2) = -1 EFAULT",
            "writev(3, [{iov_base=NULL, iov_len=0}], 1) = 0",
        ] {
            assert_eq!(Writev::from_str(line).unwrap().to_string(), line);
        }
    }
}
