// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`execveat(2)`] system call.
//!
//! [`execveat(2)`]: https://man7.org/linux/man-pages/man2/execveat.2.html

use super::dirfd::DirFd;
use super::execve::{parse_str_array, write_str_array};
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
use std::str::FromStr;

/// Flags for `execveat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct ExecveatFlags(pub u32);

impl ExecveatFlags {
    /// Don't follow a trailing symbolic link in `pathname`.
    pub const AT_SYMLINK_NOFOLLOW: Self = Self(0x100);
    /// Allow `pathname` to be an empty string, referring to `dirfd` itself.
    pub const AT_EMPTY_PATH: Self = Self(0x1000);

    const KNOWN_MASK: u32 = Self::AT_SYMLINK_NOFOLLOW.0 | Self::AT_EMPTY_PATH.0;

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

impl BitOr for ExecveatFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ExecveatFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ExecveatFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for ExecveatFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for ExecveatFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

const EXECVEAT_FLAGS: &[(ExecveatFlags, &str)] = &[
    (ExecveatFlags::AT_SYMLINK_NOFOLLOW, "AT_SYMLINK_NOFOLLOW"),
    (ExecveatFlags::AT_EMPTY_PATH, "AT_EMPTY_PATH"),
];

impl fmt::Display for ExecveatFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return f.write_str("0");
        }
        let mut first = true;
        let mut write_part = |part: &str| -> fmt::Result {
            if !first {
                f.write_str("|")?;
            }
            first = false;
            f.write_str(part)
        };
        for (flag, name) in EXECVEAT_FLAGS {
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
impl FromStr for ExecveatFlags {
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
            let named = EXECVEAT_FLAGS
                .iter()
                .find(|&&(_, name)| name == token)
                .map(|&(flag, _)| flag);
            match named {
                Some(flag) => flags |= flag,
                None => {
                    let val = if let Some(hex) = token
                        .strip_prefix("0x")
                        .or_else(|| token.strip_prefix("0X"))
                    {
                        u32::from_str_radix(hex, 16).map_err(|_| ParseError::new("flags", token))?
                    } else {
                        token
                            .parse::<u32>()
                            .map_err(|_| ParseError::new("flags", token))?
                    };
                    flags |= Self(val);
                }
            }
        }
        Ok(flags)
    }
}

/// Arguments and return value of the `execveat(2)` system call.
///
/// ```text
/// int execveat(int dirfd, const char *pathname, char *const argv[],
///              char *const envp[], int flags);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execveat {
    /// Directory against which the pathname is resolved, or [`DirFd::AT_FDCWD`].
    pub dirfd: DirFd,
    /// Pathname of the program to run, without the terminating NUL byte.
    ///
    /// Interpreted relative to `dirfd` unless it is absolute.
    pub pathname: CString,
    /// The argument vector, the `argv` argument.
    pub argv: Vec<CString>,
    /// The environment, the `envp` argument.
    pub envp: Vec<CString>,
    /// Flags controlling `pathname` resolution.
    pub flags: ExecveatFlags,
    /// Return value: zero on success, or the errno of the failure.
    ///
    /// On success the calling process is replaced and execveat does not
    /// return; strace still reports `= 0`.
    pub result: OkResult,
}

