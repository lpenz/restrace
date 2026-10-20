// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`preadv(2)`] system call.
//!
//! [`preadv(2)`]: https://man7.org/linux/man-pages/man2/preadv.2.html

use super::buffer::Buffer;
use super::fd::Fd;
use super::iovec::IovecList;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `preadv(2)` system call.
///
/// ```text
/// ssize_t preadv(int fd, const struct iovec *iov, int iovcnt, off_t offset);
/// ```
///
/// On a short read, `strace` prints only the bytes the call actually
/// returned, spread across the buffers in order, while every `iov_len` keeps
/// the size of the buffer it names; a buffer beyond that point is printed
/// with an empty base. Unlike [`readv(2)`](super::readv::Readv), the call
/// leaves the file offset of `fd` unchanged; `offset` names the position the
/// read starts at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preadv {
    /// File descriptor the call read from, with the path `strace` resolved
    /// for it when run with `-y`.
    pub fd: Fd,
    /// The buffers the call read into, or the raw address of the iov array
    /// when `strace` could not read it. After a failure the bases are shown
    /// as addresses too, so the buffers carry no decoded bytes.
    pub iov: Buffer<IovecList>,
    /// Number of entries in the iov array.
    pub iovcnt: u64,
    /// Offset, in bytes, the call read from.
    pub offset: i64,
    /// Return value: the number of bytes read, or the errno of the failure.
    pub result: LenResult,
}

