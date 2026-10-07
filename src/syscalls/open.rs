// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`open(2)`] system call.
//!
//! [`open(2)`]: https://man7.org/linux/man-pages/man2/open.2.html

use super::fd::FdResult;
use super::mode::Mode;
use super::parse::{self, ParseError};
use std::ffi::CString;
use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};
use std::str::FromStr;

/// Named flags of [`OpenFlags`], in [`Display`](fmt::Display) order.
///
/// `O_RDONLY` is left out because it is zero, and thus always "contained";
/// [`Display`](fmt::Display) adds it when no other access mode is set.
const NAMED_FLAGS: &[(OpenFlags, &str)] = &[
    (OpenFlags::O_WRONLY, "O_WRONLY"),
    (OpenFlags::O_RDWR, "O_RDWR"),
    (OpenFlags::O_CREAT, "O_CREAT"),
    (OpenFlags::O_EXCL, "O_EXCL"),
    (OpenFlags::O_NOCTTY, "O_NOCTTY"),
    (OpenFlags::O_TRUNC, "O_TRUNC"),
    (OpenFlags::O_APPEND, "O_APPEND"),
    (OpenFlags::O_NONBLOCK, "O_NONBLOCK"),
    (OpenFlags::O_DSYNC, "O_DSYNC"),
    (OpenFlags::O_ASYNC, "O_ASYNC"),
    (OpenFlags::O_DIRECT, "O_DIRECT"),
    (OpenFlags::O_LARGEFILE, "O_LARGEFILE"),
    (OpenFlags::O_DIRECTORY, "O_DIRECTORY"),
    (OpenFlags::O_NOFOLLOW, "O_NOFOLLOW"),
    (OpenFlags::O_NOATIME, "O_NOATIME"),
    (OpenFlags::O_CLOEXEC, "O_CLOEXEC"),
    (OpenFlags::O_PATH, "O_PATH"),
    (OpenFlags::O_TMPFILE, "O_TMPFILE"),
];

/// Arguments and return value of the `open(2)` system call.
///
/// ```text
/// int open(const char *pathname, int flags, mode_t mode);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    /// Pathname of the file to open, without the terminating NUL byte.
    pub pathname: CString,
    /// File status flags, the `flags` argument.
    pub flags: OpenFlags,
    /// File creation mode, the `mode` argument.
    ///
    /// Only present when the flags ask for the file to be created
    /// (`O_CREAT` or `O_TMPFILE`), which is when the kernel reads it.
    pub mode: Option<Mode>,
    /// Return value: a file descriptor, or the errno that caused the failure.
    pub result: FdResult,
}

