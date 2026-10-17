// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Return values of the system calls that report a length in bytes.

use super::errno::Errno;
use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// The return value of a system call that reports how many bytes it
/// transferred, such as [`getdents(2)`](super::getdents::Getdents),
/// [`read(2)`](super::read::Read),
/// [`readv(2)`](super::readv::Readv),
/// [`pread64(2)`](super::pread64::Pread64),
/// [`preadv(2)`](super::preadv::Preadv),
/// [`preadv2(2)`](super::preadv2::Preadv2),
/// [`readlink(2)`](super::readlink::Readlink),
/// [`readlinkat(2)`](super::readlinkat::Readlinkat) and
/// [`write(2)`](super::write::Write).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LenResult {
    /// The call succeeded; it returned this many bytes.
    Len(u64),
    /// The call failed with this errno.
    Errno(Errno),
}

impl LenResult {
    /// Converts a raw return value into a [`LenResult`].
    ///
    /// The kernel reports errors as `-errno` in the range -1..=-4095;
    /// anything else is the number of bytes written.
    #[must_use]
    pub fn from_raw(raw: i64) -> Self {
        if (-4095..=-1).contains(&raw) {
            Self::Errno(Errno(-raw as i32))
        } else {
            Self::Len(raw.unsigned_abs())
        }
    }

    /// Returns the number of bytes, if the call succeeded.
    #[must_use]
    pub const fn byte_count(self) -> Option<u64> {
        match self {
            Self::Len(len) => Some(len),
            Self::Errno(_) => None,
        }
    }

    /// Returns true if the call succeeded.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Len(_))
    }
}

impl fmt::Display for LenResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Len(len) => write!(f, "{len}"),
            Self::Errno(errno) => write!(f, "-1 {errno}"),
        }
    }
}

/// Parses a return value as [`Display`](fmt::Display) writes it: a byte count,
/// or `-1` followed by the errno.
impl FromStr for LenResult {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("return value", s);
        if let Some(errno) = s.strip_prefix("-1 ") {
            return Ok(Self::Errno(
                Errno::from_str(parse::strip_description(errno.trim())).map_err(|_| error())?,
            ));
        }
        s.parse::<u64>().map(Self::Len).map_err(|_| error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw() {
        assert_eq!(LenResult::from_raw(0), LenResult::Len(0));
        assert_eq!(LenResult::from_raw(48), LenResult::Len(48));
        assert_eq!(LenResult::from_raw(-9), LenResult::Errno(Errno::EBADF));
        assert!(LenResult::from_raw(48).is_ok());
        assert!(!LenResult::from_raw(-9).is_ok());
        assert_eq!(LenResult::from_raw(48).byte_count(), Some(48));
        assert_eq!(LenResult::from_raw(-9).byte_count(), None);
    }

    #[test]
    fn display() {
        assert_eq!(LenResult::Len(48).to_string(), "48");
        assert_eq!(LenResult::Len(0).to_string(), "0");
        assert_eq!(LenResult::Errno(Errno::EBADF).to_string(), "-1 EBADF");
    }

    #[test]
    fn parse() {
        assert_eq!(LenResult::from_str("48"), Ok(LenResult::Len(48)));
        assert_eq!(LenResult::from_str("0"), Ok(LenResult::Len(0)));
        assert_eq!(
            LenResult::from_str("-1 EBADF"),
            Ok(LenResult::Errno(Errno::EBADF))
        );
        assert_eq!(
            LenResult::from_str("-1 EBADF (Bad file descriptor)"),
            Ok(LenResult::Errno(Errno::EBADF))
        );
        assert_eq!(
            LenResult::from_str("-1 9999"),
            Ok(LenResult::Errno(Errno(9999)))
        );
        assert!(LenResult::from_str("-1").is_err());
        assert!(LenResult::from_str("").is_err());
        assert!(LenResult::from_str("x").is_err());
    }

    #[test]
    fn round_trip() {
        for result in [
            LenResult::Len(0),
            LenResult::Len(48),
            LenResult::Errno(Errno::ENOENT),
            LenResult::Errno(Errno(9999)),
        ] {
            assert_eq!(LenResult::from_str(&result.to_string()).unwrap(), result);
        }
    }
}
