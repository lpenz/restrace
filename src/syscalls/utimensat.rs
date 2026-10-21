// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`utimensat(2)`] system call.
//!
//! [`utimensat(2)`]: https://man7.org/linux/man-pages/man2/utimensat.2.html

use super::dirfd::DirFd;
use super::flags;
use super::parse::{self, ParseError};
use super::result::OkResult;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Named bits of [`UtimensatFlags`], in [`Display`](fmt::Display) order.
const NAMED_FLAGS: &[(u32, &str)] = &[
    (UtimensatFlags::AT_SYMLINK_NOFOLLOW.0, "AT_SYMLINK_NOFOLLOW"),
    (UtimensatFlags::AT_EMPTY_PATH.0, "AT_EMPTY_PATH"),
];

/// Arguments and return value of the `utimensat(2)` system call.
///
/// ```text
/// int utimensat(int dirfd, const char *pathname,
///               const struct timespec times[2], int flags);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utimensat {
    /// Directory against which the pathname is resolved, or
    /// [`DirFd::AT_FDCWD`].
    pub dirfd: DirFd,
    /// Pathname of the file to update, without the terminating NUL byte.
    ///
    /// May be `None` when operating on the file referred to by `dirfd` with
    /// appropriate flags.
    pub pathname: Option<CString>,
    /// The times to set, or `None` to set to current time.
    pub times: Option<UtimensatTimes>,
    /// Flags, the `flags` argument; see [`UtimensatFlags`].
    pub flags: UtimensatFlags,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Utimensat {
    /// Builds a `utimensat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        dirfd: DirFd,
        pathname: Option<impl AsRef<[u8]>>,
        times: Option<UtimensatTimes>,
        flags: UtimensatFlags,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: pathname
                .map(|p| CString::new(p.as_ref()).expect("pathname contains NUL byte")),
            times,
            flags,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Utimensat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "utimensat({}, ", self.dirfd)?;
        match &self.pathname {
            Some(pathname) => write!(f, "{:?}", pathname.to_string_lossy())?,
            None => write!(f, "NULL")?,
        }
        write!(f, ", ")?;
        match &self.times {
            Some(times) => write!(f, "{times}")?,
            None => write!(f, "NULL")?,
        }
        write!(f, ", {}) = {}", self.flags, self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `utimensat(AT_FDCWD, "/path", NULL, 0) = 0` or with times and flags.
impl FromStr for Utimensat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("utimensat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("utimensat(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 4 {
            return Err(ParseError::new("utimensat arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = if parts[1].trim() == "NULL" {
            None
        } else {
            let p = parse::unquote(parts[1].trim(), "pathname")?;
            Some(CString::new(p).map_err(|_| ParseError::new("pathname", parts[1]))?)
        };
        let times = if parts[2].trim() == "NULL" {
            None
        } else {
            Some(UtimensatTimes::from_str(parts[2].trim())?)
        };
        let flags = UtimensatFlags::from_str(parts[3].trim())?;
        Ok(Self {
            dirfd,
            pathname,
            times,
            flags,
            result: OkResult::from_str(result)?,
        })
    }
}

/// The `flags` argument of `utimensat(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct UtimensatFlags(pub u32);

impl UtimensatFlags {
    /// Don't follow symbolic links.
    pub const AT_SYMLINK_NOFOLLOW: Self = Self(0x100);
    /// Use pathname relative to dirfd even if pathname is empty.
    pub const AT_EMPTY_PATH: Self = Self(0x1000);
}

impl fmt::Display for UtimensatFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        flags::display(f, self.0, NAMED_FLAGS, "AT_???")
    }
}

/// Parses flags as [`Display`](fmt::Display) writes them.
impl FromStr for UtimensatFlags {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        flags::parse(s, NAMED_FLAGS, "flags").map(Self)
    }
}

/// The times array passed to `utimensat(2)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UtimensatTimes([Timespec; 2]);

