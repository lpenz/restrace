// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The directory file descriptor argument of the `*at` system calls.

use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// A directory file descriptor, the `dirfd` argument of the `*at` system
/// calls such as [`openat(2)`](super::openat::Openat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct DirFd(pub i32);

impl DirFd {
    /// Use the current working directory of the calling process.
    pub const AT_FDCWD: Self = Self(-100);

    /// Returns true if this refers to the current working directory.
    #[must_use]
    pub const fn is_cwd(self) -> bool {
        self.0 == Self::AT_FDCWD.0
    }
}

impl From<i32> for DirFd {
    fn from(fd: i32) -> Self {
        Self(fd)
    }
}

impl fmt::Display for DirFd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_cwd() {
            f.write_str("AT_FDCWD")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Parses a directory descriptor as [`Display`](fmt::Display) writes it:
/// `AT_FDCWD` or a decimal descriptor.
impl FromStr for DirFd {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s == "AT_FDCWD" {
            return Ok(Self::AT_FDCWD);
        }
        s.parse::<i32>()
            .map(Self)
            .map_err(|_| ParseError::new("dirfd", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display() {
        assert_eq!(DirFd::AT_FDCWD.to_string(), "AT_FDCWD");
        assert_eq!(DirFd(-100).to_string(), "AT_FDCWD");
        assert_eq!(DirFd(3).to_string(), "3");
        assert!(DirFd::AT_FDCWD.is_cwd());
        assert!(!DirFd(3).is_cwd());
    }

    #[test]
    fn parse() {
        assert_eq!(DirFd::from_str("AT_FDCWD"), Ok(DirFd::AT_FDCWD));
        assert_eq!(DirFd::from_str("-100"), Ok(DirFd::AT_FDCWD));
        assert_eq!(DirFd::from_str("3"), Ok(DirFd(3)));
        assert!(DirFd::from_str("AT_FDC").is_err());
        assert!(DirFd::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for dirfd in [DirFd::AT_FDCWD, DirFd(0), DirFd(4096)] {
            assert_eq!(DirFd::from_str(&dirfd.to_string()).unwrap(), dirfd);
        }
    }
}
