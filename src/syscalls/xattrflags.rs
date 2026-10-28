// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `flags` argument of the `setxattr(2)` family.
//!
//! [`setxattr(2)`]: https://man7.org/linux/man-pages/man2/setxattr.2.html

use super::flags;
use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// Named bits of [`XattrFlags`], in [`Display`](fmt::Display) order.
const NAMED_FLAGS: &[(u32, &str)] = &[
    (XattrFlags::XATTR_CREATE.0, "XATTR_CREATE"),
    (XattrFlags::XATTR_REPLACE.0, "XATTR_REPLACE"),
];

/// The `flags` argument of [`setxattr(2)`](super::setxattr::Setxattr),
/// [`fsetxattr(2)`](super::fsetxattr::Fsetxattr) and
/// [`lsetxattr(2)`](super::lsetxattr::Lsetxattr).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct XattrFlags(pub u32);

impl XattrFlags {
    /// Fail if the extended attribute already exists.
    pub const XATTR_CREATE: Self = Self(1);
    /// Fail if the extended attribute does not already exist.
    pub const XATTR_REPLACE: Self = Self(2);
}

impl fmt::Display for XattrFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_FLAGS, "XATTR_???")
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them.
impl FromStr for XattrFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        flags::parse(s, NAMED_FLAGS, "flags").map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display() {
        assert_eq!(XattrFlags(0).to_string(), "0");
        assert_eq!(XattrFlags::XATTR_CREATE.to_string(), "XATTR_CREATE");
        assert_eq!(XattrFlags::XATTR_REPLACE.to_string(), "XATTR_REPLACE");
        assert_eq!(
            XattrFlags(XattrFlags::XATTR_CREATE.0 | XattrFlags::XATTR_REPLACE.0).to_string(),
            "XATTR_CREATE|XATTR_REPLACE"
        );
    }

    #[test]
    fn display_unknown() {
        assert_eq!(XattrFlags(0x4).to_string(), "0x4 /* XATTR_??? */");
        assert_eq!(
            XattrFlags(XattrFlags::XATTR_CREATE.0 | 0x4).to_string(),
            "XATTR_CREATE|0x4"
        );
    }

    #[test]
    fn parse() {
        assert_eq!(XattrFlags::from_str("0"), Ok(XattrFlags(0)));
        assert_eq!(
            XattrFlags::from_str("XATTR_CREATE"),
            Ok(XattrFlags::XATTR_CREATE)
        );
        assert_eq!(
            XattrFlags::from_str("XATTR_CREATE|XATTR_REPLACE"),
            Ok(XattrFlags(3))
        );
        assert_eq!(
            XattrFlags::from_str("0x4 /* XATTR_??? */"),
            Ok(XattrFlags(4))
        );
        assert!(XattrFlags::from_str("XATTR_BOGUS").is_err());
        assert!(XattrFlags::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for flags in [
            XattrFlags(0),
            XattrFlags::XATTR_CREATE,
            XattrFlags::XATTR_REPLACE,
            XattrFlags(3),
            XattrFlags(4),
            XattrFlags(5),
        ] {
            assert_eq!(XattrFlags::from_str(&flags.to_string()).unwrap(), flags);
        }
    }
}
