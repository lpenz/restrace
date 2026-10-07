// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`openat(2)`] system call.
//!
//! [`openat(2)`]: https://man7.org/linux/man-pages/man2/openat.2.html

use super::dirfd::DirFd;
use super::open::{Mode, OpenFlags, OpenResult};
use super::parse::{self, ParseError};
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `openat(2)` system call.
///
/// ```text
/// int openat(int dirfd, const char *pathname, int flags, mode_t mode);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Openat {
    /// Directory against which the pathname is resolved, or [`DirFd::AT_FDCWD`].
    pub dirfd: DirFd,
    /// Pathname of the file to open, without the terminating NUL byte.
    ///
    /// Interpreted relative to `dirfd` unless it is absolute.
    pub pathname: CString,
    /// File status flags, the `flags` argument.
    pub flags: OpenFlags,
    /// File creation mode, the `mode` argument.
    ///
    /// Only present when the flags ask for the file to be created
    /// (`O_CREAT` or `O_TMPFILE`), which is when the kernel reads it.
    pub mode: Option<Mode>,
    /// Return value: a file descriptor, or the errno that caused the failure.
    pub result: OpenResult,
}

impl Openat {
    /// Builds an `openat(2)` record from its arguments and raw return value.
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
        dirfd: DirFd,
        pathname: P,
        flags: OpenFlags,
        mode: Option<Mode>,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: CString::new(pathname.as_ref()).expect("pathname contains NUL byte"),
            flags,
            mode,
            result: OpenResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Openat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "openat({}, {:?}, {}",
            self.dirfd,
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
/// `openat(dirfd, "path", O_RDONLY[, mode]) = fd` or `... = -1 ENOENT`.
impl FromStr for Openat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("openat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("openat(").ok_or_else(syntax)?;
        let parts = parse::split_args(args);
        if parts.len() < 3 || parts.len() > 4 {
            return Err(ParseError::new("openat arguments", args));
        }
        let dirfd = DirFd::from_str(parts[0].trim())?;
        let pathname = parse::unquote(parts[1].trim(), "pathname")?;
        let flags = OpenFlags::from_str(parts[2].trim())?;
        let mode = parts
            .get(3)
            .map(|part| Mode::from_str(part.trim()))
            .transpose()?;
        Ok(Self {
            dirfd,
            pathname: CString::new(pathname).map_err(|_| ParseError::new("pathname", parts[1]))?,
            flags,
            mode,
            result: OpenResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_success() {
        let openat = Openat::from_raw(DirFd::AT_FDCWD, "/tmp/file", OpenFlags::O_RDONLY, None, 3);
        assert_eq!(
            openat.to_string(),
            "openat(AT_FDCWD, \"/tmp/file\", O_RDONLY) = 3"
        );
        assert_eq!(openat.result.fd(), Some(3));
    }

    #[test]
    fn display_dirfd_and_mode() {
        let openat = Openat::from_raw(
            DirFd(4),
            "file",
            OpenFlags::O_WRONLY | OpenFlags::O_CREAT | OpenFlags::O_TRUNC,
            Some(Mode(0o600)),
            -13,
        );
        assert_eq!(
            openat.to_string(),
            "openat(4, \"file\", O_WRONLY|O_CREAT|O_TRUNC, 0600) = -1 EACCES"
        );
    }

    #[test]
    fn parse_success() {
        let openat = Openat::from_str("openat(AT_FDCWD, \"/f\", O_RDONLY) = 3").unwrap();
        assert_eq!(
            openat,
            Openat::from_raw(DirFd::AT_FDCWD, "/f", OpenFlags::O_RDONLY, None, 3)
        );
        assert!(openat.dirfd.is_cwd());
        assert_eq!(openat.mode, None);
    }

    #[test]
    fn parse_failure() {
        let openat =
            Openat::from_str("openat(4, \"f\", O_WRONLY|O_CREAT, 0600) = -1 EACCES").unwrap();
        assert_eq!(openat.dirfd, DirFd(4));
        assert_eq!(openat.mode, Some(Mode(0o600)));
        assert_eq!(openat.result, OpenResult::Errno(Errno::EACCES));
        assert_eq!(openat.pathname.to_str().unwrap(), "f");
    }

    #[test]
    fn parse_invalid() {
        assert!(Openat::from_str("openat(AT_FDCWD, \"/f\", O_RDONLY) 3").is_err());
        assert!(Openat::from_str("openat(\"/f\", O_RDONLY) = 3").is_err());
        assert!(Openat::from_str("openat(AT_FDCWD, \"/f\") = 3").is_err());
        assert!(Openat::from_str("openat(/f, \"/f\", O_RDONLY) = 3").is_err());
        assert!(Openat::from_str("open(\"/f\", O_RDONLY) = 3").is_err());
        assert!(Openat::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "openat(AT_FDCWD, \"/tmp/file\", O_RDONLY) = 3",
            "openat(4, \"file\", O_WRONLY|O_CREAT|O_TRUNC, 0600) = -1 EACCES",
            "openat(AT_FDCWD, \"relative/x\", O_RDONLY|O_DIRECTORY) = -1 ENOTDIR",
            "openat(0, \"a, b\", O_RDONLY|O_PATH) = -1 EBADF",
            "openat(AT_FDCWD, \"/tmp\", O_RDONLY|0x1000000) = -1 9999",
        ] {
            assert_eq!(Openat::from_str(line).unwrap().to_string(), line);
        }
    }
}
