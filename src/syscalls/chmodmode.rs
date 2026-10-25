// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `mode_t` argument of `chmod(2)`, `fchmod(2)` and `fchmodat(2)`.

use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// The `mode_t` argument of `chmod(2)`, `fchmod(2)` and [`fchmodat(2)`](super::fchmodat::Fchmodat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ChmodMode(pub u32);

impl ChmodMode {
    /// Owner has read, write, and execute permission.
    pub const S_IRWXU: Self = Self(0o700);
    /// Owner has read permission.
    pub const S_IRUSR: Self = Self(0o400);
    /// Owner has write permission.
    pub const S_IWUSR: Self = Self(0o200);
    /// Owner has execute permission.
    pub const S_IXUSR: Self = Self(0o100);
    /// Group has read, write, and execute permission.
    pub const S_IRWXG: Self = Self(0o070);
    /// Group has read permission.
    pub const S_IRGRP: Self = Self(0o040);
    /// Group has write permission.
    pub const S_IWGRP: Self = Self(0o020);
    /// Group has execute permission.
    pub const S_IXGRP: Self = Self(0o010);
    /// Others have read, write, and execute permission.
    pub const S_IRWXO: Self = Self(0o007);
    /// Others have read permission.
    pub const S_IROTH: Self = Self(0o004);
    /// Others have write permission.
    pub const S_IWOTH: Self = Self(0o002);
    /// Others have execute permission.
    pub const S_IXOTH: Self = Self(0o001);
    /// Set-user-ID on execution.
    pub const S_ISUID: Self = Self(0o4000);
    /// Set-group-ID on execution.
    pub const S_ISGID: Self = Self(0o2000);
    /// Sticky bit.
    pub const S_ISVTX: Self = Self(0o1000);
}

impl fmt::Display for ChmodMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04o}", self.0)
    }
}

/// Parses a mode as [`Display`](fmt::Display) writes it: octal digits with a
/// leading zero, or with an explicit `0o` or `0x` prefix.
impl FromStr for ChmodMode {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("mode", s);
        if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            return Ok(Self(u32::from_str_radix(hex, 16).map_err(|_| error())?));
        }
        let octal = s
            .strip_prefix("0o")
            .or_else(|| s.strip_prefix("0O"))
            .unwrap_or(s);
        u32::from_str_radix(octal, 8).map(Self).map_err(|_| error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display() {
        assert_eq!(ChmodMode(0o644).to_string(), "0644");
        assert_eq!(ChmodMode(0o755).to_string(), "0755");
    }

    #[test]
    fn parse() {
        assert_eq!(ChmodMode::from_str("0644"), Ok(ChmodMode(0o644)));
        assert_eq!(ChmodMode::from_str("644"), Ok(ChmodMode(0o644)));
        assert_eq!(ChmodMode::from_str("0o644"), Ok(ChmodMode(0o644)));
        assert!(ChmodMode::from_str("899").is_err());
        assert!(ChmodMode::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for mode in [
            ChmodMode(0),
            ChmodMode(0o644),
            ChmodMode(0o755),
            ChmodMode(0o777),
        ] {
            assert_eq!(ChmodMode::from_str(&mode.to_string()).unwrap(), mode);
        }
    }
}