impl UtimensatTimes {
    /// Builds a times array from raw values.
    #[must_use]
    pub fn from_raw(a_sec: i64, a_nsec: i64, m_sec: i64, m_nsec: i64) -> Self {
        Self([
            Timespec {
                tv_sec: a_sec,
                tv_nsec: a_nsec,
            },
            Timespec {
                tv_sec: m_sec,
                tv_nsec: m_nsec,
            },
        ])
    }
}

impl fmt::Display for UtimensatTimes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.0[0], self.0[1])
    }
}

impl FromStr for UtimensatTimes {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("utimensat times", s);
        if !s.starts_with('[') || !s.ends_with(']') {
            return Err(syntax());
        }
        let inner = &s[1..s.len() - 1];
        let parts = parse::split_args_nested(inner);
        if parts.len() != 2 {
            return Err(syntax());
        }
        let t0 = Timespec::from_str(parts[0].trim())?;
        let t1 = Timespec::from_str(parts[1].trim())?;
        Ok(Self([t0, t1]))
    }
}

/// A `struct timespec` used by `utimensat(2)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timespec {
    /// Seconds.
    pub tv_sec: i64,
    /// Nanoseconds.
    pub tv_nsec: i64,
}

impl fmt::Display for Timespec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.tv_nsec == UTIME_NOW {
            write!(f, "UTIME_NOW")
        } else if self.tv_nsec == UTIME_OMIT {
            write!(f, "UTIME_OMIT")
        } else {
            write!(f, "{{tv_sec={}, tv_nsec={}}}", self.tv_sec, self.tv_nsec)
        }
    }
}

const UTIME_NOW: i64 = (1 << 30) - 1;
const UTIME_OMIT: i64 = (1 << 30) - 2;

