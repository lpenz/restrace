// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fallocate(2)`] system call.
//!
//! [`fallocate(2)`]: https://man7.org/linux/man-pages/man2/fallocate.2.html

use super::fd::Fd;
use super::flags;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::fmt;
use std::str::FromStr;

/// Named bits of [`FallocateMode`], in [`Display`](fmt::Display) order.
const NAMED_MODES: &[(u32, &str)] = &[
    (FallocateMode::KEEP_SIZE.0, "FALLOC_FL_KEEP_SIZE"),
    (FallocateMode::PUNCH_HOLE.0, "FALLOC_FL_PUNCH_HOLE"),
    (FallocateMode::NO_HIDE_STALE.0, "FALLOC_FL_NO_HIDE_STALE"),
    (FallocateMode::COLLAPSE_RANGE.0, "FALLOC_FL_COLLAPSE_RANGE"),
    (FallocateMode::ZERO_RANGE.0, "FALLOC_FL_ZERO_RANGE"),
    (FallocateMode::INSERT_RANGE.0, "FALLOC_FL_INSERT_RANGE"),
    (FallocateMode::UNSHARE_RANGE.0, "FALLOC_FL_UNSHARE_RANGE"),
    (FallocateMode::WRITE_ZEROES.0, "FALLOC_FL_WRITE_ZEROES"),
];

