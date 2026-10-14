// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`unlinkat(2)`] system call.
//!
//! [`unlinkat(2)`]: https://man7.org/linux/man-pages/man2/unlinkat.2.html

use super::dirfd::DirFd;
use super::flags;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Named bits of [`UnlinkatFlags`], in [`Display`](fmt::Display) order.
const NAMED_FLAGS: &[(u32, &str)] = &[(UnlinkatFlags::AT_REMOVEDIR.0, "AT_REMOVEDIR")];

/// Arguments and return value of the `unlinkat(2)` system call.
///
/// ```text
/// int unlinkat(int dirfd, const char *pathname, int flags);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unlinkat {
    /// Directory against which the pathname is resolved, or [`DirFd::AT_FDCWD`].
    pub dirfd: DirFd,
    /// Pathname of the file or directory to remove, without the terminating
    /// NUL byte.
    ///
    /// Interpreted relative to `dirfd` unless it is absolute.
    pub pathname: CString,
    /// Flags, the `flags` argument; see [`UnlinkatFlags`].
    pub flags: UnlinkatFlags,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Unlinkat {
    /// Builds an `unlinkat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(
        dirfd: DirFd,
        pathname: P,
        flags: UnlinkatFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Unlinkat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unlinkat({}, {:?}, {}) = {}",
            self.dirfd,
            self.pathname.to_string_lossy(),
            self.flags,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `unlinkat(AT_FDCWD, "path", 0) = 0` or `... = -1 ENOENT`.
impl FromStr for Unlinkat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("unlinkat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("unlinkat(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 3 {
            return Err(ParseError::new("unlinkat arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        let flags = UnlinkatFlags::from_str(parts[2].trim())?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
            flags,
            result: OkResult::from_str(result)?,
        })
    }
}

/// The `flags` argument of `unlinkat(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct UnlinkatFlags(pub u32);

impl UnlinkatFlags {
    /// Remove the directory named by the pathname instead of a file.
    pub const AT_REMOVEDIR: Self = Self(0o200);
}

impl fmt::Display for UnlinkatFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_FLAGS)
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them.
impl FromStr for UnlinkatFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        flags::parse(s, NAMED_FLAGS, "flags").map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_success() {
        let unlinkat = Unlinkat::from_raw(DirFd::AT_FDCWD, "/tmp/file", UnlinkatFlags(0), 0);
        assert_eq!(
            unlinkat.to_string(),
            "unlinkat(AT_FDCWD, \"/tmp/file\", 0) = 0"
        );
        assert!(unlinkat.result.is_ok());
    }

    #[test]
    fn display_removedir_failure() {
        let unlinkat = Unlinkat::from_raw(DirFd(4), "dir", UnlinkatFlags::AT_REMOVEDIR, -20);
        assert_eq!(
            unlinkat.to_string(),
            "unlinkat(4, \"dir\", AT_REMOVEDIR) = -1 ENOTDIR"
        );
    }

    #[test]
    fn parse_success() {
        let unlinkat = Unlinkat::from_str("unlinkat(AT_FDCWD, \"/tmp/file\", 0) = 0").unwrap();
        assert_eq!(
            unlinkat,
            Unlinkat::from_raw(DirFd::AT_FDCWD, "/tmp/file", UnlinkatFlags(0), 0)
        );
    }

    #[test]
    fn parse_removedir() {
        let unlinkat =
            Unlinkat::from_str("unlinkat(3, \"dir\", AT_REMOVEDIR) = -1 ENOENT").unwrap();
        assert_eq!(unlinkat.dirfd, DirFd(3));
        assert_eq!(unlinkat.flags, UnlinkatFlags::AT_REMOVEDIR);
        assert_eq!(unlinkat.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Unlinkat::from_str("unlinkat(AT_FDCWD, \"f\", 0) 0").is_err());
        assert!(Unlinkat::from_str("unlinkat(AT_FDCWD, \"f\") = 0").is_err());
        assert!(Unlinkat::from_str("unlinkat(\"f\", 0) = 0").is_err());
        assert!(Unlinkat::from_str("unlinkat(AT_FDCWD, \"f\", BOGUS) = 0").is_err());
        assert!(Unlinkat::from_str("unlink(\"/f\") = 0").is_err());
        assert!(Unlinkat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "unlinkat(AT_FDCWD, \"/tmp/file\", 0) = 0",
            "unlinkat(3, \"dir\", AT_REMOVEDIR) = -1 ENOENT",
            "unlinkat(AT_FDCWD, \"a, b\", 0) = -1 EACCES",
        ] {
            assert_eq!(Unlinkat::from_str(line).unwrap().to_string(), line);
        }
    }
}
