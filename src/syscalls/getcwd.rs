// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`getcwd(2)`] system call.
//!
//! [`getcwd(2)`]: https://man7.org/linux/man-pages/man2/getcwd.2.html

use super::buffer::Buffer;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use super::quoted::Quoted;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `getcwd(2)` system call.
///
/// ```text
/// char *getcwd(char *buf, size_t size);
/// ```
///
/// `strace` decodes the buffer on exit: the current working directory
/// (`buf`) on success, or the raw address on failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Getcwd {
    /// The current working directory `strace` read back from the buffer, or
    /// the raw address of the buffer when the call failed.
    pub buf: Buffer<Quoted>,
    /// Size of the buffer the path was read into.
    pub size: u64,
    /// Return value: the length of the path in bytes, or the errno of the
    /// failure.
    pub result: LenResult,
}

impl Getcwd {
    /// Builds a `getcwd(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: the number
    /// of bytes written on success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(buf: Buffer<Quoted>, size: u64, raw_result: i64) -> Self {
        Self {
            buf,
            size,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Getcwd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "getcwd({}, {}) = {}", self.buf, self.size, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `getcwd("/tmp", 4096) = 4` or `getcwd(0xADDR, 4096) = -1 ENOENT`.
impl FromStr for Getcwd {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("getcwd call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("getcwd(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 2 {
            return Err(ParseError::new("getcwd arguments", args));
        }
        let buf = super::buffer::parse(parts[0].trim())?;
        let size = parts[1]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("size", parts[1]))?;
        Ok(Self {
            buf,
            size,
            result: LenResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;
    use std::ffi::CString;

    fn path(value: &str) -> Buffer<Quoted> {
        Buffer::Decoded(Quoted(CString::new(value).unwrap()))
    }

    #[test]
    fn display_success() {
        let call = Getcwd::from_raw(path("/tmp/opencode"), 4096, 13);
        assert_eq!(call.to_string(), "getcwd(\"/tmp/opencode\", 4096) = 13");
        assert_eq!(call.result, LenResult::Len(13));
    }

    #[test]
    fn display_failure() {
        let call = Getcwd::from_raw(Buffer::Address(0x7ffd1234), 4096, -2);
        assert_eq!(call.to_string(), "getcwd(0x7ffd1234, 4096) = -1 ENOENT");
        assert_eq!(call.result, LenResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_success() {
        let line = "getcwd(\"/tmp/opencode\", 4096) = 13";
        let call = Getcwd::from_str(line).unwrap();
        assert_eq!(call.buf, path("/tmp/opencode"));
        assert_eq!(call.size, 4096);
        assert_eq!(call.result, LenResult::Len(13));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_failure() {
        let line = "getcwd(0x1e33b810, 4096) = -1 ENOENT (No such file or directory)";
        let call = Getcwd::from_str(line).unwrap();
        assert_eq!(call.buf, Buffer::<Quoted>::Address(0x1e33b810));
        assert_eq!(call.size, 4096);
        assert_eq!(call.result, LenResult::Errno(Errno::ENOENT));
        assert_eq!(call.to_string(), "getcwd(0x1e33b810, 4096) = -1 ENOENT");
    }

    #[test]
    fn parse_invalid() {
        assert!(Getcwd::from_str("getcwd(\"/tmp\") = 4").is_err());
        assert!(Getcwd::from_str("getcwd(/tmp, 4096) = 4").is_err());
        assert!(Getcwd::from_str("getcwd(\"/tmp\", 4096, 1) = 4").is_err());
        assert!(Getcwd::from_str("chdir(\"/tmp\") = 0").is_err());
        assert!(Getcwd::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "getcwd(\"/tmp/opencode\", 4096) = 13",
            "getcwd(\"/\", 4096) = 1",
            "getcwd(\"/a, b\", 4096) = 5",
            "getcwd(0x7ffd1234, 4096) = -1 ENOENT",
        ] {
            assert_eq!(Getcwd::from_str(line).unwrap().to_string(), line);
        }
    }
}
