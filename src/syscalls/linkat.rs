// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`linkat(2)`] system call.
//!
//! [`linkat(2)`]: https://man7.org/linux/man-pages/man2/linkat.2.html

use super::dirfd::DirFd;
use super::flags;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Named bits of [`LinkatFlags`], in [`Display`](fmt::Display) order.
const NAMED_FLAGS: &[(u32, &str)] = &[
    (LinkatFlags::AT_SYMLINK_FOLLOW.0, "AT_SYMLINK_FOLLOW"),
    (LinkatFlags::AT_EMPTY_PATH.0, "AT_EMPTY_PATH"),
];

/// Arguments and return value of the `linkat(2)` system call.
///
/// ```text
/// int linkat(int olddirfd, const char *oldpath,
///            int newdirfd, const char *newpath, int flags);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Linkat {
    /// Directory against which the old pathname is resolved, or
    /// [`DirFd::AT_FDCWD`].
    pub olddirfd: DirFd,
    /// Pathname of the existing file, without the terminating NUL byte.
    ///
    /// Interpreted relative to `olddirfd` unless it is absolute.
    pub oldpath: CString,
    /// Directory against which the new pathname is resolved, or
    /// [`DirFd::AT_FDCWD`].
    pub newdirfd: DirFd,
    /// Pathname of the new link, without the terminating NUL byte.
    ///
    /// Interpreted relative to `newdirfd` unless it is absolute.
    pub newpath: CString,
    /// Flags, the `flags` argument; see [`LinkatFlags`].
    pub flags: LinkatFlags,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Linkat {
    /// Builds a `linkat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if a pathname contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>, Q: AsRef<[u8]>>(
        olddirfd: DirFd,
        oldpath: P,
        newdirfd: DirFd,
        newpath: Q,
        flags: LinkatFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            olddirfd,
            oldpath: CString::new(oldpath.as_ref()).expect("pathname contains NUL byte"),
            newdirfd,
            newpath: CString::new(newpath.as_ref()).expect("pathname contains NUL byte"),
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Linkat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "linkat({}, {:?}, {}, {:?}, {}) = {}",
            self.olddirfd,
            self.oldpath.to_string_lossy(),
            self.newdirfd,
            self.newpath.to_string_lossy(),
            self.flags,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `linkat(AT_FDCWD, "old", AT_FDCWD, "new", 0) = 0` or `... = -1 EEXIST`.
impl FromStr for Linkat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("linkat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("linkat(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 5 {
            return Err(ParseError::new("linkat arguments", args));
        }
        let olddirfd = DirFd::from_str(parts[0].trim())?;
        let oldpath = parse::unquote(parts[1].trim(), "oldpath")?;
        let newdirfd = DirFd::from_str(parts[2].trim())?;
        let newpath = parse::unquote(parts[3].trim(), "newpath")?;
        let flags = LinkatFlags::from_str(parts[4].trim())?;
        Ok(Self {
            olddirfd,
            oldpath: CString::new(oldpath).map_err(|_| ParseError::new("oldpath", parts[1]))?,
            newdirfd,
            newpath: CString::new(newpath).map_err(|_| ParseError::new("newpath", parts[3]))?,
            flags,
            result: OkResult::from_str(result)?,
        })
    }
}

/// The `flags` argument of `linkat(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct LinkatFlags(pub u32);

impl LinkatFlags {
    /// Follow symbolic links in the old pathname.
    pub const AT_SYMLINK_FOLLOW: Self = Self(0o400);
    /// Allow the old pathname to be empty, linking to the file `olddirfd`
    /// refers to, which may be of any type except a directory.
    pub const AT_EMPTY_PATH: Self = Self(0o1000);
}

impl fmt::Display for LinkatFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_FLAGS)
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them.
impl FromStr for LinkatFlags {
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
        let linkat = Linkat::from_raw(
            DirFd::AT_FDCWD,
            "/a",
            DirFd::AT_FDCWD,
            "/b",
            LinkatFlags(0),
            0,
        );
        assert_eq!(
            linkat.to_string(),
            "linkat(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\", 0) = 0"
        );
        assert!(linkat.result.is_ok());
    }

    #[test]
    fn display_flags_and_failure() {
        let linkat = Linkat::from_raw(
            DirFd(3),
            "a",
            DirFd(4),
            "b",
            LinkatFlags::AT_SYMLINK_FOLLOW,
            -1,
        );
        assert_eq!(
            linkat.to_string(),
            "linkat(3, \"a\", 4, \"b\", AT_SYMLINK_FOLLOW) = -1 EPERM"
        );
    }

    #[test]
    fn parse_success() {
        let linkat = Linkat::from_str(
            "linkat(AT_FDCWD, \"f.txt\", AT_FDCWD, \"h2.lnk\", AT_SYMLINK_FOLLOW) = 0",
        )
        .unwrap();
        assert_eq!(
            linkat,
            Linkat::from_raw(
                DirFd::AT_FDCWD,
                "f.txt",
                DirFd::AT_FDCWD,
                "h2.lnk",
                LinkatFlags::AT_SYMLINK_FOLLOW,
                0,
            )
        );
    }

    #[test]
    fn parse_failure() {
        let linkat = Linkat::from_str("linkat(3, \"a\", 4, \"b\", 0) = -1 ENOENT").unwrap();
        assert_eq!(linkat.olddirfd, DirFd(3));
        assert_eq!(linkat.newdirfd, DirFd(4));
        assert_eq!(linkat.flags, LinkatFlags(0));
        assert_eq!(linkat.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Linkat::from_str("linkat(AT_FDCWD, \"a\", AT_FDCWD, \"b\", 0) 0").is_err());
        assert!(Linkat::from_str("linkat(AT_FDCWD, \"a\", AT_FDCWD, \"b\") = 0").is_err());
        assert!(Linkat::from_str("linkat(AT_FDCWD, \"a\", \"b\", 0) = 0").is_err());
        assert!(Linkat::from_str("linkat(AT_FDCWD, \"a\", AT_FDCWD, \"b\", BOGUS) = 0").is_err());
        assert!(Linkat::from_str("link(\"a\", \"b\") = 0").is_err());
        assert!(Linkat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "linkat(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\", 0) = 0",
            "linkat(3, \"a\", 4, \"b\", AT_SYMLINK_FOLLOW) = -1 EPERM",
            "linkat(AT_FDCWD, \"a, b\", 0, \"c, d\", AT_EMPTY_PATH|0x1) = -1 ENOENT",
        ] {
            assert_eq!(Linkat::from_str(line).unwrap().to_string(), line);
        }
    }
}
