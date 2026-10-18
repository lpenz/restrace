// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`pwritev2(2)`] system call.
//!
//! [`pwritev2(2)`]: https://man7.org/linux/man-pages/man2/pwritev2.2.html

use super::buffer::Buffer;
use super::iovec::IovecList;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use super::preadv2::RwfFlags;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `pwritev2(2)` system call.
///
/// ```text
/// ssize_t pwritev2(int fd, const struct iovec *iov, int iovcnt, off_t offset,
///                  int flags);
/// ```
///
/// `strace` decodes the buffers on entry before the call executes, so even
/// on a short write or a failure the base buffers keep the bytes it printed;
/// only a base that cannot be read is shown as an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pwritev2 {
    /// File descriptor the call wrote to.
    pub fd: i32,
    /// The buffers the call wrote from, or the raw address of the iov array
    /// when `strace` could not read it.
    pub iov: Buffer<IovecList>,
    /// Number of entries in the iov array.
    pub iovcnt: u64,
    /// Offset, in bytes, the call wrote at.
    pub offset: i64,
    /// Flags, the `flags` argument; see [`RwfFlags`].
    pub flags: RwfFlags,
    /// Return value: the number of bytes written, or the errno of the failure.
    pub result: LenResult,
}

impl Pwritev2 {
    /// Builds a `pwritev2(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes written on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        fd: i32,
        iov: Buffer<IovecList>,
        iovcnt: u64,
        offset: i64,
        flags: RwfFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            fd,
            iov,
            iovcnt,
            offset,
            flags,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Pwritev2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pwritev2({}, {}, {}, {}, {}) = {}",
            self.fd, self.iov, self.iovcnt, self.offset, self.flags, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `pwritev2(3, [{iov_base="data", iov_len=4}], 1, 1024, RWF_NOWAIT) = 4` or
/// `pwritev2(3, 0xADDR, 1, 1024, 0) = -1 EFAULT`.
impl FromStr for Pwritev2 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("pwritev2 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("pwritev2(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 5 {
            return Err(ParseError::new("pwritev2 arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<i32>()
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
        let flags = RwfFlags::from_str(parts[4].trim())?;
        Ok(Self {
            fd,
            iov,
            iovcnt,
            offset,
            flags,
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

    #[test]
    fn display_success() {
        let call = Pwritev2::from_raw(
            3,
            list(vec![(b"abcd", 4), (b"", 8)]),
            2,
            1024,
            RwfFlags::RWF_DSYNC,
            4,
        );
        assert_eq!(
            call.to_string(),
            "pwritev2(3, [{iov_base=\"abcd\", iov_len=4}, {iov_base=\"\", iov_len=8}], 2, 1024, \
             RWF_DSYNC) = 4"
        );
        assert_eq!(call.result, LenResult::Len(4));
    }

    #[test]
    fn display_zero_flags() {
        let call = Pwritev2::from_raw(
            3,
            Buffer::Decoded(IovecList(Vec::new())),
            0,
            0,
            RwfFlags(0),
            0,
        );
        assert_eq!(call.to_string(), "pwritev2(3, [], 0, 0, 0) = 0");
    }

    #[test]
    fn display_combined_flags() {
        let call = Pwritev2::from_raw(
            3,
            Buffer::Decoded(IovecList(Vec::new())),
            0,
            0,
            RwfFlags(RwfFlags::RWF_HIPRI.0 | RwfFlags::RWF_DSYNC.0),
            0,
        );
        assert_eq!(
            call.to_string(),
            "pwritev2(3, [], 0, 0, RWF_HIPRI|RWF_DSYNC) = 0"
        );
    }

    #[test]
    fn display_failure() {
        let call = Pwritev2::from_raw(3, Buffer::Address(0x1), 2, 512, RwfFlags(0), -14);
        assert_eq!(call.to_string(), "pwritev2(3, 0x1, 2, 512, 0) = -1 EFAULT");
        assert_eq!(call.result, LenResult::Errno(Errno::EFAULT));
    }

    #[test]
    fn parse_success() {
        let line = "pwritev2(3, [{iov_base=\"carb\", iov_len=4}], 1, 0, RWF_NOWAIT) = 4";
        let call = Pwritev2::from_str(line).unwrap();
        assert_eq!(call.fd, 3);
        assert_eq!(call.iov, list(vec![(b"carb", 4)]));
        assert_eq!(call.iovcnt, 1);
        assert_eq!(call.offset, 0);
        assert_eq!(call.flags, RwfFlags::RWF_NOWAIT);
        assert_eq!(call.result, LenResult::Len(4));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Pwritev2::from_str("pwritev2(3, [], 0, 0, 0) 0").is_err());
        assert!(Pwritev2::from_str("pwritev2(3, [], 0, 0) = 0").is_err());
        assert!(Pwritev2::from_str("pwritev2(x, [], 0, 0, 0) = 0").is_err());
        assert!(Pwritev2::from_str("pwritev2(3, [], 0, 0, 0, 1) = 0").is_err());
        assert!(Pwritev2::from_str("pwritev2(3, [], 0, 0, BOGUS) = 0").is_err());
        assert!(Pwritev2::from_str("pwritev(3, [], 0, 0) = 0").is_err());
        assert!(Pwritev2::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "pwritev2(3, [], 0, 0, 0) = 0",
            "pwritev2(3, [{iov_base=\"\", iov_len=8}], 1, 1024, RWF_NOWAIT) = 0",
            "pwritev2(3, [{iov_base=\"abcd\", iov_len=4}], 1, 0, RWF_HIPRI|RWF_DSYNC) = 4",
            "pwritev2(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1, 512, 0) = -1 EBADF",
            "pwritev2(3, 0x1, 2, 0, 0) = -1 EFAULT",
            "pwritev2(3, [], 0, 0, 0x2000 /* RWF_??? */) = 0",
        ] {
            assert_eq!(Pwritev2::from_str(line).unwrap().to_string(), line);
        }
    }

    /// The lines `strace` itself printed for real calls: a decoded write with
    /// a single named flag, and one with the zero flags the kernel reports
    /// when the caller passes none. The paired display is what the record
    /// writes back.
    #[test]
    fn parses_strace_output() {
        for (input, display) in [
            (
                "pwritev2(3, [{iov_base=\"abcd\", iov_len=4}, {iov_base=\"efghij\", iov_len=16}], \
                 2, 0, RWF_DSYNC) = 20",
                "pwritev2(3, [{iov_base=\"abcd\", iov_len=4}, {iov_base=\"efghij\", iov_len=16}], \
                 2, 0, RWF_DSYNC) = 20",
            ),
            (
                "pwritev2(3, [{iov_base=\"ab\", iov_len=2}, {iov_base=\"cd\", iov_len=2}], 2, 0, \
                 RWF_NOSIGNAL) = 4",
                "pwritev2(3, [{iov_base=\"ab\", iov_len=2}, {iov_base=\"cd\", iov_len=2}], 2, 0, \
                 RWF_NOSIGNAL) = 4",
            ),
            (
                "pwritev2(3, [{iov_base=\"hi\\n\", iov_len=3}], 1, 0, 0) = 3",
                "pwritev2(3, [{iov_base=\"hi\\n\", iov_len=3}], 1, 0, 0) = 3",
            ),
        ] {
            let call = Pwritev2::from_str(input).unwrap();
            assert_eq!(call.to_string(), display, "for the input line {input:?}");
        }
    }
}