impl Preadv {
    /// Builds a `preadv(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes read on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        fd: i32,
        iov: Buffer<IovecList>,
        iovcnt: u64,
        offset: i64,
        raw_result: i64,
    ) -> Self {
        Self {
            fd: Fd::new(fd),
            iov,
            iovcnt,
            offset,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Preadv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "preadv({}, {}, {}, {}) = {}",
            self.fd, self.iov, self.iovcnt, self.offset, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `preadv(3, [{iov_base="data", iov_len=4}], 1, 1024) = 4` or
/// `preadv(3, 0xADDR, 1, 1024) = -1 EFAULT`.
impl FromStr for Preadv {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("preadv call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("preadv(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("preadv arguments", args));
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
        let offset = parts[3]
            .trim()
            .parse::<i64>()
            .map_err(|_| ParseError::new("offset", parts[3]))?;
        Ok(Self {
            fd,
            iov,
            iovcnt,
            offset,
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
        Buffer::Decoded(Bytes(value.to_vec()))
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

    const SUCCESS: &str = "preadv(3, [{iov_base=\"abcd\", iov_len=4}, \
                           {iov_base=\"efghij\", iov_len=16}, \
                           {iov_base=\"\", iov_len=8}], 3, 1024) = 10";

    #[test]
    fn display_success() {
        let call = Preadv::from_raw(
            3,
            list(vec![(b"abcd", 4), (b"efghij", 16), (b"", 8)]),
            3,
            1024,
            10,
        );
        assert_eq!(call.to_string(), SUCCESS);
        assert_eq!(call.result, LenResult::Len(10));
    }

    #[test]
    fn display_empty() {
        let call = Preadv::from_raw(3, Buffer::Decoded(IovecList(Vec::new())), 0, 0, 0);
        assert_eq!(call.to_string(), "preadv(3, [], 0, 0) = 0");
    }

    #[test]
    fn display_failure_addresses() {
        let call = Preadv::from_raw(
            99,
            Buffer::Decoded(IovecList(vec![Iovec {
                base: Buffer::Address(0x7f8cc17fc210),
                len: 8,
            }])),
            1,
            512,
            -9,
        );
        assert_eq!(
            call.to_string(),
            "preadv(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1, 512) = -1 EBADF"
        );
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
    }

    #[test]
    fn display_unreadable_array() {
        let call = Preadv::from_raw(3, Buffer::Address(0x1), 2, 0, -14);
        assert_eq!(call.to_string(), "preadv(3, 0x1, 2, 0) = -1 EFAULT");
    }

    #[test]
    fn parse_success() {
        let call = Preadv::from_str(SUCCESS).unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(
            call.iov,
            list(vec![(b"abcd", 4), (b"efghij", 16), (b"", 8)])
        );
        assert_eq!(call.iovcnt, 3);
        assert_eq!(call.offset, 1024);
        assert_eq!(call.result, LenResult::Len(10));
        assert_eq!(call.to_string(), SUCCESS);
    }

    #[test]
    fn parse_empty() {
        let call = Preadv::from_str("preadv(3, [], 0, 0) = 0").unwrap();
        assert_eq!(call.iov, Buffer::Decoded(IovecList(Vec::new())));
        assert_eq!(call.to_string(), "preadv(3, [], 0, 0) = 0");
    }

    #[test]
    fn parse_failure() {
        let line = "preadv(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1, 512) = -1 EBADF \
                    (Bad file descriptor)";
        let call = Preadv::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(99));
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
        assert_eq!(
            call.to_string(),
            "preadv(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1, 512) = -1 EBADF"
        );
    }

    #[test]
    fn parse_unreadable_array() {
        let line = "preadv(3, 0x1, 2, 0) = -1 EFAULT (Bad address)";
        let call = Preadv::from_str(line).unwrap();
        assert_eq!(call.iov, Buffer::Address(0x1));
        assert_eq!(call.to_string(), "preadv(3, 0x1, 2, 0) = -1 EFAULT");
    }

    #[test]
    fn parse_invalid() {
        assert!(Preadv::from_str("preadv(3, [], 0, 0) 0").is_err());
        assert!(Preadv::from_str("preadv(3, [], 0) = 0").is_err());
        assert!(Preadv::from_str("preadv(x, [], 0, 0) = 0").is_err());
        assert!(Preadv::from_str("preadv(3, [], 0, 0, 1) = 0").is_err());
        assert!(Preadv::from_str("readv(3, \"a\", 1, 0) = 1").is_err());
        assert!(Preadv::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "preadv(3, [], 0, 0) = 0",
            "preadv(3, [{iov_base=\"\", iov_len=8}], 1, 1024) = 0",
            "preadv(3, [{iov_base=\"abcd\", iov_len=4}, {iov_base=\"efghij\", iov_len=16}], 2, 0) \
             = 10",
            "preadv(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1, 512) = -1 EBADF",
            "preadv(3, 0x1, 2, 0) = -1 EFAULT",
            "preadv(3, [{iov_base=NULL, iov_len=0}], 1, 0) = 0",
        ] {
            assert_eq!(Preadv::from_str(line).unwrap().to_string(), line);
        }
    }

    /// The lines `strace` itself printed for real calls, in every form it
    /// produces: a decoded short read that truncates the buffer it shows, a
    /// failure whose iov array is readable but whose bases cannot be decoded,
    /// a failure whose iov array itself is unreadable, and the empty array.
    /// The paired display is what the record writes back with the parenthesized
    /// errno description stripped.
    #[test]
    fn parses_strace_output() {
        for (input, display) in [
            (
                "preadv(3, [{iov_base=\"carb\", iov_len=4}, {iov_base=\"onl\\n\", iov_len=16}], \
                 2, 0) = 8",
                "preadv(3, [{iov_base=\"carb\", iov_len=4}, {iov_base=\"onl\\n\", iov_len=16}], \
                 2, 0) = 8",
            ),
            (
                "preadv(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1, 512) = -1 EBADF (Bad file \
                 descriptor)",
                "preadv(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1, 512) = -1 EBADF",
            ),
            (
                "preadv(3, [], 0, 0) = -1 EBADF (Bad file descriptor)",
                "preadv(3, [], 0, 0) = -1 EBADF",
            ),
            (
                "preadv(3, [{iov_base=NULL, iov_len=0}, {iov_base=NULL, iov_len=4}], 2, 0) = -1 \
                 EFAULT (Bad address)",
                "preadv(3, [{iov_base=NULL, iov_len=0}, {iov_base=NULL, iov_len=4}], 2, 0) = -1 \
                 EFAULT",
            ),
            (
                "preadv(3, 0x1, 2, 0) = -1 EFAULT (Bad address)",
                "preadv(3, 0x1, 2, 0) = -1 EFAULT",
            ),
            (
                "preadv(3, [{iov_base=\"abcd\", iov_len=4}, {iov_base=\"efghij\", iov_len=16}, \
                 {iov_base=\"\", iov_len=8}], 3, 1024) = 10",
                "preadv(3, [{iov_base=\"abcd\", iov_len=4}, {iov_base=\"efghij\", iov_len=16}, \
                 {iov_base=\"\", iov_len=8}], 3, 1024) = 10",
            ),
        ] {
            let call = Preadv::from_str(input).unwrap();
            assert_eq!(call.to_string(), display, "for the input line {input:?}");
        }
    }
}
