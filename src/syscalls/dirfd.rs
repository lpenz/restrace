// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The directory file descriptor argument of the `*at` system calls.

use super::parse::{self, ParseError};
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// A directory file descriptor, the `dirfd` argument of the `*at` system
/// calls such as [`openat(2)`](super::openat::Openat).
///
/// When the trace was produced with `-y`, `strace` resolves the descriptor
/// against `/proc/<pid>/fd` — or, for [`AT_FDCWD`](Self::AT_FDCWD), against
/// `/proc/<pid>/cwd` — and appends the path as associated information, which
/// is kept in [`path`](Self::path).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DirFd {
    /// The descriptor number.
    pub fd: i32,
    /// The path `strace` resolved for the descriptor with `-y`, if any.
    pub path: Option<CString>,
}

impl DirFd {
    /// Use the current working directory of the calling process.
    pub const AT_FDCWD: Self = Self {
        fd: -100,
        path: None,
    };

    /// Builds a [`DirFd`] from a descriptor number, without a resolved path.
    #[must_use]
    pub const fn new(fd: i32) -> Self {
        Self { fd, path: None }
    }

    /// Returns true if this refers to the current working directory.
    #[must_use]
    pub const fn is_cwd(&self) -> bool {
        self.fd == Self::AT_FDCWD.fd
    }

    /// Returns the path `strace` resolved for the descriptor, if any.
    #[must_use]
    pub fn path(&self) -> Option<&CString> {
        self.path.as_ref()
    }
}

impl From<i32> for DirFd {
    fn from(fd: i32) -> Self {
        Self::new(fd)
    }
}

impl fmt::Display for DirFd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_cwd() {
            f.write_str("AT_FDCWD")?;
        } else {
            write!(f, "{}", self.fd)?;
        }
        if let Some(path) = &self.path {
            write!(f, "<{}>", path.to_string_lossy())?;
        }
        Ok(())
    }
}

/// Parses a directory descriptor as [`Display`](fmt::Display) writes it:
/// `AT_FDCWD` or a decimal descriptor, optionally followed by the `<...>`
/// annotation `strace` appends with `-y`.
impl FromStr for DirFd {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let (value, annotation) = parse::split_annotation(s);
        let fd = if value == "AT_FDCWD" {
            Self::AT_FDCWD.fd
        } else {
            value
                .parse::<i32>()
                .map_err(|_| ParseError::new("dirfd", s))?
        };
        let path = annotation
            .map(|text| CString::new(text).map_err(|_| ParseError::new("dirfd", s)))
            .transpose()?;
        Ok(Self { fd, path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn annotated(fd: i32, path: &str) -> DirFd {
        DirFd {
            fd,
            path: Some(CString::new(path).unwrap()),
        }
    }

    #[test]
    fn display() {
        assert_eq!(DirFd::AT_FDCWD.to_string(), "AT_FDCWD");
        assert_eq!(DirFd::new(3).to_string(), "3");
        assert_eq!(annotated(-100, "/src").to_string(), "AT_FDCWD</src>");
        assert_eq!(annotated(3, "/src").to_string(), "3</src>");
        assert!(DirFd::AT_FDCWD.is_cwd());
        assert!(!DirFd::new(3).is_cwd());
    }

    #[test]
    fn parse() {
        assert_eq!(DirFd::from_str("AT_FDCWD"), Ok(DirFd::AT_FDCWD));
        assert_eq!(DirFd::from_str("-100"), Ok(DirFd::new(-100)));
        assert_eq!(DirFd::from_str("3"), Ok(DirFd::new(3)));
        assert_eq!(
            DirFd::from_str("AT_FDCWD</src>"),
            Ok(annotated(-100, "/src"))
        );
        assert_eq!(DirFd::from_str("3</src>"), Ok(annotated(3, "/src")));
        assert!(DirFd::from_str("AT_FDC").is_err());
        assert!(DirFd::from_str("").is_err());
    }

    #[test]
    fn path_accessor() {
        assert_eq!(DirFd::AT_FDCWD.path(), None);
        assert_eq!(
            annotated(-100, "/src").path().map(|p| p.to_str().unwrap()),
            Some("/src")
        );
    }

    #[test]
    fn round_trip() {
        for dirfd in [
            DirFd::AT_FDCWD,
            DirFd::new(0),
            DirFd::new(4096),
            annotated(-100, "/src"),
            annotated(4, "/tmp/dir"),
        ] {
            assert_eq!(DirFd::from_str(&dirfd.to_string()).unwrap(), dirfd);
        }
    }
}
