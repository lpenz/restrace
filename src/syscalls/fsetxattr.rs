// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fsetxattr(2)`] system call.
//!
//! [`fsetxattr(2)`]: https://man7.org/linux/man-pages/man2/fsetxattr.2.html

use super::buffer::Buffer;
use super::bytes::Bytes;
use super::fd::Fd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use super::xattrflags::XattrFlags;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `fsetxattr(2)` system call.
///
/// ```text
/// int fsetxattr(int fd, const char *name,
///               const void *value, size_t size, int flags);
/// ```
///
/// `strace` decodes `value` as a string of `size` bytes, omitting a NUL byte
/// in the last position, so the decoded [`Bytes`] can be one byte shorter
/// than `size`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fsetxattr {
    /// File descriptor of the file whose extended attribute is set, with the
    /// path `strace` resolved for it when run with `-y`.
    pub fd: Fd,
    /// Name of the extended attribute, without the terminating NUL byte.
    pub name: CString,
    /// The value of the extended attribute, or the raw address of the buffer
    /// when `strace` could not decode it.
    pub value: Buffer<Bytes>,
    /// Size of the value in bytes.
    pub size: u64,
    /// Flags, the `flags` argument; see [`XattrFlags`].
    pub flags: XattrFlags,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Fsetxattr {
    /// Builds a `fsetxattr(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `name` contains an interior NUL byte, which cannot happen
    /// for strings read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<N: AsRef<[u8]>>(
        fd: i32,
        name: N,
        value: Buffer<Bytes>,
        size: u64,
        flags: XattrFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            fd: Fd::new(fd),
            name: CString::new(name.as_ref()).expect("name contains NUL byte"),
            value,
            size,
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fsetxattr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fsetxattr({}, {:?}, {}, {}, {}) = {}",
            self.fd,
            self.name.to_string_lossy(),
            self.value,
            self.size,
            self.flags,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fsetxattr(3</tmp/f>, "user.a", "value", 5, 0) = 0` or
/// `... = -1 EBADF`.
impl FromStr for Fsetxattr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fsetxattr call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("fsetxattr(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 5 {
            return Err(ParseError::new("fsetxattr arguments", args));
        }
        let fd = parts[0]
            .trim()
            .parse::<Fd>()
            .map_err(|_| ParseError::new("fd", parts[0]))?;
        let name = parse::unquote(parts[1].trim(), "name")?;
        let value = super::buffer::parse(parts[2].trim())?;
        let size = parts[3]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("size", parts[3]))?;
        let flags = XattrFlags::from_str(parts[4].trim())?;
        Ok(Self {
            fd,
            name: CString::new(name).map_err(|_| ParseError::new("name", parts[1]))?,
            value,
            size,
            flags,
            result: OkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    fn decoded(value: &[u8]) -> Buffer<Bytes> {
        Buffer::Decoded(Bytes::new(value.to_vec()))
    }

    #[test]
    fn display_success() {
        let call = Fsetxattr::from_raw(3, "user.a", decoded(b"hello"), 5, XattrFlags(0), 0);
        assert_eq!(
            call.to_string(),
            "fsetxattr(3, \"user.a\", \"hello\", 5, 0) = 0"
        );
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_flags_and_failure() {
        let call = Fsetxattr::from_raw(3, "user.a", decoded(b"v"), 1, XattrFlags::XATTR_CREATE, -9);
        assert_eq!(
            call.to_string(),
            "fsetxattr(3, \"user.a\", \"v\", 1, XATTR_CREATE) = -1 EBADF"
        );
        assert_eq!(call.result, OkResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_success() {
        let line = "fsetxattr(3</tmp/f>, \"user.a\", \"hi\\0there\", 8, 0) = 0";
        let call = Fsetxattr::from_str(line).unwrap();
        assert_eq!(call.fd.fd(), 3);
        assert_eq!(
            call.fd.path().map(|path| path.to_str().unwrap()),
            Some("/tmp/f")
        );
        assert_eq!(call.name.to_str().unwrap(), "user.a");
        assert_eq!(call.value, decoded(b"hi\0there"));
        assert_eq!(call.size, 8);
        assert_eq!(call.flags, XattrFlags(0));
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_flags_and_failure() {
        let line = "fsetxattr(3, \"user.a\", \"\", 0, XATTR_REPLACE) = -1 EBADF";
        let call = Fsetxattr::from_str(line).unwrap();
        assert_eq!(call.fd, Fd::new(3));
        assert_eq!(call.value, decoded(b""));
        assert_eq!(call.flags, XattrFlags::XATTR_REPLACE);
        assert_eq!(call.result, OkResult::Errno(Errno::EBADF));
    }

    #[test]
    fn parse_invalid() {
        assert!(Fsetxattr::from_str("fsetxattr(3, \"n\", \"v\", 1, 0) 0").is_err());
        assert!(Fsetxattr::from_str("fsetxattr(3, \"n\", \"v\", 1) = 0").is_err());
        assert!(Fsetxattr::from_str("fsetxattr(3, \"n\", \"v\", 1, 0, 1) = 0").is_err());
        assert!(Fsetxattr::from_str("fsetxattr(x, \"n\", \"v\", 1, 0) = 0").is_err());
        assert!(Fsetxattr::from_str("fsetxattr(3, 0x1, \"v\", 1, 0) = 0").is_err());
        assert!(Fsetxattr::from_str("fsetxattr(3, \"n\", \"v\", 1, BOGUS) = 0").is_err());
        assert!(Fsetxattr::from_str("setxattr(\"/f\", \"n\", \"v\", 1, 0) = 0").is_err());
        assert!(Fsetxattr::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fsetxattr(3, \"user.a\", \"hello\", 5, 0) = 0",
            "fsetxattr(3</tmp/f>, \"user.a\", \"hello\", 5, 0) = 0",
            "fsetxattr(3, \"user.a\", \"\", 0, XATTR_CREATE) = -1 EEXIST",
            "fsetxattr(3, \"user.a\", \"hi\\0there\", 8, 0) = 0",
            "fsetxattr(3, \"user.a\", 0x7fff1234, 65537, 0) = -1 ERANGE",
            "fsetxattr(3, \"user.a\", \"a, b\", 4, XATTR_CREATE|XATTR_REPLACE|0x4) = 0",
            "fsetxattr(3, \"user.a\", \"v\", 1, 0x4 /* XATTR_??? */) = 0",
        ] {
            assert_eq!(Fsetxattr::from_str(line).unwrap().to_string(), line);
        }
    }
}
