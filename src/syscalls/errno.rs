// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! `errno` values, as returned by failed system calls.

use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// Names of the known `errno` constants, with their values.
const NAMES: &[(i32, &str)] = &[
    (1, "EPERM"),
    (2, "ENOENT"),
    (4, "EINTR"),
    (5, "EIO"),
    (9, "EBADF"),
    (11, "EAGAIN"),
    (13, "EACCES"),
    (17, "EEXIST"),
    (20, "ENOTDIR"),
    (21, "EISDIR"),
    (22, "EINVAL"),
    (23, "ENFILE"),
    (24, "EMFILE"),
    (28, "ENOSPC"),
    (30, "EROFS"),
    (32, "EPIPE"),
    (36, "ENAMETOOLONG"),
    (38, "ENOSYS"),
];

/// An `errno` value.
///
/// System calls report errors as a negated errno in their return value;
/// [`Errno::from_raw`] converts such a return value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Errno(pub i32);

impl Errno {
    /// Operation not permitted.
    pub const EPERM: Self = Self(1);
    /// No such file or directory.
    pub const ENOENT: Self = Self(2);
    /// Interrupted system call.
    pub const EINTR: Self = Self(4);
    /// I/O error.
    pub const EIO: Self = Self(5);
    /// Bad file descriptor.
    pub const EBADF: Self = Self(9);
    /// Permission denied.
    pub const EACCES: Self = Self(13);
    /// File exists.
    pub const EEXIST: Self = Self(17);
    /// Not a directory.
    pub const ENOTDIR: Self = Self(20);
    /// Is a directory.
    pub const EISDIR: Self = Self(21);
    /// Invalid argument.
    pub const EINVAL: Self = Self(22);
    /// File table overflow.
    pub const ENFILE: Self = Self(23);
    /// Too many open files.
    pub const EMFILE: Self = Self(24);
    /// No space left on device.
    pub const ENOSPC: Self = Self(28);
    /// Read-only file system.
    pub const EROFS: Self = Self(30);
    /// File name too long.
    pub const ENAMETOOLONG: Self = Self(36);
    /// Resource temporarily unavailable.
    pub const EAGAIN: Self = Self(11);
    /// Broken pipe.
    pub const EPIPE: Self = Self(32);
    /// Function not implemented.
    pub const ENOSYS: Self = Self(38);

    /// Converts a raw, negated system call return value into an [`Errno`].
    ///
    /// The kernel reports errors as `-errno` in the range -1..=-4095;
    /// magnitudes above 4095 are clamped to 4095.
    #[must_use]
    pub fn from_raw(raw: i64) -> Self {
        Self(raw.unsigned_abs().min(4095) as i32)
    }

    /// Returns the symbolic name of this errno, if known.
    #[must_use]
    pub fn name(self) -> Option<&'static str> {
        NAMES
            .iter()
            .find(|&&(code, _)| code == self.0)
            .map(|&(_, name)| name)
    }
}

impl fmt::Display for Errno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "{}", self.0),
        }
    }
}

/// Parses an errno from its symbolic name (`ENOENT`) or its decimal value
/// (`2`), as [`Display`](fmt::Display) writes them.  A leading `-` is
/// accepted and dropped, so that raw `-errno` return values parse too.
impl FromStr for Errno {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if let Some(&(code, _)) = NAMES.iter().find(|&&(_, name)| name == s) {
            return Ok(Self(code));
        }
        s.strip_prefix('-')
            .unwrap_or(s)
            .parse::<i32>()
            .map(Self)
            .map_err(|_| ParseError::new("errno", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_known() {
        assert_eq!(Errno::ENOENT.to_string(), "ENOENT");
        assert_eq!(Errno(2).to_string(), "ENOENT");
    }

    #[test]
    fn display_unknown() {
        assert_eq!(Errno(9999).to_string(), "9999");
    }

    #[test]
    fn from_raw() {
        assert_eq!(Errno::from_raw(-2), Errno::ENOENT);
        assert_eq!(Errno::from_raw(-4095), Errno(4095));
        assert_eq!(Errno::from_raw(-4096), Errno(4095));
    }

    #[test]
    fn name() {
        assert_eq!(Errno::ENOENT.name(), Some("ENOENT"));
        assert_eq!(Errno(3).name(), None);
    }

    #[test]
    fn parse_name() {
        assert_eq!(Errno::from_str("ENOENT"), Ok(Errno::ENOENT));
        assert_eq!(Errno::from_str("ENOSYS"), Ok(Errno::ENOSYS));
    }

    #[test]
    fn parse_number() {
        assert_eq!(Errno::from_str("2"), Ok(Errno::ENOENT));
        assert_eq!(Errno::from_str("-2"), Ok(Errno::ENOENT));
        assert_eq!(Errno::from_str("9999"), Ok(Errno(9999)));
    }

    #[test]
    fn parse_invalid() {
        assert_eq!(
            Errno::from_str("ENOSUCH"),
            Err(ParseError::new("errno", "ENOSUCH"))
        );
        assert!(Errno::from_str("").is_err());
        assert!(Errno::from_str("2x").is_err());
    }

    #[test]
    fn round_trip() {
        for &(code, name) in NAMES {
            let errno = Errno(code);
            assert_eq!(errno.to_string(), name);
            assert_eq!(Errno::from_str(name), Ok(errno));
        }
        assert_eq!(Errno::from_str(&Errno(9999).to_string()), Ok(Errno(9999)));
    }
}
