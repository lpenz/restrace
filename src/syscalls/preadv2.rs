// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`preadv2(2)`] system call.
//!
//! [`preadv2(2)`]: https://man7.org/linux/man-pages/man2/preadv2.2.html

use super::buffer::Buffer;
use super::flags;
use super::iovec::IovecList;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Named bits of [`RwfFlags`], in [`Display`](fmt::Display) order.
const NAMED_FLAGS: &[(u32, &str)] = &[
    (RwfFlags::RWF_HIPRI.0, "RWF_HIPRI"),
    (RwfFlags::RWF_DSYNC.0, "RWF_DSYNC"),
    (RwfFlags::RWF_SYNC.0, "RWF_SYNC"),
    (RwfFlags::RWF_NOWAIT.0, "RWF_NOWAIT"),
    (RwfFlags::RWF_APPEND.0, "RWF_APPEND"),
    (RwfFlags::RWF_NOAPPEND.0, "RWF_NOAPPEND"),
    (RwfFlags::RWF_ATOMIC.0, "RWF_ATOMIC"),
    (RwfFlags::RWF_DONTCACHE.0, "RWF_DONTCACHE"),
    (RwfFlags::RWF_NOSIGNAL.0, "RWF_NOSIGNAL"),
];

/// Arguments and return value of the `preadv2(2)` system call.
///
/// ```text
/// ssize_t preadv2(int fd, const struct iovec *iov, int iovcnt, off_t offset,
///                 int flags);
/// ```
///
/// On a short read, `strace` prints only the bytes the call actually
/// returned, spread across the buffers in order, while every `iov_len` keeps
/// the size of the buffer it names; a buffer beyond that point is printed
/// with an empty base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preadv2 {
    /// File descriptor the call read from.
    pub fd: i32,
    /// The buffers the call read into, or the raw address of the iov array
    /// when `strace` could not read it. After a failure the bases are shown
    /// as addresses too, so the buffers carry no decoded bytes.
    pub iov: Buffer<IovecList>,
    /// Number of entries in the iov array.
    pub iovcnt: u64,
    /// Offset, in bytes, the call read from.
    pub offset: i64,
    /// Flags, the `flags` argument; see [`RwfFlags`].
    pub flags: RwfFlags,
    /// Return value: the number of bytes read, or the errno of the failure.
    pub result: LenResult,
}

