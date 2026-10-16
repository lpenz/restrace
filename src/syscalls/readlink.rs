// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`readlink(2)`] system call.
//!
//! [`readlink(2)`]: https://man7.org/linux/man-pages/man2/readlink.2.html

use super::buffer::Buffer;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use super::quoted::Quoted;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `readlink(2)` system call.
///
/// ```text
/// ssize_t readlink(const char *restrict pathname, char *restrict buf, size_t bufsiz);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readlink {
    /// Pathname of the symbolic link to read.
    pub pathname: CString,
    /// The string stored in the buffer on success, or the raw address on
    /// failure.
    pub buf: Buffer<Quoted>,
    /// Size of the buffer the string was read into.
    pub bufsiz: u64,
    /// Return value: the number of bytes read (not including the terminating
    /// NUL), or the errno of the failure.
    pub result: LenResult,
}

impl Readlink {
    /// Builds a `readlink(2)` record from its arguments and raw return value.
    #[must_use]
    pub fn from_raw(
        pathname: impl Into<Vec<u8>>,
        buf: Buffer<Quoted>,
        bufsiz: u64,
        raw_result: i64,
    ) -> Self {
        Self {
            pathname: CString::new(pathname).expect("pathname contains NUL byte"),
            buf,
            bufsiz,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Readlink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "readlink({:?}, {}, {}) = {}",
            self.pathname.to_string_lossy(),
            self.buf,
            self.bufsiz,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it.
impl FromStr for Readlink {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("readlink call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("readlink(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 3 {
            return Err(ParseError::new("readlink arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
            buf: super::buffer::parse(parts[1].trim())?,
            bufsiz: parts[2]
                .trim()
                .parse::<u64>()
                .map_err(|_| ParseError::new("bufsiz", parts[2]))?,
            result: LenResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;
    use crate::syscalls::quoted::Quoted;

    fn target() -> Buffer<Quoted> {
        Buffer::Decoded(Quoted(CString::new("/tmp/opencode/test_rl").unwrap()))
    }

    #[test]
    fn display_success() {
        let call = Readlink::from_raw("/proc/self/exe", target(), 256, 21);
        assert_eq!(
            call.to_string(),
            "readlink(\"/proc/self/exe\", \"/tmp/opencode/test_rl\", 256) = 21"
        );
    }

    #[test]
    fn display_failure() {
        let call = Readlink::from_raw("/nope", Buffer::Address(0x7ffebf56f290), 64, -2);
        assert_eq!(
            call.to_string(),
            "readlink(\"/nope\", 0x7ffebf56f290, 64) = -1 ENOENT"
        );
        assert_eq!(call.result, LenResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_success() {
        let line = "readlink(\"/proc/self/exe\", \"/tmp/opencode/test_rl\", 256) = 21";
        let call = Readlink::from_str(line).unwrap();
        assert_eq!(call.pathname.to_str().unwrap(), "/proc/self/exe");
        assert_eq!(call.buf, target());
        assert_eq!(call.bufsiz, 256);
        assert_eq!(call.result, LenResult::Len(21));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_failure() {
        let line = "readlink(\"/nope\", 0x7ffebf56f290, 64) = -1 ENOENT";
        let call = Readlink::from_str(line).unwrap();
        assert_eq!(call.buf, Buffer::<Quoted>::Address(0x7ffebf56f290));
        assert_eq!(call.result, LenResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Readlink::from_str("readlink(\"/a\") = 0").is_err());
        assert!(Readlink::from_str("readlink(/a, 0x1, 64) = 0").is_err());
        assert!(Readlink::from_str("readlinkat(AT_FDCWD, \"/a\", 0x1, 64) = 0").is_err());
        assert!(Readlink::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "readlink(\"/proc/self/exe\", \"/tmp/opencode/test_rl\", 256) = 21",
            "readlink(\"/nope\", 0x7ffebf56f290, 64) = -1 ENOENT",
            "readlink(\"/a\", NULL, 64) = -1 EACCES",
        ] {
            assert_eq!(Readlink::from_str(line).unwrap().to_string(), line);
        }
    }
}
