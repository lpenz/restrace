// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The `renameat2` system call.
//!
//! [man7]: https://man7.org/linux/man-pages/man2/renameat2.2.html

use super::dirfd::DirFd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
use std::str::FromStr;

/// Flags for `renameat2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct Renameat2Flags(pub u32);

impl Renameat2Flags {
    /// Don't overwrite the destination if it exists.
    pub const RENAME_NOREPLACE: Self = Self(0x1);
    /// Atomically exchange the old and new paths.
    pub const RENAME_EXCHANGE: Self = Self(0x2);
    /// Move the source to the destination; if source is a directory, don't
    /// allow it to be moved across filesystems.
    pub const RENAME_WHITEOUT: Self = Self(0x4);

    const KNOWN_MASK: u32 =
        Self::RENAME_NOREPLACE.0 | Self::RENAME_EXCHANGE.0 | Self::RENAME_WHITEOUT.0;

    /// Returns true if all bits of `flag` are set in these flags.
    #[must_use]
    pub const fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) == flag.0
    }

    /// Returns the bits of these flags that no known flag covers.
    #[must_use]
    pub const fn unknown_bits(self) -> u32 {
        self.0 & !Self::KNOWN_MASK
    }
}

impl BitOr for Renameat2Flags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Renameat2Flags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for Renameat2Flags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for Renameat2Flags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for Renameat2Flags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

const RENAMEAT2_FLAGS: &[(Renameat2Flags, &str)] = &[
    (Renameat2Flags::RENAME_NOREPLACE, "RENAME_NOREPLACE"),
    (Renameat2Flags::RENAME_EXCHANGE, "RENAME_EXCHANGE"),
    (Renameat2Flags::RENAME_WHITEOUT, "RENAME_WHITEOUT"),
];

impl fmt::Display for Renameat2Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        let mut write_part = |part: &str| -> fmt::Result {
            if !first {
                f.write_str("|")?;
            }
            first = false;
            f.write_str(part)
        };
        if self.0 == 0 {
            return f.write_str("0");
        }
        for (flag, name) in RENAMEAT2_FLAGS {
            if self.contains(*flag) {
                write_part(name)?;
            }
        }
        let unknown = self.unknown_bits();
        if unknown != 0 {
            write_part(&format!("0x{unknown:x}"))?;
        }
        Ok(())
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them: flag names joined
/// by `|`, or `0` for no flags; supports `0x` prefixes for unknown bits.
impl FromStr for Renameat2Flags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseError::new("flags", s));
        }
        if s == "0" {
            return Ok(Self::default());
        }
        let mut flags = Self::default();
        for token in s.split('|') {
            let token = token.trim();
            let named = RENAMEAT2_FLAGS
                .iter()
                .find(|&&(_, name)| name == token)
                .map(|&(flag, _)| flag);
            match named {
                Some(flag) => flags |= flag,
                None if token.starts_with("0x") || token.starts_with("0X") => {
                    let hex = token.trim_start_matches("0x").trim_start_matches("0X");
                    let val = u32::from_str_radix(hex, 16)
                        .map_err(|_| ParseError::new("flags", token))?;
                    flags |= Self(val);
                }
                None => {
                    let val = token
                        .parse::<u32>()
                        .map_err(|_| ParseError::new("flags", token))?;
                    flags |= Self(val);
                }
            }
        }
        Ok(flags)
    }
}

