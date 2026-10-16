// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`newfstatat(2)`] system call.
//!
//! [`newfstatat(2)`]: https://man7.org/linux/man-pages/man2/newfstatat.2.html

use super::buffer::Buffer;
use super::dirfd::DirFd;
use super::flags;
use super::parse::{self, ParseError};
use super::result::OkResult;
use super::statbuf::StatBuf;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `newfstatat(2)` system call.
///
/// ```text
/// int newfstatat(int dirfd, const char *restrict pathname,
///                struct stat *restrict statbuf, int flags);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Newfstatat {
    /// Directory file descriptor.
    pub dirfd: DirFd,
    /// Pathname of the file to stat, without the terminating NUL byte.
    pub pathname: CString,
    /// The `struct stat` the call filled in, or the raw address of the buffer
    /// when `strace` could not decode it, such as after a failure.
    pub buf: Buffer<StatBuf>,
    /// The `at_flags` argument.
    pub flags: NewfstatatFlags,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Newfstatat {
    /// Builds a `newfstatat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw(
        dirfd: DirFd,
        pathname: impl AsRef<[u8]>,
        buf: Buffer<StatBuf>,
        flags: NewfstatatFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            buf,
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Newfstatat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "newfstatat({}, {:?}, {}, {}) = {}",
            self.dirfd,
            self.pathname.to_string_lossy(),
            self.buf,
            self.flags,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `newfstatat(AT_FDCWD, "path", {st_dev=..., ...}, AT_SYMLINK_NOFOLLOW) = 0`.
impl FromStr for Newfstatat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("newfstatat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("newfstatat(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("newfstatat arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        let buf = super::buffer::parse(parts[2].trim())?;
        let flags = NewfstatatFlags::from_str(parts[3].trim())?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
            buf,
            flags,
            result: OkResult::from_str(result)?,
        })
    }
}

/// Flags argument of the `newfstatat(2)` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct NewfstatatFlags(pub u32);

impl fmt::Display for NewfstatatFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMES, "AT_???")
    }
}

impl FromStr for NewfstatatFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        Ok(Self(flags::parse(s, NAMES, "newfstatat flags")?))
    }
}

/// Flag names of [`NewfstatatFlags`].
///
/// These are the names of strace's shared `at_flags` table, which it uses
/// when printing the flags of `newfstatat(2)`.
const NAMES: &[(u32, &str)] = &[
    (0x100, "AT_SYMLINK_NOFOLLOW"),
    (0x200, "AT_REMOVEDIR"),
    (0x400, "AT_SYMLINK_FOLLOW"),
    (0x800, "AT_NO_AUTOMOUNT"),
    (0x1000, "AT_EMPTY_PATH"),
    (0x8000, "AT_RECURSIVE"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn flags_display() {
        assert_eq!(NewfstatatFlags(0).to_string(), "0");
        assert_eq!(NewfstatatFlags(0x100).to_string(), "AT_SYMLINK_NOFOLLOW");
        assert_eq!(NewfstatatFlags(0x200).to_string(), "AT_REMOVEDIR");
        assert_eq!(NewfstatatFlags(0x400).to_string(), "AT_SYMLINK_FOLLOW");
        assert_eq!(NewfstatatFlags(0x800).to_string(), "AT_NO_AUTOMOUNT");
        assert_eq!(NewfstatatFlags(0x1000).to_string(), "AT_EMPTY_PATH");
        assert_eq!(NewfstatatFlags(0x8000).to_string(), "AT_RECURSIVE");
        assert_eq!(
            NewfstatatFlags(0x100 | 0x1000).to_string(),
            "AT_SYMLINK_NOFOLLOW|AT_EMPTY_PATH"
        );
    }

    #[test]
    fn flags_parse() {
        assert_eq!(NewfstatatFlags::from_str("0"), Ok(NewfstatatFlags(0)));
        assert_eq!(
            NewfstatatFlags::from_str("AT_REMOVEDIR|AT_SYMLINK_FOLLOW"),
            Ok(NewfstatatFlags(0x200 | 0x400))
        );
        assert_eq!(
            NewfstatatFlags::from_str("AT_NO_AUTOMOUNT"),
            Ok(NewfstatatFlags(0x800))
        );
        assert_eq!(
            NewfstatatFlags::from_str("0x8000"),
            Ok(NewfstatatFlags(0x8000))
        );
        assert!(NewfstatatFlags::from_str("AT_NOPE").is_err());
    }

    #[test]
    fn display_success() {
        let call = Newfstatat::from_raw(
            DirFd::AT_FDCWD,
            "/etc/hostname",
            super::super::buffer::Buffer::Address(0x7ffd539f6650),
            NewfstatatFlags(0x100),
            0,
        );
        assert_eq!(
            call.to_string(),
            "newfstatat(AT_FDCWD, \"/etc/hostname\", 0x7ffd539f6650, AT_SYMLINK_NOFOLLOW) = 0"
        );
    }

    #[test]
    fn display_failure() {
        let call = Newfstatat::from_raw(
            DirFd::AT_FDCWD,
            "/etc/hostname",
            super::super::buffer::Buffer::Address(0),
            NewfstatatFlags(0x200),
            -22,
        );
        assert_eq!(
            call.to_string(),
            "newfstatat(AT_FDCWD, \"/etc/hostname\", NULL, AT_REMOVEDIR) = -1 EINVAL"
        );
    }

    #[test]
    fn parse_success() {
        let line = "newfstatat(AT_FDCWD, \"/etc/hostname\", NULL, AT_EMPTY_PATH) = -1 ENOENT";
        let call = Newfstatat::from_str(line).unwrap();
        assert_eq!(call.dirfd, DirFd::AT_FDCWD);
        assert_eq!(call.pathname.to_str().unwrap(), "/etc/hostname");
        assert_eq!(call.buf, super::super::buffer::Buffer::Address(0));
        assert_eq!(call.flags, NewfstatatFlags(0x1000));
        assert_eq!(call.result, OkResult::Errno(Errno::ENOENT));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Newfstatat::from_str("newfstatat(AT_FDCWD, \"/a\") = 0").is_err());
        assert!(Newfstatat::from_str("newfstatat(AT_FDCWD, \"/a\", NULL, AT_NOPE) = 0").is_err());
        assert!(Newfstatat::from_str("stat(\"/a\", NULL) = 0").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "newfstatat(AT_FDCWD, \"/etc/hostname\", NULL, 0) = -1 ENOENT",
            "newfstatat(AT_FDCWD, \"/etc/hostname\", NULL, AT_SYMLINK_NOFOLLOW) = -1 EINVAL",
            "newfstatat(AT_FDCWD, \"/etc/hostname\", NULL, AT_REMOVEDIR) = -1 EINVAL",
            "newfstatat(AT_FDCWD, \"/etc/hostname\", NULL, AT_SYMLINK_FOLLOW) = -1 EINVAL",
            "newfstatat(AT_FDCWD, \"/etc/hostname\", NULL, AT_NO_AUTOMOUNT) = -1 ENOENT",
            "newfstatat(AT_FDCWD, \"/etc/hostname\", NULL, AT_EMPTY_PATH) = -1 ENOENT",
            "newfstatat(AT_FDCWD, \"/etc/hostname\", NULL, AT_RECURSIVE) = -1 EINVAL",
            "newfstatat(AT_FDCWD, \"/etc/hostname\", NULL, AT_SYMLINK_NOFOLLOW|AT_EMPTY_PATH) = -1 ENOENT",
        ] {
            assert_eq!(Newfstatat::from_str(line).unwrap().to_string(), line);
        }
    }
}
