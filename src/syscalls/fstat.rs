// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fstat(2)`] system call.
//!
//! [`fstat(2)`]: https://man7.org/linux/man-pages/man2/fstat.2.html

use super::buffer::Buffer;
use super::fd::Fd;
use super::parse::{self, ParseError};
use super::statbuf::StatBuf;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `fstat(2)` system call.
///
/// ```text
/// int fstat(int fd, struct stat *restrict statbuf);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fstat {
    /// File descriptor to stat, with the path `strace` resolved for it when
    /// run with `-y`.
    pub fd: Fd,
    /// The `struct stat` the call filled in, or the raw address of the buffer
    /// when `strace` could not decode it, such as after a failure.
    pub buf: Buffer<StatBuf>,
    /// Return value: zero on success, or the errno of the failure.
    pub result: super::result::OkResult,
}

impl Fstat {
    /// Builds an `fstat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(fd: i32, buf: Buffer<StatBuf>, raw_result: i64) -> Self {
        Self {
            fd: Fd::new(fd),
            buf,
            result: super::result::OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fstat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fstat({}, {}) = {}", self.fd, self.buf, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fstat(3, {st_dev=..., ...}) = 0` or `fstat(-1, 0xADDR) = -1 EBADF`.
impl FromStr for Fstat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fstat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("fstat(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 2 {
            return Err(ParseError::new("fstat arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[0]))?;
        let buf = super::buffer::parse(parts[1].trim())?;
        let result = super::result::OkResult::from_str(result)?;
        Ok(Self { fd, buf, result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::device::Device;
    use crate::syscalls::filemode::FileMode;
    use crate::syscalls::result::OkResult;

    fn buf() -> StatBuf {
        StatBuf {
            dev: Device::new(0, 0x1e),
            ino: 6997,
            mode: FileMode(0o100644),
            nlink: 1,
            uid: 0,
            gid: 0,
            blksize: 4096,
            blocks: 8,
            rdev: None,
            size: Some(8),
            atime: 1754766014,
            atime_nsec: 986788270,
            mtime: 1545480610,
            mtime_nsec: 541614343,
            ctime: 1754766014,
            ctime_nsec: 986788270,
        }
    }

    #[test]
    fn display_success() {
        let fstat = Fstat::from_raw(3, Buffer::Decoded(buf()), 0);
        assert_eq!(
            fstat.to_string(),
            "fstat(3, {st_dev=makedev(0, 0x1e), st_ino=6997, \
             st_mode=S_IFREG|0644, st_nlink=1, st_uid=0, st_gid=0, st_blksize=4096, \
             st_blocks=8, st_size=8, st_atime=1754766014, st_atime_nsec=986788270, \
             st_mtime=1545480610, st_mtime_nsec=541614343, st_ctime=1754766014, \
             st_ctime_nsec=986788270}) = 0"
        );
    }

    #[test]
    fn display_failure() {
        let fstat = Fstat::from_raw(-1, Buffer::Address(0x7ffc1234), -9);
        assert_eq!(fstat.to_string(), "fstat(-1, 0x7ffc1234) = -1 EBADF");
    }

    #[test]
    fn parse_success() {
        let fstat = Fstat::from_str(
            "fstat(3, {st_dev=makedev(0, 0x1e), st_ino=6997, \
             st_mode=S_IFREG|0644, st_nlink=1, st_uid=0, st_gid=0, st_blksize=4096, \
             st_blocks=8, st_size=8, st_atime=1754766014, st_atime_nsec=986788270, \
             st_mtime=1545480610, st_mtime_nsec=541614343, st_ctime=1754766014, \
             st_ctime_nsec=986788270}) = 0",
        )
        .unwrap();
        assert_eq!(fstat.fd, Fd::new(3));
        assert_eq!(fstat.result, OkResult::Ok);
        assert!(matches!(fstat.buf, Buffer::Decoded(_)));
    }

    #[test]
    fn parse_invalid() {
        assert!(Fstat::from_str("fstat(3) = 0").is_err());
        assert!(Fstat::from_str("fstat(3, 0x1, 0x2) = 0").is_err());
        assert!(Fstat::from_str("stat(3, 0x1) = 0").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fstat(3, {st_dev=makedev(0, 0x1e), st_ino=6997, \
             st_mode=S_IFREG|0644, st_nlink=1, st_uid=0, st_gid=0, st_blksize=4096, \
             st_blocks=8, st_size=8, st_atime=1754766014, st_atime_nsec=986788270, \
             st_mtime=1545480610, st_mtime_nsec=541614343, st_ctime=1754766014, \
             st_ctime_nsec=986788270}) = 0",
            "fstat(-1, 0x7ffd684fe7b0) = -1 EBADF",
            "fstat(1, NULL) = -1 EACCES",
        ] {
            assert_eq!(Fstat::from_str(line).unwrap().to_string(), line);
        }
    }
}