impl Open {
    /// Builds an `open(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: a file
    /// descriptor (>= 0) on success, or `-errno` on failure.
    ///
    /// # Panics
    ///
    /// Panics if `pathname` contains an interior NUL byte, which cannot
    /// happen for pathnames read from a tracee up to their terminator.
    #[must_use]
    pub fn from_raw<P: AsRef<[u8]>>(
        pathname: P,
        flags: OpenFlags,
        mode: Option<Mode>,
        raw_result: i64,
    ) -> Self {
        Self {
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            flags,
            mode,
            result: FdResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Open {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "open({:?}, {}",
            self.pathname.to_string_lossy(),
            self.flags
        )?;
        if let Some(mode) = self.mode {
            write!(f, ", {mode}")?;
        }
        write!(f, ") = {}", self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `open("path", O_RDONLY[, mode]) = fd` or `... = -1 ENOENT`.
impl FromStr for Open {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("open call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("open(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() < 2 || parts.len() > 3 {
            return Err(ParseError::new("open arguments", args));
        }
        let pathname = parse::unquote(parts[0].trim(), "pathname")?;
        let flags = OpenFlags::from_str(parts[1].trim())?;
        let mode = parts
            .get(2)
            .map(|part| Mode::from_str(part.trim()))
            .transpose()?;
        Ok(Self {
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[0]))?,
            flags,
            mode,
            result: FdResult::from_str(result)?,
        })
    }
}

/// The `flags` argument of `open(2)`.
///
/// Values are the Linux `asm-generic` ones, which is what all architectures
/// supported by restrace use except where noted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct OpenFlags(pub u32);

impl OpenFlags {
    /// Mask for the file access mode.
    pub const O_ACCMODE: Self = Self(0o0000003);
    /// Open for reading only.
    pub const O_RDONLY: Self = Self(0o0000000);
    /// Open for writing only.
    pub const O_WRONLY: Self = Self(0o0000001);
    /// Open for reading and writing.
    pub const O_RDWR: Self = Self(0o0000002);
    /// Create the file if it does not exist.
    pub const O_CREAT: Self = Self(0o0000100);
    /// Fail if the file already exists.
    pub const O_EXCL: Self = Self(0o0000200);
    /// Open does not become the controlling terminal.
    pub const O_NOCTTY: Self = Self(0o0000400);
    /// Truncate the file to zero length.
    pub const O_TRUNC: Self = Self(0o0001000);
    /// Writes go to the end of the file.
    pub const O_APPEND: Self = Self(0o0002000);
    /// Open in nonblocking mode.
    pub const O_NONBLOCK: Self = Self(0o0004000);
    /// Synchronized I/O data integrity completion.
    pub const O_DSYNC: Self = Self(0o0010000);
    /// Send `SIGIO` for input and output events.
    pub const O_ASYNC: Self = Self(0o0020000);
    /// Bypass the buffer cache.
    pub const O_DIRECT: Self = Self(0o0040000);
    /// Only on 32-bit architectures: allow files larger than 2 GiB.
    pub const O_LARGEFILE: Self = Self(0o0100000);
    /// Open directory only; the call fails if the path is not a directory.
    pub const O_DIRECTORY: Self = Self(0o0200000);
    /// Fail if the final path component is a symbolic link.
    pub const O_NOFOLLOW: Self = Self(0o0400000);
    /// Do not update the file access time.
    pub const O_NOATIME: Self = Self(0o1000000);
    /// Close the file descriptor on `exec`.
    pub const O_CLOEXEC: Self = Self(0o2000000);
    /// Obtain a descriptor that can only be used for `*at`-family calls.
    pub const O_PATH: Self = Self(0o10000000);
    /// Create an unnamed temporary file; combines with [`OpenFlags::O_DIRECTORY`].
    pub const O_TMPFILE: Self = Self(0o20200000);

    // Union of every known flag above, for [`OpenFlags::unknown_bits`].
    const KNOWN_MASK: u32 = Self::O_ACCMODE.0
        | Self::O_CREAT.0
        | Self::O_EXCL.0
        | Self::O_NOCTTY.0
        | Self::O_TRUNC.0
        | Self::O_APPEND.0
        | Self::O_NONBLOCK.0
        | Self::O_DSYNC.0
        | Self::O_ASYNC.0
        | Self::O_DIRECT.0
        | Self::O_LARGEFILE.0
        | Self::O_DIRECTORY.0
        | Self::O_NOFOLLOW.0
        | Self::O_NOATIME.0
        | Self::O_CLOEXEC.0
        | Self::O_PATH.0
        | Self::O_TMPFILE.0;

    /// Returns true if all bits of `flag` are set in these flags.
    #[must_use]
    pub const fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) == flag.0
    }

    /// Returns the file access mode bits of these flags.
    #[must_use]
    pub const fn access_mode(self) -> Self {
        Self(self.0 & Self::O_ACCMODE.0)
    }

    /// Returns the bits of these flags that no known flag covers.
    #[must_use]
    pub const fn unknown_bits(self) -> u32 {
        self.0 & !Self::KNOWN_MASK
    }
}

impl BitOr for OpenFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for OpenFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for OpenFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for OpenFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl Not for OpenFlags {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

impl fmt::Display for OpenFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        let mut write_part = |part: &str| -> fmt::Result {
            if !first {
                f.write_str("|")?;
            }
            first = false;
            f.write_str(part)
        };
        if self.access_mode() == Self::O_RDONLY {
            write_part("O_RDONLY")?;
        }
        for (flag, name) in NAMED_FLAGS {
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
/// by `|`, plus `0x`-prefixed hexadecimal for bits that have no name.
impl FromStr for OpenFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseError::new("flags", s));
        }
        let mut flags = Self::default();
        for token in s.split('|') {
            let token = token.trim();
            let named = NAMED_FLAGS
                .iter()
                .find(|&&(_, name)| name == token)
                .map(|&(flag, _)| flag);
            let flag = match named {
                Some(flag) => flag,
                None if token == "O_RDONLY" => Self::O_RDONLY,
                None => Self(parse_number(token, "flags")?),
            };
            flags |= flag;
        }
        Ok(flags)
    }
}

