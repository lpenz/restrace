// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `uid_t` and `gid_t` arguments of the `chown(2)` family.

use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// A user or group ID argument, the `uid_t` and `gid_t` of
/// [`chown(2)`](super::chown::Chown) and its variants.
///
/// The value is unsigned. `strace` prints the special `(uid_t) -1` value that
/// leaves the owner or group unchanged as `-1`, which [`NO_CHANGE`](Self::NO_CHANGE)
/// represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Uid(pub u32);

impl Uid {
    /// The value that leaves the owner or group unchanged, `(uid_t) -1`.
    pub const NO_CHANGE: Self = Self(u32::MAX);
}

impl fmt::Display for Uid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::NO_CHANGE {
            f.write_str("-1")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Parses an ID as [`Display`](fmt::Display) writes it: a decimal value, or
/// `-1` for [`Uid::NO_CHANGE`].
impl FromStr for Uid {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s == "-1" {
            return Ok(Self::NO_CHANGE);
        }
        s.parse::<u32>()
            .map(Self)
            .map_err(|_| ParseError::new("uid", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display() {
        assert_eq!(Uid(0).to_string(), "0");
        assert_eq!(Uid(1000).to_string(), "1000");
        assert_eq!(Uid::NO_CHANGE.to_string(), "-1");
    }

    #[test]
    fn parse() {
        assert_eq!(Uid::from_str("0"), Ok(Uid(0)));
        assert_eq!(Uid::from_str("1000"), Ok(Uid(1000)));
        assert_eq!(Uid::from_str("-1"), Ok(Uid::NO_CHANGE));
        assert_eq!(Uid::from_str("4294967294"), Ok(Uid(4294967294)));
        assert!(Uid::from_str("4294967296").is_err());
        assert!(Uid::from_str("-2").is_err());
        assert!(Uid::from_str("root").is_err());
        assert!(Uid::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for uid in [Uid(0), Uid(1000), Uid(4294967294), Uid::NO_CHANGE] {
            assert_eq!(Uid::from_str(&uid.to_string()).unwrap(), uid);
        }
    }
}