/// Arguments and return value of the `renameat2` system call.
///
/// ```text
/// int renameat2(int olddirfd, const char *oldpath, int newdirfd, const char *newpath, unsigned int flags);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renameat2 {
    /// Directory against which the old pathname is resolved.
    pub olddirfd: DirFd,
    /// Old pathname, without the terminating NUL byte.
    ///
    /// Interpreted relative to `olddirfd` unless it is absolute.
    pub oldpath: CString,
    /// Directory against which the new pathname is resolved.
    pub newdirfd: DirFd,
    /// New pathname, without the terminating NUL byte.
    ///
    /// Interpreted relative to `newdirfd` unless it is absolute.
    pub newpath: CString,
    /// Flags controlling the operation.
    pub flags: Renameat2Flags,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Renameat2 {
    /// Builds a `renameat2(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if pathnames contain an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>, Q: AsRef<[u8]>>(
        olddirfd: DirFd,
        oldpath: P,
        newdirfd: DirFd,
        newpath: Q,
        flags: Renameat2Flags,
        raw_result: i64,
    ) -> Self {
        Self {
            olddirfd,
            oldpath: CString::new(oldpath.as_ref()).expect("pathname contains NUL byte"),
            newdirfd,
            newpath: CString::new(newpath.as_ref()).expect("pathname contains NUL byte"),
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Renameat2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "renameat2({}, {:?}, {}, {:?}, {}) = {}",
            self.olddirfd,
            self.oldpath.to_string_lossy(),
            self.newdirfd,
            self.newpath.to_string_lossy(),
            self.flags,
            self.result
        )
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `renameat2(AT_FDCWD, "old", AT_FDCWD, "new", flags) = 0`.
impl FromStr for Renameat2 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("renameat2 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("renameat2(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() != 5 {
            return Err(ParseError::new("renameat2 arguments", args));
        }
        let olddirfd = DirFd::from_str(parts[0].trim())?;
        let oldpath = parse::unquote(parts[1].trim(), "pathname")?;
        let newdirfd = DirFd::from_str(parts[2].trim())?;
        let newpath = parse::unquote(parts[3].trim(), "pathname")?;
        let flags = Renameat2Flags::from_str(parts[4].trim())?;
        Ok(Self {
            olddirfd,
            oldpath: CString::new(oldpath).map_err(|_| ParseError::new("pathname", parts[1]))?,
            newdirfd,
            newpath: CString::new(newpath).map_err(|_| ParseError::new("pathname", parts[3]))?,
            flags,
            result: OkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn flags_display() {
        assert_eq!(Renameat2Flags::default().to_string(), "0");
        assert_eq!(
            (Renameat2Flags::RENAME_NOREPLACE | Renameat2Flags::RENAME_EXCHANGE).to_string(),
            "RENAME_NOREPLACE|RENAME_EXCHANGE"
        );
        assert_eq!(Renameat2Flags(0x100).to_string(), "0x100");
        assert_eq!(
            (Renameat2Flags::RENAME_WHITEOUT | Renameat2Flags(0x8)).to_string(),
            "RENAME_WHITEOUT|0x8"
        );
    }

    #[test]
    fn flags_parse() {
        assert_eq!(Renameat2Flags::from_str("0"), Ok(Renameat2Flags::default()));
        assert_eq!(
            Renameat2Flags::from_str("RENAME_NOREPLACE"),
            Ok(Renameat2Flags::RENAME_NOREPLACE)
        );
        assert_eq!(
            Renameat2Flags::from_str("RENAME_NOREPLACE|RENAME_EXCHANGE"),
            Ok(Renameat2Flags::RENAME_NOREPLACE | Renameat2Flags::RENAME_EXCHANGE)
        );
        assert_eq!(
            Renameat2Flags::from_str("0x1"),
            Ok(Renameat2Flags::RENAME_NOREPLACE)
        );
        assert_eq!(Renameat2Flags::from_str("0x100"), Ok(Renameat2Flags(0x100)));
        assert!(Renameat2Flags::from_str("RENAME_BOGUS").is_err());
        assert!(Renameat2Flags::from_str("").is_err());
    }

    #[test]
    fn display_success() {
        let renameat2 = Renameat2::from_raw(
            DirFd::AT_FDCWD,
            "/a",
            DirFd::AT_FDCWD,
            "/b",
            Renameat2Flags::RENAME_NOREPLACE,
            0,
        );
        assert_eq!(
            renameat2.to_string(),
            "renameat2(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\", RENAME_NOREPLACE) = 0"
        );
        assert!(renameat2.flags.contains(Renameat2Flags::RENAME_NOREPLACE));
    }

    #[test]
    fn display_zero_flags() {
        let renameat2 = Renameat2::from_raw(
            DirFd::AT_FDCWD,
            "/a",
            DirFd::AT_FDCWD,
            "/b",
            Renameat2Flags::default(),
            0,
        );
        assert_eq!(
            renameat2.to_string(),
            "renameat2(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\", 0) = 0"
        );
    }

    #[test]
    fn parse_success() {
        let renameat2 = Renameat2::from_str(
            "renameat2(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\", RENAME_EXCHANGE) = 0",
        )
        .unwrap();
        assert_eq!(
            renameat2,
            Renameat2::from_raw(
                DirFd::AT_FDCWD,
                "/a",
                DirFd::AT_FDCWD,
                "/b",
                Renameat2Flags::RENAME_EXCHANGE,
                0
            )
        );
        assert!(renameat2.flags.contains(Renameat2Flags::RENAME_EXCHANGE));
    }

    #[test]
    fn parse_zero_flags_and_failure() {
        let renameat2 =
            Renameat2::from_str("renameat2(3, \"a\", 4, \"b\", 0) = -1 EACCES").unwrap();
        assert_eq!(renameat2.olddirfd, DirFd(3));
        assert_eq!(renameat2.newdirfd, DirFd(4));
        assert_eq!(renameat2.flags, Renameat2Flags::default());
        assert_eq!(renameat2.result, OkResult::Errno(Errno::EACCES));
    }

    #[test]
    fn parse_invalid() {
        assert!(Renameat2::from_str("renameat2(AT_FDCWD, \"a\", AT_FDCWD, \"b\", 0) 0").is_err());
        assert!(Renameat2::from_str("renameat2(AT_FDCWD, \"a\", AT_FDCWD, \"b\") = 0").is_err());
        assert!(
            Renameat2::from_str("renameat2(AT_FDCWD, \"a\", AT_FDCWD, \"b\", BOGUS) = 0").is_err()
        );
        assert!(Renameat2::from_str("renameat(AT_FDCWD, \"a\", AT_FDCWD, \"b\") = 0").is_err());
        assert!(Renameat2::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "renameat2(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\", 0) = 0",
            "renameat2(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\", RENAME_NOREPLACE) = 0",
            "renameat2(AT_FDCWD, \"/a\", AT_FDCWD, \"/b\", RENAME_NOREPLACE|RENAME_EXCHANGE) = 0",
            "renameat2(3, \"a\", 4, \"b\", RENAME_WHITEOUT|0x8) = -1 EINVAL",
        ] {
            assert_eq!(Renameat2::from_str(line).unwrap().to_string(), line);
        }
    }
}
