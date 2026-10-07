// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Return values of the system calls that report success as zero.

use super::errno::Errno;
use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// The return value of a system call that succeeds by returning 0, such as
/// [`close(2)`](super::close::Close), [`chdir(2)`](super::chdir::Chdir) and
/// [`fchdir(2)`](super::fchdir::Fchdir).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OkResult {
    /// The call succeeded; it returned 0.
    Ok,
    /// The call failed with this errno.
    Errno(Errno),
}

impl OkResult {
    /// Converts a raw return value into an [`OkResult`].
    ///
    /// The kernel reports errors as `-errno` in the range -1..=-4095;
    /// anything else counts as success, as these calls only report 0.
    #[must_use]
    pub fn from_raw(raw: i64) -> Self {
        if (-4095..=-1).contains(&raw) {
            Self::Errno(Errno(-raw as i32))
        } else {
            Self::Ok
        }
    }

    /// Returns true if the call succeeded.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }
}

impl fmt::Display for OkResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => f.write_str("0"),
            Self::Errno(errno) => write!(f, "-1 {errno}"),
        }
    }
}

/// Parses a return value as [`Display`](fmt::Display) writes it: 0, or `-1`
/// followed by the errno.
impl FromStr for OkResult {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("return value", s);
        if let Some(errno) = s.strip_prefix("-1 ") {
            return Ok(Self::Errno(
                Errno::from_str(errno.trim()).map_err(|_| error())?,
            ));
        }
        if s == "0" {
            return Ok(Self::Ok);
        }
        Err(error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw() {
        assert_eq!(OkResult::from_raw(0), OkResult::Ok);
        assert_eq!(OkResult::from_raw(-9), OkResult::Errno(Errno::EBADF));
        assert_eq!(OkResult::from_raw(4096), OkResult::Ok);
        assert!(OkResult::from_raw(0).is_ok());
        assert!(!OkResult::from_raw(-9).is_ok());
    }

    #[test]
    fn parse() {
        assert_eq!(OkResult::from_str("0"), Ok(OkResult::Ok));
        assert_eq!(
            OkResult::from_str("-1 EBADF"),
            Ok(OkResult::Errno(Errno::EBADF))
        );
        assert_eq!(
            OkResult::from_str("-1 9999"),
            Ok(OkResult::Errno(Errno(9999)))
        );
        assert!(OkResult::from_str("1").is_err());
        assert!(OkResult::from_str("-1").is_err());
        assert!(OkResult::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for result in [
            OkResult::Ok,
            OkResult::Errno(Errno::ENOENT),
            OkResult::Errno(Errno(9999)),
        ] {
            assert_eq!(OkResult::from_str(&result.to_string()).unwrap(), result);
        }
    }
}
