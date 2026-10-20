// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`faccessat2(2)`] system call.
//!
//! [`faccessat2(2)`]: https://man7.org/linux/man-pages/man2/faccessat2.2.html

use super::accessmode::AccessMode;
use super::dirfd::DirFd;
use super::flags;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Named bits of [`FaccessatFlags`], in [`Display`](fmt::Display) order.
const NAMED_FLAGS: &[(u32, &str)] = &[
    (FaccessatFlags::AT_EACCESS.0, "AT_EACCESS"),
    (FaccessatFlags::AT_SYMLINK_NOFOLLOW.0, "AT_SYMLINK_NOFOLLOW"),
    (FaccessatFlags::AT_EMPTY_PATH.0, "AT_EMPTY_PATH"),
];

/// Arguments and return value of the `faccessat2(2)` system call.
///
/// ```text
/// int faccessat2(int dirfd, const char *pathname, int mode, int flags);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Faccessat2 {
    /// Directory against which the pathname is resolved, or
    /// [`DirFd::AT_FDCWD`].
    pub dirfd: DirFd,
    /// Pathname of the file to test, without the terminating NUL byte.
    ///
    /// Interpreted relative to `dirfd` unless it is absolute.
    pub pathname: CString,
    /// Permissions to test for, the `mode` argument.
    pub mode: AccessMode,
    /// Flags, the `flags` argument; see [`FaccessatFlags`].
    pub flags: FaccessatFlags,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Faccessat2 {
    /// Builds an `faccessat2(2)` record from its arguments and raw return value.
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
        mode: AccessMode,
        flags: FaccessatFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            mode,
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Faccessat2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "faccessat2({}, {:?}, {}, {}) = {}",
            self.dirfd,
            self.pathname.to_string_lossy(),
            self.mode,
            self.flags,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `faccessat2(AT_FDCWD, "path", R_OK, AT_EACCESS) = 0` or `... = -1 EACCES`.
impl FromStr for Faccessat2 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("faccessat2 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("faccessat2(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 4 {
            return Err(ParseError::new("faccessat2 arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        let mode = AccessMode::from_str(parts[2].trim())?;
        let flags = FaccessatFlags::from_str(parts[3].trim())?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
            mode,
            flags,
            result: OkResult::from_str(result)?,
        })
    }
}

/// The `flags` argument of `faccessat2(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct FaccessatFlags(pub u32);

impl FaccessatFlags {
    /// Use effective user and group IDs for permission checks.
    pub const AT_EACCESS: Self = Self(0o200);
    /// Do not follow symbolic links in the pathname.
    pub const AT_SYMLINK_NOFOLLOW: Self = Self(0o100);
    /// Allow the pathname to be empty, operating on the file `dirfd` refers to,
    /// which may be of any type.
    pub const AT_EMPTY_PATH: Self = Self(0o1000);
}

impl fmt::Display for FaccessatFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_FLAGS, "AT_???")
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them.
impl FromStr for FaccessatFlags {
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
        let faccessat2 = Faccessat2::from_raw(
            DirFd::AT_FDCWD,
            "/etc/hostname",
            AccessMode::R_OK,
            FaccessatFlags::AT_EACCESS,
            0,
        );
        assert_eq!(
            faccessat2.to_string(),
            "faccessat2(AT_FDCWD, \"/etc/hostname\", R_OK, AT_EACCESS) = 0"
        );
        assert!(faccessat2.result.is_ok());
    }

    #[test]
    fn display_failure() {
        let faccessat2 = Faccessat2::from_raw(
            DirFd::new(4),
            "file",
            AccessMode::W_OK,
            FaccessatFlags::default(),
            -13,
        );
        assert_eq!(
            faccessat2.to_string(),
            "faccessat2(4, \"file\", W_OK, 0) = -1 EACCES"
        );
    }

    #[test]
    fn parse_success() {
        let faccessat2 =
            Faccessat2::from_str("faccessat2(AT_FDCWD, \"/etc/hostname\", R_OK, AT_EACCESS) = 0")
                .unwrap();
        assert_eq!(
            faccessat2,
            Faccessat2::from_raw(
                DirFd::AT_FDCWD,
                "/etc/hostname",
                AccessMode::R_OK,
                FaccessatFlags::AT_EACCESS,
                0,
            )
        );
    }

    #[test]
    fn parse_failure() {
        let faccessat2 =
            Faccessat2::from_str("faccessat2(4, \"file\", W_OK, 0) = -1 EACCES").unwrap();
        assert_eq!(faccessat2.dirfd, DirFd::new(4));
        assert_eq!(faccessat2.mode, AccessMode::W_OK);
        assert_eq!(faccessat2.flags, FaccessatFlags(0));
        assert_eq!(faccessat2.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Faccessat2::from_str("faccessat2(AT_FDCWD, \"/f\", R_OK, 0) 0").is_err());
        assert!(Faccessat2::from_str("faccessat2(AT_FDCWD, \"/f\", R_OK) = 0").is_err());
        assert!(Faccessat2::from_str("faccessat(AT_FDCWD, \"/f\", R_OK) = 0").is_err());
        assert!(Faccessat2::from_str("faccessat2(AT_FDCWD, \"/f\", R_OK, BOGUS) = 0").is_err());
        assert!(Faccessat2::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "faccessat2(AT_FDCWD, \"/etc/hostname\", R_OK, AT_EACCESS) = 0",
            "faccessat2(4, \"file\", W_OK, 0) = -1 EACCES",
            "faccessat2(AT_FDCWD, \"a, b\", F_OK, AT_SYMLINK_NOFOLLOW|AT_EMPTY_PATH) = -1 ENOENT",
        ] {
            assert_eq!(Faccessat2::from_str(line).unwrap().to_string(), line);
        }
    }
}
