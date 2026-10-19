// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`copy_file_range(2)`] system call.
//!
//! [`copy_file_range(2)`]: https://man7.org/linux/man-pages/man2/copy_file_range.2.html

use super::buffer::Buffer;
use super::indirect::Indirect;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `copy_file_range(2)` system call.
///
/// ```text
/// ssize_t copy_file_range(int fd_in, loff_t *off_in, int fd_out,
///                         loff_t *off_out, size_t len, unsigned int flags);
/// ```
///
/// Copies a range of data from `fd_in` to `fd_out`. If `off_in` is `NULL`,
/// the file offset of `fd_in` is used and advanced; if not, the offset is
/// read from and written back to `*off_in`. Similarly for `off_out`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyFileRange {
    /// File descriptor to copy data from.
    pub fd_in: i32,
    /// Input file offset pointer, printed as `NULL`, an address, or
    /// `[value]`/`[value => new]` when decoded.
    pub off_in: Buffer<Indirect<i64>>,
    /// File descriptor to copy data to.
    pub fd_out: i32,
    /// Output file offset pointer, printed as for `off_in`.
    pub off_out: Buffer<Indirect<i64>>,
    /// Number of bytes to copy.
    pub len: u64,
    /// Flags that modify the behavior of the call.
    pub flags: u64,
    /// Return value: the number of bytes copied, or the errno of the failure.
    pub result: LenResult,
}

impl CopyFileRange {
    /// Builds a `copy_file_range(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes copied on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        fd_in: i32,
        off_in: Buffer<Indirect<i64>>,
        fd_out: i32,
        off_out: Buffer<Indirect<i64>>,
        len: u64,
        flags: u64,
        raw_result: i64,
    ) -> Self {
        Self {
            fd_in,
            off_in,
            fd_out,
            off_out,
            len,
            flags,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for CopyFileRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "copy_file_range({}, {}, {}, {}, {}, {}) = {}",
            self.fd_in, self.off_in, self.fd_out, self.off_out, self.len, self.flags, self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `copy_file_range(3, [0], 4, [0], 5, 0) = 5` or
/// `copy_file_range(3, 0xADDR, 4, 0xADDR, 5, 0) = -1 EBADF`.
impl FromStr for CopyFileRange {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("copy_file_range call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("copy_file_range(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 6 {
            return Err(ParseError::new("copy_file_range arguments", args));
        }
        let fd_in = parts[0]
            .trim()
            .parse::<i32>()
            .map_err(|_| ParseError::new("fd_in", parts[0]))?;
        let off_in = super::buffer::parse(parts[1].trim())?;
        let fd_out = parts[2]
            .trim()
            .parse::<i32>()
            .map_err(|_| ParseError::new("fd_out", parts[2]))?;
        let off_out = super::buffer::parse(parts[3].trim())?;
        let len = parts[4]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("len", parts[4]))?;
        let flags = parts[5]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("flags", parts[5]))?;
        Ok(Self {
            fd_in,
            off_in,
            fd_out,
            off_out,
            len,
            flags,
            result: LenResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::buffer::Buffer;
    use crate::syscalls::errno::Errno;
    use crate::syscalls::indirect::Indirect;

    #[test]
    fn display_success() {
        let call = CopyFileRange::from_raw(
            3,
            Buffer::Decoded(Indirect::new(0i64)),
            4,
            Buffer::Decoded(Indirect::new(0i64)),
            5,
            0,
            5,
        );
        assert_eq!(
            call.to_string(),
            "copy_file_range(3, [0], 4, [0], 5, 0) = 5"
        );
        assert_eq!(call.result, LenResult::Len(5));
    }

    #[test]
    fn display_null() {
        let call = CopyFileRange::from_raw(3, Buffer::Address(0), 4, Buffer::Address(0), 5, 0, -1);
        assert!(call.to_string().contains("NULL"));
        assert_eq!(call.result, LenResult::Errno(Errno::EPERM));
    }

    #[test]
    fn parse_success() {
        let s = "copy_file_range(3, [0], 4, [0], 5, 0) = 5";
        let call = CopyFileRange::from_str(s).unwrap();
        assert_eq!(call.fd_in, 3);
        assert_eq!(call.fd_out, 4);
        assert_eq!(call.len, 5);
        assert_eq!(call.flags, 0);
        assert_eq!(call.result, LenResult::Len(5));
    }

    #[test]
    fn parse_failure() {
        let s = "copy_file_range(3, 0x7f, 4, [0], 5, 0) = -1 EBADF";
        let call = CopyFileRange::from_str(s).unwrap();
        match call.off_in {
            Buffer::Address(addr) => assert_eq!(addr, 0x7f),
            _ => panic!("expected raw"),
        }
        assert_eq!(call.result, LenResult::Errno(Errno::EBADF));
    }
}
