// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`getdents64(2)`] system call.
//!
//! [`getdents64(2)`]: https://man7.org/linux/man-pages/man2/getdents.2.html

use super::buffer::Buffer;
use super::dirent::{Dirent64, DirentList};
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `getdents64(2)` system call.
///
/// ```text
/// long getdents64(unsigned int fd, struct linux_dirent64 *dirp, unsigned int count);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Getdents64 {
    /// File descriptor of the directory to read.
    pub fd: i32,
    /// The entries the call read, or the raw address of the buffer when
    /// `strace` could not decode it, such as after a failure.
    pub buf: Buffer<DirentList<Dirent64>>,
    /// Size of the buffer the entries were read into.
    pub count: u64,
    /// Return value: the number of bytes read, or the errno of the failure.
    pub result: LenResult,
}

impl Getdents64 {
    /// Builds a `getdents64(2)` record from its arguments and raw return
    /// value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes read on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        fd: i32,
        buf: Buffer<DirentList<Dirent64>>,
        count: u64,
        raw_result: i64,
    ) -> Self {
        Self {
            fd,
            buf,
            count,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Getdents64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "getdents64({}, {}, {}) = {}",
            self.fd, self.buf, self.count, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `getdents64(3, [...], 4096) = 48` or `getdents64(-1, 0xADDR, 64) = -1 EBADF`.
impl FromStr for Getdents64 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("getdents64 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("getdents64(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 3 {
            return Err(ParseError::new("getdents64 arguments", args));
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
    use crate::syscalls::dirent::DirentType;
    use crate::syscalls::errno::Errno;
    use std::ffi::CString;

    fn entry(ino: u64, off: u64, name: &str, dtype: DirentType) -> Dirent64 {
        Dirent64 {
            ino,
            off,
            reclen: 24,
            dtype,
            name: CString::new(name).unwrap(),
        }
    }

    fn list() -> Buffer<DirentList<Dirent64>> {
        Buffer::Decoded(DirentList(vec![
            entry(54, 1, ".", DirentType::DIR),
            entry(6997, 2, "hostname", DirentType::REG),
        ]))
    }

    const SUCCESS: &str = "getdents64(3, [{d_ino=54, d_off=1, d_reclen=24, d_type=DT_DIR, \
                           d_name=\".\"}, {d_ino=6997, d_off=2, d_reclen=24, d_type=DT_REG, \
                           d_name=\"hostname\"}], 4096) = 48";

    #[test]
    fn display_success() {
        let call = Getdents64::from_raw(3, list(), 4096, 48);
        assert_eq!(call.to_string(), SUCCESS);
    }

    #[test]
    fn display_failure() {
        let call = Getdents64::from_raw(-1, Buffer::Address(0x7ffc1234), 64, -9);
        assert_eq!(
            call.to_string(),
            "getdents64(-1, 0x7ffc1234, 64) = -1 EBADF"
        );
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
    }

    #[test]
    fn display_empty() {
        let call = Getdents64::from_raw(3, Buffer::Decoded(DirentList(Vec::new())), 4096, 0);
        assert_eq!(call.to_string(), "getdents64(3, [], 4096) = 0");
    }

    #[test]
    fn parse_success() {
        let call = Getdents64::from_str(SUCCESS).unwrap();
        assert_eq!(call.fd, 3);
        assert_eq!(call.buf, list());
        assert_eq!(call.count, 4096);
        assert_eq!(call.result, LenResult::Len(48));
    }

    #[test]
    fn parse_failure() {
        let call = Getdents64::from_str("getdents64(-1, 0x7ffc1234, 64) = -1 EBADF").unwrap();
        assert_eq!(call.buf, Buffer::Address(0x7ffc1234));
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_invalid() {
        assert!(Getdents64::from_str("getdents64(3, []) = 0").is_err());
        assert!(Getdents64::from_str("getdents64(3, [], 4096, 1) = 0").is_err());
        assert!(Getdents64::from_str("getdents(3, [], 4096) = 0").is_err());
        assert!(Getdents64::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            SUCCESS,
            "getdents64(3, [], 4096) = 0",
            "getdents64(-1, 0x7ffc1234, 64) = -1 EBADF",
        ] {
            assert_eq!(Getdents64::from_str(line).unwrap().to_string(), line);
        }
    }
}
