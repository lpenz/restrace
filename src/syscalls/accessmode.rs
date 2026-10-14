// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `access` mode argument of the access system calls.
//!
//! [`access(2)`]: https://man7.org/linux/man-pages/man2/access.2.html

use super::flags;
use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// Named bits of [`AccessMode`], in [`Display`](fmt::Display) order.
const NAMED: &[(u32, &str)] = &[(4, "R_OK"), (2, "W_OK"), (1, "X_OK")];

/// The `mode` argument of [`access(2)`](super::access::Access) and the
/// `*faccessat` calls: which permissions to test for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct AccessMode(pub u8);

impl AccessMode {
    /// Test for the existence of the file.
    pub const F_OK: Self = Self(0);
    /// Test for execute permission.
    pub const X_OK: Self = Self(1);
    /// Test for write permission.
    pub const W_OK: Self = Self(2);
    /// Test for read permission.
    pub const R_OK: Self = Self(4);

    /// Returns true if all bits of `bit` are set in this mode.
    #[must_use]
    pub const fn contains(self, bit: Self) -> bool {
        (self.0 & bit.0) == bit.0
    }
}

impl fmt::Display for AccessMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == Self::F_OK.0 {
            return f.write_str("F_OK");
        }
        flags::display(f, u32::from(self.0), NAMED)
    }
}

/// Parses a mode as [`Display`](fmt::Display) writes it: `F_OK`, or the names
/// of the tested permissions joined by `|`.
impl FromStr for AccessMode {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s == "F_OK" {
            return Ok(Self::F_OK);
        }
        let value = flags::parse(s, NAMED, "access mode")?;
        u8::try_from(value)
            .map(Self)
            .map_err(|_| ParseError::new("access mode", s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display() {
        assert_eq!(AccessMode::F_OK.to_string(), "F_OK");
        assert_eq!(AccessMode::R_OK.to_string(), "R_OK");
        assert_eq!(
            AccessMode(AccessMode::R_OK.0 | AccessMode::W_OK.0).to_string(),
            "R_OK|W_OK"
        );
    }

    #[test]
    fn contains() {
        assert!(AccessMode::R_OK.contains(AccessMode::R_OK));
        assert!(!AccessMode::R_OK.contains(AccessMode::W_OK));
        assert!(AccessMode::F_OK.contains(AccessMode::F_OK));
    }

    #[test]
    fn parse() {
        assert_eq!(AccessMode::from_str("F_OK"), Ok(AccessMode::F_OK));
        assert_eq!(AccessMode::from_str("R_OK"), Ok(AccessMode::R_OK));
        assert_eq!(
            AccessMode::from_str("R_OK|W_OK"),
            Ok(AccessMode(AccessMode::R_OK.0 | AccessMode::W_OK.0))
        );
        assert_eq!(AccessMode::from_str("0"), Ok(AccessMode::F_OK));
        assert!(AccessMode::from_str("X_BOGUS").is_err());
        assert!(AccessMode::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for mode in [
            AccessMode::F_OK,
            AccessMode::X_OK,
            AccessMode::W_OK,
            AccessMode::R_OK,
            AccessMode(AccessMode::R_OK.0 | AccessMode::W_OK.0),
        ] {
            assert_eq!(AccessMode::from_str(&mode.to_string()).unwrap(), mode);
        }
    }
}
