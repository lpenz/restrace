// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`readlinkat(2)`] system call.
//!
//! [`readlinkat(2)`]: https://man7.org/linux/man-pages/man2/readlinkat.2.html

use super::buffer::Buffer;
use super::dirfd::DirFd;
use super::lenresult::LenResult;
use super::parse::{self, ParseError};
use super::quoted::Quoted;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `readlinkat(2)` system call.
///
/// ```text
/// ssize_t readlinkat(int dirfd, const char *restrict pathname,
///                    char *restrict buf, size_t bufsiz);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Readlinkat {
    /// Directory file descriptor to interpret `pathname` relative to.
    pub dirfd: DirFd,
    /// Pathname of the symbolic link to read.
    pub pathname: CString,
    /// The string stored in the buffer on success, or the raw address on
    /// failure.
    pub buf: Buffer<Quoted>,
    /// Size of the buffer the string was read into.
    pub bufsiz: u64,
    /// Return value: the number of bytes read, or the errno of the failure.
    pub result: LenResult,
}

impl Readlinkat {
    /// Builds a `readlinkat(2)` record from its arguments and raw return
    /// value.
    #[must_use]
    pub fn from_raw(
        dirfd: DirFd,
        pathname: impl Into<Vec<u8>>,
        buf: Buffer<Quoted>,
        bufsiz: u64,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname).expect("pathname contains NUL byte"),
            buf,
            bufsiz,
            result: LenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Readlinkat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "readlinkat({}, {:?}, {}, {}) = {}",
            self.dirfd,
            self.pathname.to_string_lossy(),
            self.buf,
            self.bufsiz,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it.
impl FromStr for Readlinkat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("readlinkat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("readlinkat(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("readlinkat arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
            buf: super::buffer::parse(parts[2].trim())?,
            bufsiz: parts[3]
                .trim()
                .parse::<u64>()
                .map_err(|_| ParseError::new("bufsiz", parts[3]))?,
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
        Buffer::Decoded(Quoted::new(b"/tmp/opencode/test_rlat").unwrap())
    }

    #[test]
    fn display_success() {
        let call = Readlinkat::from_raw(DirFd::AT_FDCWD, "/proc/self/exe", target(), 256, 23);
        assert_eq!(
            call.to_string(),
            "readlinkat(AT_FDCWD, \"/proc/self/exe\", \"/tmp/opencode/test_rlat\", 256) = 23"
        );
    }

    #[test]
    fn display_failure() {
        let call = Readlinkat::from_raw(
            DirFd::AT_FDCWD,
            "/missing",
            Buffer::Address(0x7ffc9c575d80),
            64,
            -2,
        );
        assert_eq!(
            call.to_string(),
            "readlinkat(AT_FDCWD, \"/missing\", 0x7ffc9c575d80, 64) = -1 ENOENT"
        );
        assert_eq!(call.result, LenResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_success() {
        let line =
            "readlinkat(AT_FDCWD, \"/proc/self/exe\", \"/tmp/opencode/test_rlat\", 256) = 23";
        let call = Readlinkat::from_str(line).unwrap();
        assert_eq!(call.dirfd, DirFd::AT_FDCWD);
        assert_eq!(call.pathname.to_str().unwrap(), "/proc/self/exe");
        assert_eq!(call.buf, target());
        assert_eq!(call.bufsiz, 256);
        assert_eq!(call.result, LenResult::Len(23));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Readlinkat::from_str("readlinkat(AT_FDCWD, \"/a\") = 0").is_err());
        assert!(Readlinkat::from_str("readlink(\"/a\", 0x1, 64) = 0").is_err());
        assert!(Readlinkat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "readlinkat(AT_FDCWD, \"/proc/self/exe\", \"/tmp/opencode/test_rlat\", 256) = 23",
            "readlinkat(AT_FDCWD, \"/missing\", 0x7ffc9c575d80, 64) = -1 ENOENT",
            "readlinkat(3, \"/a\", NULL, 64) = -1 EBADF",
        ] {
            assert_eq!(Readlinkat::from_str(line).unwrap().to_string(), line);
        }
    }
}