/// Parses a number with an optional `0x` or `0o` prefix.
fn parse_number(token: &str, part: &'static str) -> Result<u32, ParseError> {
    let error = |_| ParseError::new(part, token);
    if let Some(hex) = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16).map_err(error)
    } else if let Some(oct) = token
        .strip_prefix("0o")
        .or_else(|| token.strip_prefix("0O"))
    {
        u32::from_str_radix(oct, 8).map_err(error)
    } else {
        token.parse::<u32>().map_err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_read_only() {
        assert_eq!(OpenFlags::O_RDONLY.to_string(), "O_RDONLY");
    }

    #[test]
    fn display_combination() {
        let flags = OpenFlags::O_WRONLY | OpenFlags::O_CREAT | OpenFlags::O_TRUNC;
        assert_eq!(flags.to_string(), "O_WRONLY|O_CREAT|O_TRUNC");
    }

    #[test]
    fn display_unknown_bits() {
        let flags = OpenFlags(0o100000000);
        assert_eq!(flags.to_string(), "O_RDONLY|0x1000000");
    }

    #[test]
    fn access_mode() {
        assert_eq!(
            (OpenFlags::O_RDWR | OpenFlags::O_CREAT).access_mode(),
            OpenFlags::O_RDWR
        );
    }

    #[test]
    fn tmpfile_contains_directory() {
        assert!(OpenFlags::O_TMPFILE.contains(OpenFlags::O_DIRECTORY));
    }

    #[test]
    fn display_success() {
        let open = Open::from_raw(
            "/tmp/file",
            OpenFlags::O_WRONLY | OpenFlags::O_CREAT | OpenFlags::O_TRUNC,
            Some(Mode(0o644)),
            3,
        );
        assert_eq!(
            open.to_string(),
            "open(\"/tmp/file\", O_WRONLY|O_CREAT|O_TRUNC, 0644) = 3"
        );
    }

    #[test]
    fn display_failure() {
        let open = Open::from_raw("/nonexistent", OpenFlags::O_RDONLY, None, -2);
        assert_eq!(
            open.to_string(),
            "open(\"/nonexistent\", O_RDONLY) = -1 ENOENT"
        );
    }

    #[test]
    fn parse_flags() {
        assert_eq!(OpenFlags::from_str("O_RDONLY"), Ok(OpenFlags::O_RDONLY));
        assert_eq!(
            OpenFlags::from_str("O_WRONLY|O_CREAT|O_TRUNC"),
            Ok(OpenFlags::O_WRONLY | OpenFlags::O_CREAT | OpenFlags::O_TRUNC)
        );
        assert_eq!(
            OpenFlags::from_str("O_RDONLY|0x1000000"),
            Ok(OpenFlags(0o100000000))
        );
        assert!(OpenFlags::from_str("O_BOGUS").is_err());
        assert!(OpenFlags::from_str("O_RDONLY|").is_err());
        assert!(OpenFlags::from_str("").is_err());
    }

    #[test]
    fn parse_success() {
        let open =
            Open::from_str("open(\"/tmp/file\", O_WRONLY|O_CREAT|O_TRUNC, 0644) = 3").unwrap();
        assert_eq!(
            open,
            Open::from_raw(
                "/tmp/file",
                OpenFlags::O_WRONLY | OpenFlags::O_CREAT | OpenFlags::O_TRUNC,
                Some(Mode(0o644)),
                3,
            )
        );
        assert_eq!(open.result.fd(), Some(3));
    }

    #[test]
    fn parse_failure() {
        let open = Open::from_str("open(\"/nonexistent\", O_RDONLY) = -1 ENOENT").unwrap();
        assert_eq!(open.mode, None);
        assert_eq!(open.result, FdResult::Errno(Errno::ENOENT));
        assert_eq!(open.pathname.to_str().unwrap(), "/nonexistent");
    }

    #[test]
    fn parse_invalid() {
        assert!(Open::from_str("open(\"/f\", O_RDONLY) 3").is_err());
        assert!(Open::from_str("open(\"/f\") = 3").is_err());
        assert!(Open::from_str("open(/f, O_RDONLY) = 3").is_err());
        assert!(Open::from_str("creat(\"/f\", 0644) = 3").is_err());
        assert!(Open::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "open(\"/tmp/file\", O_WRONLY|O_CREAT|O_TRUNC, 0644) = 3",
            "open(\"/nonexistent\", O_RDONLY) = -1 ENOENT",
            "open(\"/a, b\", O_RDONLY|O_PATH) = 0",
            "open(\"quote\\\"here\", O_RDONLY|O_NOFOLLOW) = -1 EACCES",
            "open(\"/tmp\", O_RDONLY|0x1000000) = -1 9999",
        ] {
            assert_eq!(Open::from_str(line).unwrap().to_string(), line);
        }
    }
}
