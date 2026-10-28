// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`lsetxattr(2)`] system call.
//!
//! [`lsetxattr(2)`]: https://man7.org/linux/man-pages/man2/lsetxattr.2.html

use super::buffer::Buffer;
use super::bytes::Bytes;
use super::parse::{self, ParseError};
use super::result::OkResult;
use super::xattrflags::XattrFlags;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `lsetxattr(2)` system call.
///
/// ```text
/// int lsetxattr(const char *pathname, const char *name,
///               const void *value, size_t size, int flags);
/// ```
///
/// Unlike [`setxattr(2)`](super::setxattr::Setxattr), the call does not follow
/// a symbolic link in `pathname`.
///
/// `strace` decodes `value` as a string of `size` bytes, omitting a NUL byte
/// in the last position, so the decoded [`Bytes`] can be one byte shorter
/// than `size`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lsetxattr {
    /// Pathname of the file whose extended attribute is set, without the
    /// terminating NUL byte.
    pub pathname: CString,
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

impl Lsetxattr {
    /// Builds a `lsetxattr(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` or `name` contains an interior NUL byte, which
    /// cannot happen for strings read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>, N: AsRef<[u8]>>(
        pathname: P,
        name: N,
        value: Buffer<Bytes>,
        size: u64,
        flags: XattrFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            name: CString::new(name.as_ref()).expect("name contains NUL byte"),
            value,
            size,
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Lsetxattr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "lsetxattr({:?}, {:?}, {}, {}, {}) = {}",
            self.pathname.to_string_lossy(),
            self.name.to_string_lossy(),
            self.value,
            self.size,
            self.flags,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `lsetxattr("/tmp/f", "user.a", "value", 5, 0) = 0` or
/// `... = -1 EPERM`.
impl FromStr for Lsetxattr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("lsetxattr call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("lsetxattr(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 5 {
            return Err(ParseError::new("lsetxattr arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        let name = parse::unquote(parts[1].trim(), "name")?;
        let value = super::buffer::parse(parts[2].trim())?;
        let size = parts[3]
            .trim()
            .parse::<u64>()
            .map_err(|_| ParseError::new("size", parts[3]))?;
        let flags = XattrFlags::from_str(parts[4].trim())?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
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
        let call = Lsetxattr::from_raw("/tmp/f", "user.a", decoded(b"hello"), 5, XattrFlags(0), 0);
        assert_eq!(
            call.to_string(),
            "lsetxattr(\"/tmp/f\", \"user.a\", \"hello\", 5, 0) = 0"
        );
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_flags_and_failure() {
        let call = Lsetxattr::from_raw(
            "/tmp/f",
            "user.a",
            decoded(b"v"),
            1,
            XattrFlags::XATTR_REPLACE,
            -1,
        );
        assert_eq!(
            call.to_string(),
            "lsetxattr(\"/tmp/f\", \"user.a\", \"v\", 1, XATTR_REPLACE) = -1 EPERM"
        );
        assert_eq!(call.result, OkResult::Errno(Errno::EPERM));
    }

    #[test]
    fn parse_success() {
        let line = "lsetxattr(\"/tmp/f\", \"user.a\", \"hi\\0there\", 8, 0) = 0";
        let call = Lsetxattr::from_str(line).unwrap();
        assert_eq!(call.pathname.to_str().unwrap(), "/tmp/f");
        assert_eq!(call.name.to_str().unwrap(), "user.a");
        assert_eq!(call.value, decoded(b"hi\0there"));
        assert_eq!(call.size, 8);
        assert_eq!(call.flags, XattrFlags(0));
        assert_eq!(call.result, OkResult::Ok);
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_flags_and_failure() {
        let line = "lsetxattr(\"/tmp/f\", \"user.a\", \"\", 0, XATTR_CREATE) = -1 EEXIST";
        let call = Lsetxattr::from_str(line).unwrap();
        assert_eq!(call.value, decoded(b""));
        assert_eq!(call.flags, XattrFlags::XATTR_CREATE);
        assert_eq!(call.result, OkResult::Errno(Errno::EEXIST));
    }

    #[test]
    fn parse_invalid() {
        assert!(Lsetxattr::from_str("lsetxattr(\"/f\", \"n\", \"v\", 1, 0) 0").is_err());
        assert!(Lsetxattr::from_str("lsetxattr(\"/f\", \"n\", \"v\", 1) = 0").is_err());
        assert!(Lsetxattr::from_str("lsetxattr(\"/f\", \"n\", \"v\", 1, 0, 1) = 0").is_err());
        assert!(Lsetxattr::from_str("lsetxattr(\"/f\", 0x1, \"v\", 1, 0) = 0").is_err());
        assert!(Lsetxattr::from_str("lsetxattr(\"/f\", \"n\", \"v\", x, 0) = 0").is_err());
        assert!(Lsetxattr::from_str("lsetxattr(\"/f\", \"n\", \"v\", 1, BOGUS) = 0").is_err());
        assert!(Lsetxattr::from_str("setxattr(\"/f\", \"n\", \"v\", 1, 0) = 0").is_err());
        assert!(Lsetxattr::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "lsetxattr(\"/tmp/f\", \"user.a\", \"hello\", 5, 0) = 0",
            "lsetxattr(\"/tmp/f\", \"user.a\", \"\", 0, XATTR_CREATE) = -1 EEXIST",
            "lsetxattr(\"/tmp/f\", \"user.a\", \"hi\\0there\", 8, 0) = 0",
            "lsetxattr(\"/tmp/f\", \"user.a\", 0x7fff1234, 65537, 0) = -1 ERANGE",
            "lsetxattr(\"/tmp/f\", \"user.a\", \"a, b\", 4, XATTR_CREATE|XATTR_REPLACE|0x4) = 0",
            "lsetxattr(\"/tmp/f\", \"user.a\", \"v\", 1, 0x4 /* XATTR_??? */) = 0",
        ] {
            assert_eq!(Lsetxattr::from_str(line).unwrap().to_string(), line);
        }
    }
}
