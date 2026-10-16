// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`openat2(2)`] system call.
//!
//! [`openat2(2)`]: https://man7.org/linux/man-pages/man2/openat2.2.html

use super::dirfd::DirFd;
use super::fd::FdResult;
use super::flags;
use super::mode::Mode;
use super::parse::{self, ParseError};
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// The `resolve` flags of `openat2(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct ResolveFlags(pub u32);

/// Named bits of [`ResolveFlags`], in [`Display`](fmt::Display) order.
const RESOLVE_NAMES: &[(u32, &str)] = &[
    (0x0001, "RESOLVE_NO_XDEV"),
    (0x0002, "RESOLVE_NO_MAGICLINKS"),
    (0x0004, "RESOLVE_NO_SYMLINKS"),
    (0x0008, "RESOLVE_BENEATH"),
    (0x0010, "RESOLVE_IN_ROOT"),
    (0x0020, "RESOLVE_CACHED"),
];

impl ResolveFlags {
    /// No flag set.
    pub const NONE: Self = Self(0);
}

impl fmt::Display for ResolveFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, RESOLVE_NAMES)
    }
}

impl FromStr for ResolveFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let value = flags::parse(s, RESOLVE_NAMES, "resolve flags")?;
        Ok(Self(value))
    }
}

/// The `open_how` argument of `openat2(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OpenHow {
    /// File creation flags.
    pub flags: u32,
    /// Mode to create the file with.
    pub mode: Option<Mode>,
    /// Path resolution flags.
    pub resolve: ResolveFlags,
}

impl fmt::Display for OpenHow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{flags=")?;
        write!(f, "{}", super::open::OpenFlags(self.flags))?;
        if let Some(mode) = self.mode {
            write!(f, ", mode={mode}")?;
        }
        write!(f, ", resolve={}}}", self.resolve)
    }
}

impl FromStr for OpenHow {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("open_how", s);
        let body = s
            .strip_prefix('{')
            .and_then(|x| x.strip_suffix('}'))
            .ok_or_else(error)?;
        let (mut flags, mut mode, mut resolve) = (None, None, ResolveFlags::NONE);
        for field in parse::split_args_nested(body) {
            let (key, value) = field.trim().split_once('=').ok_or_else(error)?;
            let value = value.trim();
            match key {
                "flags" => flags = Some(super::open::OpenFlags::from_str(value)?.0),
                "mode" => mode = Some(Mode::from_str(value)?),
                "resolve" => resolve = ResolveFlags::from_str(value)?,
                _ => return Err(error()),
            }
        }
        let flags = flags.ok_or_else(error)?;
        Ok(Self {
            flags,
            mode,
            resolve,
        })
    }
}

impl super::buffer::BufferValue for OpenHow {
    fn decode_buffer(input: &str) -> Result<Self, super::parse::ParseError> {
        Self::from_str(input)
    }
}

/// Arguments and return value of the `openat2(2)` system call.
///
/// ```text
/// long openat2(int dirfd, const char *pathname, struct open_how *how, size_t size);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Openat2 {
    /// Directory file descriptor.
    pub dirfd: DirFd,
    /// Pathname of the file to open.
    pub pathname: CString,
    /// Open how specification.
    pub how: super::buffer::Buffer<OpenHow>,
    /// Size of the open_how structure.
    pub size: u64,
    /// Return value: the new file descriptor, or the errno of the failure.
    pub result: FdResult,
}

impl Openat2 {
    /// Builds an `openat2(2)` record from its arguments and raw return value.
    #[must_use]
    pub fn from_raw(
        dirfd: DirFd,
        pathname: impl Into<Vec<u8>>,
        how: super::buffer::Buffer<OpenHow>,
        size: u64,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname).expect("pathname contains NUL byte"),
            how,
            size,
            result: FdResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Openat2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "openat2({}, {:?}, {}, {}) = {}",
            self.dirfd,
            self.pathname.to_string_lossy(),
            self.how,
            self.size,
            self.result
        )
    }
}

impl FromStr for Openat2 {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("openat2 call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("openat2(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("openat2 arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
            how: super::buffer::parse(parts[2].trim())?,
            size: parts[3]
                .trim()
                .parse::<u64>()
                .map_err(|_| ParseError::new("size", parts[3]))?,
            result: FdResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::open::OpenFlags;

    fn how(flags: u32, resolve: ResolveFlags) -> OpenHow {
        OpenHow {
            flags,
            mode: None,
            resolve,
        }
    }

    #[test]
    fn display_resolve() {
        assert_eq!(
            how(0, ResolveFlags::NONE).to_string(),
            "{flags=O_RDONLY, resolve=0}"
        );
        assert_eq!(
            how(OpenFlags::O_CLOEXEC.0, ResolveFlags(0x0008)).to_string(),
            "{flags=O_RDONLY|O_CLOEXEC, resolve=RESOLVE_BENEATH}"
        );
    }

    #[test]
    fn parse_resolve() {
        assert_eq!(
            OpenHow::from_str("{flags=O_RDONLY, resolve=0}").unwrap(),
            how(0, ResolveFlags::NONE)
        );
        assert_eq!(
            OpenHow::from_str("{flags=O_RDONLY, resolve=RESOLVE_BENEATH}").unwrap(),
            how(0, ResolveFlags(0x0008))
        );
    }

    #[test]
    fn round_trip_how() {
        for line in [
            "{flags=O_RDONLY, resolve=0}",
            "{flags=O_WRONLY|O_CREAT, mode=0644, resolve=0}",
            "{flags=O_RDONLY, resolve=RESOLVE_BENEATH}",
        ] {
            assert_eq!(OpenHow::from_str(line).unwrap().to_string(), line);
        }
    }

    #[test]
    fn display_success() {
        let call = Openat2::from_raw(
            DirFd::AT_FDCWD,
            "/etc/hostname",
            super::super::buffer::Buffer::Decoded(how(0, ResolveFlags::NONE)),
            24,
            3,
        );
        assert_eq!(
            call.to_string(),
            "openat2(AT_FDCWD, \"/etc/hostname\", {flags=O_RDONLY, resolve=0}, 24) = 3"
        );
    }

    #[test]
    fn display_failure() {
        let call = Openat2::from_raw(
            DirFd::AT_FDCWD,
            "/etc/hostname",
            super::super::buffer::Buffer::Address(0x7ffc1234),
            24,
            -17,
        );
        assert_eq!(
            call.to_string(),
            "openat2(AT_FDCWD, \"/etc/hostname\", 0x7ffc1234, 24) = -1 EEXIST"
        );
    }

    #[test]
    fn parse_success() {
        let line = "openat2(AT_FDCWD, \"/etc/hostname\", {flags=O_RDONLY, resolve=0}, 24) = 3";
        let call = Openat2::from_str(line).unwrap();
        assert_eq!(call.dirfd, DirFd::AT_FDCWD);
        assert_eq!(
            call.how,
            super::super::buffer::Buffer::Decoded(how(0, ResolveFlags::NONE))
        );
        assert_eq!(call.size, 24);
        assert_eq!(call.result, FdResult::Fd(3));
        assert_eq!(call.to_string(), line);
    }

    #[test]
    fn parse_invalid() {
        assert!(Openat2::from_str("openat2(AT_FDCWD, \"/f\") = 0").is_err());
        assert!(
            Openat2::from_str("openat2(AT_FDCWD, \"/f\", {flags=O_RDONLY, resolve=0}) = 0")
                .is_err()
        );
        assert!(Openat2::from_str("openat(AT_FDCWD, \"/f\", O_RDONLY) = 0").is_err());
    }
}
