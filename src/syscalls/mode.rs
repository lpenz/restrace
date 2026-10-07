// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `mode_t` file creation mode.

use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// The `mode_t` argument of `open(2)` and [`creat(2)`](super::creat::Creat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Mode(pub u32);

impl Mode {
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

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04o}", self.0)
    }
}

/// Parses a mode as [`Display`](fmt::Display) writes it: octal digits with a
/// leading zero, or with an explicit `0o` or `0x` prefix.
impl FromStr for Mode {
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
        assert_eq!(Mode::S_IRUSR.to_string(), "0400");
        assert_eq!(Mode(0o644).to_string(), "0644");
    }

    #[test]
    fn parse() {
        assert_eq!(Mode::from_str("0644"), Ok(Mode(0o644)));
        assert_eq!(Mode::from_str("644"), Ok(Mode(0o644)));
        assert_eq!(Mode::from_str("0o644"), Ok(Mode(0o644)));
        assert_eq!(Mode::from_str("0x1ff"), Ok(Mode(0o777)));
        assert!(Mode::from_str("899").is_err());
        assert!(Mode::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for mode in [Mode(0), Mode(0o644), Mode(0o7777)] {
            assert_eq!(Mode::from_str(&mode.to_string()).unwrap(), mode);
        }
    }
}