impl FromStr for Timespec {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        // Strip comments if present (e.g. " ... /* ... */")
        let s = parse::strip_comment(s).trim();
        if s == "UTIME_NOW" {
            return Ok(Self {
                tv_sec: 0,
                tv_nsec: UTIME_NOW,
            });
        }
        if s == "UTIME_OMIT" {
            return Ok(Self {
                tv_sec: 0,
                tv_nsec: UTIME_OMIT,
            });
        }
        let syntax = || ParseError::new("timespec", s);
        if !s.starts_with('{') || !s.ends_with('}') {
            return Err(syntax());
        }
        let inner = &s[1..s.len() - 1];
        let parts = parse::split_args(inner);
        let mut tv_sec = None;
        let mut tv_nsec = None;
        for part in parts {
            let part = part.trim();
            if let Some((k, v)) = part.split_once('=') {
                let v = v.trim();
                // Remove comments from value if attached
                let v = parse::strip_comment(v).trim();
                match k.trim() {
                    "tv_sec" => {
                        tv_sec = Some(v.parse::<i64>().map_err(|_| syntax())?);
                    }
                    "tv_nsec" => {
                        tv_nsec = Some(v.parse::<i64>().map_err(|_| syntax())?);
                    }
                    _ => return Err(syntax()),
                }
            } else {
                return Err(syntax());
            }
        }
        Ok(Self {
            tv_sec: tv_sec.ok_or_else(syntax)?,
            tv_nsec: tv_nsec.ok_or_else(syntax)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_nulls() {
        let ut = Utimensat::from_raw(
            DirFd::AT_FDCWD,
            Option::<&[u8]>::None,
            None,
            UtimensatFlags(0),
            0,
        );
        assert_eq!(ut.to_string(), "utimensat(AT_FDCWD, NULL, NULL, 0) = 0");
    }

    #[test]
    fn display_with_path_and_times() {
        let times = UtimensatTimes::from_raw(123456789, 456789, 654321, 321000);
        let ut = Utimensat::from_raw(
            DirFd::AT_FDCWD,
            Some("/tmp/ut13"),
            Some(times),
            UtimensatFlags(0),
            0,
        );
        assert_eq!(
            ut.to_string(),
            "utimensat(AT_FDCWD, \"/tmp/ut13\", [{tv_sec=123456789, tv_nsec=456789}, {tv_sec=654321, tv_nsec=321000}], 0) = 0"
        );
    }

    #[test]
    fn display_utime_now_omit() {
        let times = UtimensatTimes([
            Timespec {
                tv_sec: 0,
                tv_nsec: UTIME_NOW,
            },
            Timespec {
                tv_sec: 0,
                tv_nsec: UTIME_OMIT,
            },
        ]);
        let ut = Utimensat::from_raw(
            DirFd::AT_FDCWD,
            Some("/tmp/ut12"),
            Some(times),
            UtimensatFlags(0),
            0,
        );
        assert_eq!(
            ut.to_string(),
            "utimensat(AT_FDCWD, \"/tmp/ut12\", [UTIME_NOW, UTIME_OMIT], 0) = 0"
        );
    }

    #[test]
    fn display_with_flags() {
        let ut = Utimensat::from_raw(
            DirFd::AT_FDCWD,
            Some("/tmp/ut14"),
            None,
            UtimensatFlags::AT_SYMLINK_NOFOLLOW,
            0,
        );
        assert_eq!(
            ut.to_string(),
            "utimensat(AT_FDCWD, \"/tmp/ut14\", NULL, AT_SYMLINK_NOFOLLOW) = 0"
        );
    }

    #[test]
    fn parse_nulls() {
        let ut = Utimensat::from_str("utimensat(AT_FDCWD, NULL, NULL, 0) = 0").unwrap();
        assert_eq!(ut.pathname, None);
        assert_eq!(ut.times, None);
        assert_eq!(ut.flags, UtimensatFlags(0));
    }

    #[test]
    fn parse_touch_style() {
        let ut = Utimensat::from_str("utimensat(0, NULL, NULL, 0) = 0").unwrap();
        assert_eq!(ut.dirfd, DirFd::new(0));
        assert_eq!(ut.pathname, None);
        assert_eq!(ut.times, None);
    }

    #[test]
    fn parse_with_times() {
        let ut = Utimensat::from_str(
            "utimensat(AT_FDCWD, \"/tmp/ut13\", [{tv_sec=123, tv_nsec=456789} /* 1970-01-01T01:02:03.000456789+0100 */, {tv_sec=654, tv_nsec=321000} /* 1970-01-01T01:10:54.000321000+0100 */], 0) = 0",
        )
        .unwrap();
        assert_eq!(ut.pathname.as_ref().unwrap().to_string_lossy(), "/tmp/ut13");
    }

    #[test]
    fn parse_utime_now_omit() {
        let ut = Utimensat::from_str(
            "utimensat(AT_FDCWD, \"/tmp/ut12\", [UTIME_OMIT, UTIME_NOW], 0) = 0",
        )
        .unwrap();
        assert_eq!(ut.times.as_ref().unwrap().0[0].tv_nsec, UTIME_OMIT);
        assert_eq!(ut.times.as_ref().unwrap().0[1].tv_nsec, UTIME_NOW);
    }

    #[test]
    fn parse_failure() {
        let ut =
            Utimensat::from_str("utimensat(AT_FDCWD, \"/tmp/x\", NULL, 0) = -1 ENOENT").unwrap();
        assert_eq!(ut.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn round_trip() {
        for line in [
            "utimensat(AT_FDCWD, NULL, NULL, 0) = 0",
            "utimensat(0, NULL, NULL, 0) = 0",
            "utimensat(AT_FDCWD, \"/tmp/ut14\", NULL, AT_SYMLINK_NOFOLLOW) = 0",
            "utimensat(AT_FDCWD, \"/tmp/ut12\", [UTIME_NOW, UTIME_NOW], 0) = 0",
            "utimensat(AT_FDCWD, \"/tmp/ut12\", [UTIME_OMIT, UTIME_NOW], 0) = 0",
            "utimensat(AT_FDCWD, \"/tmp/ut13\", [{tv_sec=123, tv_nsec=456789}, {tv_sec=654, tv_nsec=321000}], 0) = 0",
            "utimensat(123, \"/tmp/ut16\", NULL, 0) = 0",
        ] {
            assert_eq!(Utimensat::from_str(line).unwrap().to_string(), line);
        }
    }
}