/// Arguments and return value of the `fallocate(2)` system call.
///
/// ```text
/// int fallocate(int fd, int mode, off_t offset, off_t len);
/// ```
///
/// `strace` prints `offset` and `len` as signed values, so a negative one
/// shows up with its sign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fallocate {
    /// File descriptor of the file to allocate space in, with the path
    /// `strace` resolved for it when run with `-y`.
    pub fd: Fd,
    /// Flags, the `mode` argument; see [`FallocateMode`].
    pub mode: FallocateMode,
    /// Offset of the range, in bytes.
    pub offset: i64,
    /// Length of the range, in bytes.
    pub len: i64,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Fallocate {
    /// Builds a `fallocate(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, mode: FallocateMode, offset: i64, len: i64, raw_result: i64) -> Self {
        Self {
            fd: Fd::new(fd),
            mode,
            offset,
            len,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fallocate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fallocate({}, {}, {}, {}) = {}",
            self.fd, self.mode, self.offset, self.len, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fallocate(3, 0, 0, 4096) = 0` or
/// `fallocate(3, FALLOC_FL_KEEP_SIZE, 0, 4096) = -1 EINVAL`.
impl FromStr for Fallocate {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fallocate call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("fallocate(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("fallocate arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[0]))?;
        let mode = FallocateMode::from_str(parts[1].trim())?;
        let offset = parts[2]
            .trim()
            .parse::<i64>()
            .map_err(|_| ParseError::new("offset", parts[2]))?;
        let len = parts[3]
            .trim()
            .parse::<i64>()
            .map_err(|_| ParseError::new("len", parts[3]))?;
        Ok(Self {
            fd,
            mode,
            offset,
            len,
            result: OkResult::from_str(result)?,
        })
    }
}

/// The `mode` argument of the [`fallocate(2)`] system call.
///
/// [`fallocate(2)`]: https://man7.org/linux/man-pages/man2/fallocate.2.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct FallocateMode(pub u32);

impl FallocateMode {
    /// Keep the file size unchanged.
    pub const KEEP_SIZE: Self = Self(0x01);
    /// Deallocate a range, leaving a hole.
    pub const PUNCH_HOLE: Self = Self(0x02);
    /// Do not hide stale data in the deallocated range.
    pub const NO_HIDE_STALE: Self = Self(0x04);
    /// Collapse a range, shifting the data that follows it.
    pub const COLLAPSE_RANGE: Self = Self(0x08);
    /// Zero a range without deallocating it.
    pub const ZERO_RANGE: Self = Self(0x10);
    /// Insert a hole, shifting the data that follows it.
    pub const INSERT_RANGE: Self = Self(0x20);
    /// Unshare a range that was shared by reflinking.
    pub const UNSHARE_RANGE: Self = Self(0x40);
    /// Convert the range to written extents of zeroes.
    pub const WRITE_ZEROES: Self = Self(0x80);
}

impl fmt::Display for FallocateMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_MODES, "FALLOC_FL_???")
    }
}

impl FromStr for FallocateMode {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        flags::parse(s, NAMED_MODES, "mode").map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_success() {
        let call = Fallocate::from_raw(3, FallocateMode(0), 0, 4096, 0);
        assert_eq!(call.to_string(), "fallocate(3, 0, 0, 4096) = 0");
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_flags() {
        let call = Fallocate::from_raw(3, FallocateMode::KEEP_SIZE, -5, 0, -22);
        assert_eq!(
            call.to_string(),
            "fallocate(3, FALLOC_FL_KEEP_SIZE, -5, 0) = -1 EINVAL"
        );
        assert_eq!(call.result, OkResult::Errno(Errno::EINVAL));
    }

    #[test]
    fn display_unknown_flags() {
        let call = Fallocate::from_raw(3, FallocateMode(0x200), 0, 4096, -9);
        assert_eq!(
            call.to_string(),
            "fallocate(3, 0x200 /* FALLOC_FL_??? */, 0, 4096) = -1 EBADF"
        );
        let call = Fallocate::from_raw(3, FallocateMode(0x101), 0, 4096, 0);
        assert_eq!(
            call.to_string(),
            "fallocate(3, FALLOC_FL_KEEP_SIZE|0x100, 0, 4096) = 0"
        );
    }

    #[test]
    fn parse_success() {
        let line = "fallocate(3, FALLOC_FL_KEEP_SIZE|FALLOC_FL_PUNCH_HOLE, 0, 4096) = 0";
        let call = Fallocate::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(
            call.mode,
            FallocateMode(FallocateMode::KEEP_SIZE.0 | FallocateMode::PUNCH_HOLE.0)
        );
        assert_eq!(call.offset, 0);
        assert_eq!(call.len, 4096);
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_failure() {
        let line = "fallocate(999, 0, -5, -6) = -1 EBADF";
        let call = Fallocate::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(999));
        assert_eq!(call.mode, FallocateMode(0));
        assert_eq!(call.offset, -5);
        assert_eq!(call.len, -6);
        assert_eq!(call.result, OkResult::Errno(Errno::EBADF));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_annotated_fd() {
        let line = "fallocate(3</tmp/file>, 0, 0, 4096) = 0";
        let call = Fallocate::from_str(line).unwrap();
        assert_eq!(call.fd.fd(), 3);
        assert_eq!(
            call.fd.path().map(|p| p.to_str().unwrap()),
            Some("/tmp/file")
        );
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Fallocate::from_str("fallocate(3, 0, 0, 4096) 0").is_err());
        assert!(Fallocate::from_str("fallocate(3, 0, 0) = 0").is_err());
        assert!(Fallocate::from_str("fallocate(3, 0, 0, 4096, 1) = 0").is_err());
        assert!(Fallocate::from_str("fallocate(x, 0, 0, 4096) = 0").is_err());
        assert!(Fallocate::from_str("fallocate(3, BOGUS, 0, 4096) = 0").is_err());
        assert!(Fallocate::from_str("fallocate(3, 0, x, 4096) = 0").is_err());
        assert!(Fallocate::from_str("fallocate(3, 0, 0, x) = 0").is_err());
        assert!(Fallocate::from_str("fallocate(3, 0, 0, 4096) = 1").is_err());
        assert!(Fallocate::from_str("ftruncate(3, 0, 0, 4096) = 0").is_err());
        assert!(Fallocate::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fallocate(3, 0, 0, 4096) = 0",
            "fallocate(3, FALLOC_FL_KEEP_SIZE, -5, 0) = -1 EINVAL",
            "fallocate(3, FALLOC_FL_KEEP_SIZE|FALLOC_FL_PUNCH_HOLE, 1, 0) = -1 EINVAL",
            "fallocate(3</tmp/file>, 0, 0, 4096) = 0",
            "fallocate(4, FALLOC_FL_KEEP_SIZE|FALLOC_FL_PUNCH_HOLE|FALLOC_FL_NO_HIDE_STALE|FALLOC_FL_COLLAPSE_RANGE|FALLOC_FL_ZERO_RANGE|FALLOC_FL_INSERT_RANGE|FALLOC_FL_UNSHARE_RANGE|FALLOC_FL_WRITE_ZEROES|0xffffff00, -5, -6) = -1 EINVAL",
            "fallocate(999, 0x200 /* FALLOC_FL_??? */, 0, 4096) = -1 EBADF",
        ] {
            assert_eq!(Fallocate::from_str(line).unwrap().to_string(), line);
        }
    }
}