impl Preadv2 {
    /// Builds a `preadv2(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes read on success, or `-errno` on failure.
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

impl fmt::Display for Preadv2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "preadv2({}, {}, {}, {}, {}) = {}",
            self.fd, self.iov, self.iovcnt, self.offset, self.flags, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `preadv2(3, [{iov_base="data", iov_len=4}], 1, 1024, RWF_NOWAIT) = 4` or
/// `preadv2(3, 0xADDR, 1, 1024, 0) = -1 EFAULT`.
impl FromStr for Preadv2 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("preadv2 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("preadv2(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 5 {
            return Err(ParseError::new("preadv2 arguments", args));
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

/// The `flags` argument of the `preadv2(2)` and `pwritev2(2)` system calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct RwfFlags(pub u32);

impl RwfFlags {
    /// High priority read or write.
    pub const RWF_HIPRI: Self = Self(0x01);
    /// Read or write with the equivalent of `O_DSYNC`.
    pub const RWF_DSYNC: Self = Self(0x02);
    /// Read or write with the equivalent of `O_SYNC`.
    pub const RWF_SYNC: Self = Self(0x04);
    /// Return `EAGAIN` if the read or write would block.
    pub const RWF_NOWAIT: Self = Self(0x08);
    /// Append to the file instead of writing at `offset`.
    pub const RWF_APPEND: Self = Self(0x10);
    /// Do not append, even if the file was opened with `O_APPEND`.
    pub const RWF_NOAPPEND: Self = Self(0x20);
    /// Write a single atomic block (requires `RWF_ATOMIC` support).
    pub const RWF_ATOMIC: Self = Self(0x40);
    /// Do not cache the result of the operation.
    pub const RWF_DONTCACHE: Self = Self(0x80);
    /// Do not send `SIGIO` for the completion of the operation.
    pub const RWF_NOSIGNAL: Self = Self(0x100);
}

impl fmt::Display for RwfFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_FLAGS, "RWF_???")
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them.
impl FromStr for RwfFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        flags::parse(s, NAMED_FLAGS, "flags").map(Self)
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
        let call = Preadv2::from_raw(
            3,
            list(vec![(b"abcd", 4), (b"", 8)]),
            2,
            1024,
            RwfFlags::RWF_NOWAIT,
            4,
        );
        assert_eq!(
            call.to_string(),
            "preadv2(3, [{iov_base=\"abcd\", iov_len=4}, {iov_base=\"\", iov_len=8}], 2, 1024, \
             RWF_NOWAIT) = 4"
        );
        assert_eq!(call.result, LenResult::Len(4));
    }

    #[test]
    fn display_zero_flags() {
        let call = Preadv2::from_raw(
            3,
            Buffer::Decoded(IovecList(Vec::new())),
            0,
            0,
            RwfFlags(0),
            0,
        );
        assert_eq!(call.to_string(), "preadv2(3, [], 0, 0, 0) = 0");
    }

    #[test]
    fn display_combined_flags() {
        let call = Preadv2::from_raw(
            3,
            Buffer::Decoded(IovecList(Vec::new())),
            0,
            0,
            RwfFlags(RwfFlags::RWF_HIPRI.0 | RwfFlags::RWF_DSYNC.0),
            0,
        );
        assert_eq!(
            call.to_string(),
            "preadv2(3, [], 0, 0, RWF_HIPRI|RWF_DSYNC) = 0"
        );
    }

    #[test]
    fn display_failure() {
        let call = Preadv2::from_raw(3, Buffer::Address(0x1), 2, 512, RwfFlags(0), -14);
        assert_eq!(call.to_string(), "preadv2(3, 0x1, 2, 512, 0) = -1 EFAULT");
        assert_eq!(call.result, LenResult::Errno(Errno::EFAULT));
    }

    #[test]
    fn parse_success() {
        let line = "preadv2(3, [{iov_base=\"carb\", iov_len=4}], 1, 0, RWF_NOWAIT) = 4";
        let call = Preadv2::from_str(line).unwrap();
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
        assert!(Preadv2::from_str("preadv2(3, [], 0, 0, 0) 0").is_err());
        assert!(Preadv2::from_str("preadv2(3, [], 0, 0) = 0").is_err());
        assert!(Preadv2::from_str("preadv2(x, [], 0, 0, 0) = 0").is_err());
        assert!(Preadv2::from_str("preadv2(3, [], 0, 0, 0, 1) = 0").is_err());
        assert!(Preadv2::from_str("preadv2(3, [], 0, 0, BOGUS) = 0").is_err());
        assert!(Preadv2::from_str("preadv(3, [], 0, 0) = 0").is_err());
        assert!(Preadv2::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "preadv2(3, [], 0, 0, 0) = 0",
            "preadv2(3, [{iov_base=\"\", iov_len=8}], 1, 1024, RWF_NOWAIT) = 0",
            "preadv2(3, [{iov_base=\"abcd\", iov_len=4}], 1, 0, RWF_HIPRI|RWF_DSYNC) = 4",
            "preadv2(99, [{iov_base=0x7f8cc17fc210, iov_len=8}], 1, 512, 0) = -1 EBADF",
            "preadv2(3, 0x1, 2, 0, 0) = -1 EFAULT",
            "preadv2(3, [], 0, 0, 0x2000 /* RWF_??? */) = 0",
        ] {
            assert_eq!(Preadv2::from_str(line).unwrap().to_string(), line);
        }
    }
}
