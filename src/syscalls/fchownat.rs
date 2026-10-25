// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`fchownat(2)`] system call.
//!
//! [`fchownat(2)`]: https://man7.org/linux/man-pages/man2/fchownat.2.html

use super::dirfd::DirFd;
use super::flags;
use super::parse::{self, ParseError};
use super::result::OkResult;
use super::uid::Uid;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Named bits of [`FchownatFlags`], in [`Display`](fmt::Display) order.
///
/// These are the names of strace's shared `at_flags` table, which it uses
/// when printing the flags of `fchownat(2)`.
const NAMED_FLAGS: &[(u32, &str)] = &[
    (0x100, "AT_SYMLINK_NOFOLLOW"),
    (0x200, "AT_REMOVEDIR"),
    (0x400, "AT_SYMLINK_FOLLOW"),
    (0x800, "AT_NO_AUTOMOUNT"),
    (0x1000, "AT_EMPTY_PATH"),
    (0x8000, "AT_RECURSIVE"),
];

/// Arguments and return value of the `fchownat(2)` system call.
///
/// ```text
/// int fchownat(int dirfd, const char *pathname,
///              uid_t owner, gid_t group, int flags);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fchownat {
    /// Directory against which the pathname is resolved, or
    /// [`DirFd::AT_FDCWD`].
    pub dirfd: DirFd,
    /// Pathname of the file to change ownership of, without the terminating
    /// NUL byte.
    pub pathname: CString,
    /// User ID to set, or [`Uid::NO_CHANGE`] to leave it unchanged.
    pub owner: Uid,
    /// Group ID to set, or [`Uid::NO_CHANGE`] to leave it unchanged.
    pub group: Uid,
    /// Flags, the `flags` argument; see [`FchownatFlags`].
    pub flags: FchownatFlags,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Fchownat {
    /// Builds a `fchownat(2)` record from its arguments and raw return value.
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
        owner: Uid,
        group: Uid,
        flags: FchownatFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            owner,
            group,
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Fchownat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fchownat({}, {:?}, {}, {}, {}) = {}",
            self.dirfd,
            self.pathname.to_string_lossy(),
            self.owner,
            self.group,
            self.flags,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `fchownat(AT_FDCWD, "path", 1000, 1000, 0) = 0` or `... = -1 ENOENT`.
impl FromStr for Fchownat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("fchownat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("fchownat(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 5 {
            return Err(ParseError::new("fchownat arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        let owner = Uid::from_str(parts[2].trim())?;
        let group = Uid::from_str(parts[3].trim())?;
        let flags = FchownatFlags::from_str(parts[4].trim())?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
            owner,
            group,
            flags,
            result: OkResult::from_str(result)?,
        })
    }
}

/// The `flags` argument of `fchownat(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct FchownatFlags(pub u32);

impl FchownatFlags {
    /// Do not dereference symbolic links in the pathname.
    pub const AT_SYMLINK_NOFOLLOW: Self = Self(0x100);
    /// Operate on the file referred to by `dirfd`, with an empty pathname.
    pub const AT_EMPTY_PATH: Self = Self(0x1000);
}

impl fmt::Display for FchownatFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_FLAGS, "AT_???")
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them.
impl FromStr for FchownatFlags {
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
    fn flags_display() {
        assert_eq!(FchownatFlags(0).to_string(), "0");
        assert_eq!(
            FchownatFlags::AT_SYMLINK_NOFOLLOW.to_string(),
            "AT_SYMLINK_NOFOLLOW"
        );
        assert_eq!(FchownatFlags::AT_EMPTY_PATH.to_string(), "AT_EMPTY_PATH");
        assert_eq!(
            FchownatFlags(0x100 | 0x1000).to_string(),
            "AT_SYMLINK_NOFOLLOW|AT_EMPTY_PATH"
        );
    }

    #[test]
    fn flags_parse() {
        assert_eq!(FchownatFlags::from_str("0"), Ok(FchownatFlags(0)));
        assert_eq!(
            FchownatFlags::from_str("AT_SYMLINK_NOFOLLOW"),
            Ok(FchownatFlags(0x100))
        );
        assert_eq!(
            FchownatFlags::from_str("AT_EMPTY_PATH"),
            Ok(FchownatFlags(0x1000))
        );
        assert_eq!(FchownatFlags::from_str("0x8000"), Ok(FchownatFlags(0x8000)));
        assert!(FchownatFlags::from_str("AT_NOPE").is_err());
    }

    #[test]
    fn display_success() {
        let call = Fchownat::from_raw(
            DirFd::AT_FDCWD,
            "/tmp/f1",
            Uid(1000),
            Uid(1000),
            FchownatFlags(0),
            0,
        );
        assert_eq!(
            call.to_string(),
            "fchownat(AT_FDCWD, \"/tmp/f1\", 1000, 1000, 0) = 0"
        );
        assert!(call.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let call = Fchownat::from_raw(
            DirFd::new(3),
            "f1",
            Uid::NO_CHANGE,
            Uid(1000),
            FchownatFlags::AT_SYMLINK_NOFOLLOW,
            -2,
        );
        assert_eq!(
            call.to_string(),
            "fchownat(3, \"f1\", -1, 1000, AT_SYMLINK_NOFOLLOW) = -1 ENOENT"
        );
        assert!(!call.result.is_ok());
    }

    #[test]
    fn parse_success() {
        let call =
            Fchownat::from_str("fchownat(AT_FDCWD, \"/tmp/f1\", 1000, 1000, 0) = 0").unwrap();
        assert_eq!(call.dirfd, DirFd::AT_FDCWD);
        assert_eq!(call.pathname.to_str().unwrap(), "/tmp/f1");
        assert_eq!(call.owner, Uid(1000));
        assert_eq!(call.group, Uid(1000));
        assert_eq!(call.flags, FchownatFlags(0));
        assert_eq!(call.result, OkResult::Ok);
    }

    #[test]
    fn parse_flags_and_failure() {
        let call = Fchownat::from_str(
            "fchownat(3</tmp>, \"f1\", -1, -1, AT_SYMLINK_NOFOLLOW|AT_EMPTY_PATH) = -1 ENOENT",
        )
        .unwrap();
        assert_eq!(call.dirfd.fd, 3);
        assert_eq!(
            call.dirfd.path().map(|path| path.to_str().unwrap()),
            Some("/tmp")
        );
        assert_eq!(call.owner, Uid::NO_CHANGE);
        assert_eq!(call.group, Uid::NO_CHANGE);
        assert_eq!(call.flags, FchownatFlags(0x100 | 0x1000));
        assert_eq!(call.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Fchownat::from_str("fchownat(AT_FDCWD, \"/f\", 0, 0, 0) 0").is_err());
        assert!(Fchownat::from_str("fchownat(AT_FDCWD, \"/f\", 0, 0) = 0").is_err());
        assert!(Fchownat::from_str("fchownat(AT_FDCWD, \"/f\", 0, 0, 0, 1) = 0").is_err());
        assert!(Fchownat::from_str("fchownat(AT_FDCWD, \"/f\", 0, 0, AT_NOPE) = 0").is_err());
        assert!(Fchownat::from_str("chown(\"/f\", 0, 0) = 0").is_err());
        assert!(Fchownat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "fchownat(AT_FDCWD, \"/tmp/f1\", 1000, 1000, 0) = 0",
            "fchownat(3, \"f1\", -1, 1000, AT_SYMLINK_NOFOLLOW) = -1 ENOENT",
            "fchownat(3</tmp>, \"f1\", -1, -1, AT_SYMLINK_NOFOLLOW|AT_EMPTY_PATH) = -1 ENOENT",
            "fchownat(AT_FDCWD, \"/a, b\", 0, 0, AT_EMPTY_PATH|0x1) = -1 EINVAL",
            "fchownat(AT_FDCWD, \"/tmp/f1\", 4294967294, 4294967294, 0) = 0",
        ] {
            assert_eq!(Fchownat::from_str(line).unwrap().to_string(), line);
        }
    }
}