impl Execveat {
    /// Builds an `execveat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` or any array entry contains an interior NUL byte,
    /// which cannot happen for strings read from a tracee.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(
        dirfd: DirFd,
        pathname: P,
        argv: Vec<CString>,
        envp: Vec<CString>,
        flags: ExecveatFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            argv,
            envp,
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Execveat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "execveat({}, {:?}, ",
            self.dirfd,
            self.pathname.to_string_lossy()
        )?;
        write_str_array(f, &self.argv)?;
        f.write_str(", ")?;
        write_str_array(f, &self.envp)?;
        write!(f, ", {}) = {}", self.flags, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `execveat(dirfd, "path", ["argv"], ["env"], flags) = 0`.
impl FromStr for Execveat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("execveat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("execveat(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 5 {
            return Err(ParseError::new("execveat arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        let argv = parse_str_array(parts[2], "argv")?;
        let envp = parse_str_array(parts[3], "envp")?;
        let flags = ExecveatFlags::from_str(parts[4].trim())?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
            argv,
            envp,
            flags,
            result: OkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn flags_display() {
        assert_eq!(ExecveatFlags::default().to_string(), "0");
        assert_eq!(
            (ExecveatFlags::AT_SYMLINK_NOFOLLOW | ExecveatFlags::AT_EMPTY_PATH).to_string(),
            "AT_SYMLINK_NOFOLLOW|AT_EMPTY_PATH"
        );
        assert_eq!(ExecveatFlags(0x40).to_string(), "0x40");
    }

    #[test]
    fn flags_parse() {
        assert_eq!(ExecveatFlags::from_str("0"), Ok(ExecveatFlags::default()));
        assert_eq!(
            ExecveatFlags::from_str("AT_EMPTY_PATH"),
            Ok(ExecveatFlags::AT_EMPTY_PATH)
        );
        assert_eq!(
            ExecveatFlags::from_str("AT_SYMLINK_NOFOLLOW|AT_EMPTY_PATH"),
            Ok(ExecveatFlags::AT_SYMLINK_NOFOLLOW | ExecveatFlags::AT_EMPTY_PATH)
        );
        assert_eq!(
            ExecveatFlags::from_str("0x1000"),
            Ok(ExecveatFlags::AT_EMPTY_PATH)
        );
        assert_eq!(ExecveatFlags::from_str("0x40"), Ok(ExecveatFlags(0x40)));
        assert!(ExecveatFlags::from_str("AT_BOGUS").is_err());
        assert!(ExecveatFlags::from_str("").is_err());
    }

    #[test]
    fn display_success() {
        let execveat = Execveat::from_raw(
            DirFd::AT_FDCWD,
            "./prog",
            vec![c("./prog"), c("--help")],
            vec![c("PATH=/bin")],
            ExecveatFlags::AT_EMPTY_PATH,
            0,
        );
        assert_eq!(
            execveat.to_string(),
            "execveat(AT_FDCWD, \"./prog\", [\"./prog\", \"--help\"], [\"PATH=/bin\"], AT_EMPTY_PATH) = 0"
        );
        assert!(execveat.flags.contains(ExecveatFlags::AT_EMPTY_PATH));
    }

    #[test]
    fn display_zero_flags() {
        let execveat = Execveat::from_raw(
            DirFd(3),
            "prog",
            vec![c("prog")],
            vec![],
            ExecveatFlags::default(),
            0,
        );
        assert_eq!(
            execveat.to_string(),
            "execveat(3, \"prog\", [\"prog\"], [], 0) = 0"
        );
    }

    #[test]
    fn parse_success() {
        let execveat = Execveat::from_str(
            "execveat(AT_FDCWD, \"prog\", [\"prog\", \"x\"], [], AT_EMPTY_PATH) = 0",
        )
        .unwrap();
        assert_eq!(
            execveat,
            Execveat::from_raw(
                DirFd::AT_FDCWD,
                "prog",
                vec![c("prog"), c("x")],
                vec![],
                ExecveatFlags::AT_EMPTY_PATH,
                0
            )
        );
        assert_eq!(execveat.dirfd, DirFd::AT_FDCWD);
    }

    #[test]
    fn parse_failure() {
        let execveat =
            Execveat::from_str("execveat(AT_FDCWD, \"prog\", [], [], AT_EMPTY_PATH) = -1 EINVAL")
                .unwrap();
        assert_eq!(execveat.result, OkResult::Errno(Errno::EINVAL));
    }

    #[test]
    fn parse_invalid() {
        assert!(Execveat::from_str("execveat(AT_FDCWD, \"p\") = 0").is_err());
        assert!(Execveat::from_str("execveat(AT_FDCWD, \"p\", [], []) = 0").is_err());
        assert!(Execveat::from_str("execveat(AT_FDCWD, \"p\", [], [], 0) 0").is_err());
        assert!(Execveat::from_str("execveat(AT_FDCWD, \"p\", [], [], AT_BOGUS) = 0").is_err());
        assert!(Execveat::from_str("execve(\"/bin/sh\", [], []) = 0").is_err());
        assert!(Execveat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "execveat(AT_FDCWD, \"prog\", [\"prog\"], [], 0) = 0",
            "execveat(AT_FDCWD, \"./prog\", [\"prog\", \"--help\"], [\"PATH=/bin\"], AT_EMPTY_PATH) = 0",
            "execveat(3, \"a, b\", [], [], AT_SYMLINK_NOFOLLOW) = -1 ENOENT",
            "execveat(AT_FDCWD, \"prog\", [\"x \\\" y\"], [], AT_EMPTY_PATH) = -1 EACCES",
        ] {
            assert_eq!(Execveat::from_str(line).unwrap().to_string(), line);
        }
    }
}
