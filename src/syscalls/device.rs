// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `dev_t` device number of a `stat(2)` result.

use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// A device number, as reported in the `st_dev` and `st_rdev` fields of a
/// `struct stat`.
///
/// Strace prints it as the `makedev(2)` call that builds it from a major and
/// a minor number, both in hexadecimal with `%#x` semantics, where zero is
/// printed as `0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Device {
    /// The device major number.
    pub major: u32,
    /// The device minor number.
    pub minor: u32,
}

impl Device {
    /// Builds a device number from its major and minor numbers.
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}

/// Writes a device number part with `%#x` semantics: zero is `0`, everything
/// else is printed in hexadecimal with a `0x` prefix.
fn write_part(f: &mut fmt::Formatter<'_>, value: u32) -> fmt::Result {
    if value == 0 {
        f.write_str("0")
    } else {
        write!(f, "{value:#x}")
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("makedev(")?;
        write_part(f, self.major)?;
        f.write_str(", ")?;
        write_part(f, self.minor)?;
        f.write_str(")")
    }
}

/// Parses a part of a device number: hexadecimal with an optional `0x` prefix,
/// or decimal.
fn parse_part(token: &str) -> Result<u32, ParseError> {
    let token = token.trim();
    let error = || ParseError::new("device", token);
    if let Some(hex) = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).map_err(|_| error())
    } else {
        token.parse::<u32>().map_err(|_| error())
    }
}

/// Parses a device as [`Display`](fmt::Display) writes it:
/// `makedev(MAJOR, MINOR)`.
impl FromStr for Device {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("device", s);
        let body = s
            .strip_prefix("makedev(")
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(error)?;
        let (major, minor) = body.split_once(',').ok_or_else(error)?;
        Ok(Self {
            major: parse_part(major)?,
            minor: parse_part(minor)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_zero() {
        assert_eq!(Device::new(0, 0).to_string(), "makedev(0, 0)");
    }

    #[test]
    fn display_hex() {
        assert_eq!(Device::new(1, 3).to_string(), "makedev(0x1, 0x3)");
        assert_eq!(Device::new(0, 0x1e).to_string(), "makedev(0, 0x1e)");
    }

    #[test]
    fn parse() {
        assert_eq!(Device::from_str("makedev(0x1, 0x3)"), Ok(Device::new(1, 3)));
        assert_eq!(
            Device::from_str("makedev(0, 0x1e)"),
            Ok(Device::new(0, 0x1e))
        );
        assert_eq!(Device::from_str("makedev(0, 3)"), Ok(Device::new(0, 3)));
        assert!(Device::from_str("makedev(1)").is_err());
        assert!(Device::from_str("makedev(a, b)").is_err());
        assert!(Device::from_str("1, 3").is_err());
        assert!(Device::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for device in [Device::new(0, 0), Device::new(1, 3), Device::new(0, 0x1e)] {
            assert_eq!(Device::from_str(&device.to_string()).unwrap(), device);
        }
    }
}
