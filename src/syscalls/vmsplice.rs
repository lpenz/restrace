// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`vmsplice(2)`] system call.
//!
//! [`vmsplice(2)`]: https://man7.org/linux/man-pages/man2/vmsplice.2.html

use super::buffer::Buffer;
use super::fd::Fd;
use super::iovec::IovecList;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use super::splice::SpliceFlags;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `vmsplice(2)` system call.
///
/// ```text
/// ssize_t vmsplice(int fd, const struct iovec *iov, unsigned long nr_segs,
///                  unsigned int flags);
/// ```
///
/// `strace` decodes the buffers on entry before the call executes, so even
/// on a short write or a failure the base buffers keep the bytes it printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vmsplice {
    /// File descriptor the call writes to, with the path `strace` resolved for
    /// it when run with `-y`.
    pub fd: Fd,
    /// The buffers the call wrote from, or the raw address of the iov array
    /// when `strace` could not read it.
    pub iov: Buffer<IovecList>,
    /// Number of entries in the iov array.
    pub nr_segs: u64,
    /// Flags, the `flags` argument; see [`SpliceFlags`](super::splice::SpliceFlags).
    pub flags: SpliceFlags,
    /// Return value: the number of bytes written, or the errno of the failure.
    pub result: LenResult,
}

impl Vmsplice {
    /// Builds a `vmsplice(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes written on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        fd: i32,
        iov: Buffer<IovecList>,
        nr_segs: u64,
        flags: SpliceFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            fd: Fd::new(fd),
            iov,
            nr_segs,
            flags,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Vmsplice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "vmsplice({}, {}, {}, {}) = {}",
            self.fd, self.iov, self.nr_segs, self.flags, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `vmsplice(3, [{iov_base="data", iov_len=4}], 1, 0) = 4` or
/// `vmsplice(3, 0xADDR, 1, SPLICE_F_GIFT) = -1 EFAULT`.
impl FromStr for Vmsplice {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("vmsplice call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("vmsplice(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("vmsplice arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[0]))?;
        let iov = super::buffer::parse(parts[1].trim())?;
        let nr_segs = parts[2]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("nr_segs", parts[2]))?;
        let flags = SpliceFlags::from_str(parts[3].trim())?;
        Ok(Self {
            fd,
            iov,
            nr_segs,
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

    #[test]
    fn display_success() {
        let call = Vmsplice::from_raw(3, list(vec![(b"hi", 2)]), 1, SpliceFlags(0), 2);
        assert_eq!(
            call.to_string(),
            "vmsplice(3, [{iov_base=\"hi\", iov_len=2}], 1, 0) = 2"
        );
        assert_eq!(call.result, LenResult::Len(2));
    }

    #[test]
    fn display_flags() {
        let call = Vmsplice::from_raw(3, list(vec![(b"x", 1)]), 1, SpliceFlags::SPLICE_F_GIFT, 1);
        assert_eq!(
            call.to_string(),
            "vmsplice(3, [{iov_base=\"x\", iov_len=1}], 1, SPLICE_F_GIFT) = 1"
        );
    }

    #[test]
    fn display_failure() {
        let call = Vmsplice::from_raw(3, Buffer::Address(0x1), 1, SpliceFlags(0), -14);
        assert_eq!(call.to_string(), "vmsplice(3, 0x1, 1, 0) = -1 EFAULT");
        assert_eq!(call.result, LenResult::Errno(Errno::EFAULT));
    }

    #[test]
    fn parse_success() {
        let line = "vmsplice(3, [{iov_base=\"data\", iov_len=4}], 1, SPLICE_F_MOVE) = 4";
        let call = Vmsplice::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.nr_segs, 1);
        assert_eq!(call.flags, SpliceFlags::SPLICE_F_MOVE);
        assert_eq!(call.result, LenResult::Len(4));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_failure() {
        let line = "vmsplice(3, 0x1, 1, 0) = -1 EFAULT";
        let call = Vmsplice::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.iov, Buffer::<IovecList>::Address(0x1));
        assert_eq!(call.nr_segs, 1);
        assert_eq!(call.flags, SpliceFlags(0));
        assert_eq!(call.result, LenResult::Errno(Errno::EFAULT));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Vmsplice::from_str("vmsplice(3, [{iov_base=\"x\", iov_len=1}]) = 0").is_err());
        assert!(Vmsplice::from_str("vmsplice(3, [{iov_base=\"x\", iov_len=1}], 1) = 0").is_err());
        assert!(
            Vmsplice::from_str("vmsplice(x, [{iov_base=\"x\", iov_len=1}], 1, 0) = 0").is_err()
        );
        assert!(
            Vmsplice::from_str("vmsplice(3, [{iov_base=\"x\", iov_len=1}], 1, 0, 0) = 0").is_err()
        );
        assert!(Vmsplice::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "vmsplice(3, [{iov_base=\"hi\", iov_len=2}], 1, 0) = 2",
            "vmsplice(3, [{iov_base=\"data\", iov_len=4}], 1, SPLICE_F_MOVE) = 4",
            "vmsplice(3, [{iov_base=\"x\", iov_len=1}], 2, SPLICE_F_MORE|SPLICE_F_GIFT) = -1 EAGAIN",
            "vmsplice(3</tmp/f>, [{iov_base=\"a\", iov_len=1}], 1, SPLICE_F_NONBLOCK) = -1 EBADF",
            "vmsplice(3, 0x1, 1, 0) = -1 EFAULT",
        ] {
            assert_eq!(Vmsplice::from_str(line).unwrap().to_string(), line);
        }
    }
}
