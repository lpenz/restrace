// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`futimesat(2)`] system call.
//!
//! [`futimesat(2)`]: https://man7.org/linux/man-pages/man2/futimesat.2.html

use super::dirfd::DirFd;
use super::parse::{self, ParseError};
use super::result::OkResult;
use super::timeval::Timevals;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `futimesat(2)` system call.
///
/// ```text
/// int futimesat(int dirfd, const char *pathname,
///               const struct timeval times[2]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Futimesat {
    /// Directory against which the pathname is resolved, or
    /// [`DirFd::AT_FDCWD`].
    pub dirfd: DirFd,
    /// Pathname of the file to update, without the terminating NUL byte.
    ///
    /// `None` when `pathname` is `NULL`, which operates on `dirfd` itself.
    pub pathname: Option<CString>,
    /// The access and modification times, or `None` to set them to the
    /// current time.
    pub times: Option<Timevals>,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Futimesat {
    /// Builds a `futimesat(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        dirfd: DirFd,
        pathname: Option<impl AsRef<[u8]>>,
        times: Option<Timevals>,
        raw_result: i64,
    ) -> Self {
        Self {
            dirfd,
            pathname: pathname
                .map(|p| CString::new(p.as_ref()).expect("pathname contains NUL byte")),
            times,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Futimesat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "futimesat({}, ", self.dirfd)?;
        match &self.pathname {
            Some(pathname) => write!(f, "{:?}", pathname.to_string_lossy())?,
            None => f.write_str("NULL")?,
        }
        f.write_str(", ")?;
        match &self.times {
            Some(times) => write!(f, "{times}")?,
            None => f.write_str("NULL")?,
        }
        write!(f, ") = {}", self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `futimesat(AT_FDCWD, "/path", NULL) = 0` or with a timeval array.
impl FromStr for Futimesat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("futimesat call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("futimesat(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 3 {
            return Err(ParseError::new("futimesat arguments", args));
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
            Some(Timevals::from_str(parts[2].trim())?)
        };
        Ok(Self {
            dirfd,
            pathname,
            times,
            result: OkResult::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syscalls::errno::Errno;

    #[test]
    fn display_nulls() {
        let fut = Futimesat::from_raw(DirFd::AT_FDCWD, Option::<&[u8]>::None, None, 0);
        assert_eq!(fut.to_string(), "futimesat(AT_FDCWD, NULL, NULL) = 0");
    }

    #[test]
    fn display_with_path_and_times() {
        let fut = Futimesat::from_raw(
            DirFd::AT_FDCWD,
            Some("/tmp/tfile"),
            Some(Timevals::from_raw(100, 200, 300, 400)),
            0,
        );
        assert_eq!(
            fut.to_string(),
            "futimesat(AT_FDCWD, \"/tmp/tfile\", [{tv_sec=100, tv_usec=200}, {tv_sec=300, tv_usec=400}]) = 0"
        );
    }

    #[test]
    fn parse_nulls() {
        let fut = Futimesat::from_str("futimesat(AT_FDCWD, NULL, NULL) = 0").unwrap();
        assert_eq!(fut.dirfd, DirFd::AT_FDCWD);
        assert_eq!(fut.pathname, None);
        assert_eq!(fut.times, None);
    }

    #[test]
    fn parse_annotated_dirfd() {
        let fut = Futimesat::from_str(
            "futimesat(AT_FDCWD</tmp/opencode/bt2>, \"/tmp/opencode/bt2/tfile\", NULL) = 0",
        )
        .unwrap();
        assert!(fut.dirfd.is_cwd());
        assert_eq!(
            fut.dirfd.path().map(|p| p.to_string_lossy().into_owned()),
            Some("/tmp/opencode/bt2".to_string())
        );
        assert_eq!(
            fut.pathname.as_ref().unwrap().to_string_lossy(),
            "/tmp/opencode/bt2/tfile"
        );
    }

    #[test]
    fn parse_with_times_and_comments() {
        let fut = Futimesat::from_str(
            "futimesat(3</tmp/tfile>, NULL, [{tv_sec=100, tv_usec=200} /* 1970-01-01T01:01:40.000200+0100 */, {tv_sec=300, tv_usec=400} /* 1970-01-01T01:05:00.000400+0100 */]) = 0",
        )
        .unwrap();
        assert_eq!(fut.times, Some(Timevals::from_raw(100, 200, 300, 400)));
    }

    #[test]
    fn parse_failure() {
        let fut =
            Futimesat::from_str("futimesat(AT_FDCWD, \"/tmp/missing\", NULL) = -1 ENOENT").unwrap();
        assert_eq!(fut.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Futimesat::from_str("futimesat(AT_FDCWD, \"/tmp/x\") = 0").is_err());
        assert!(Futimesat::from_str("futimesat(AT_FDCWD, \"/tmp/x\", NULL, 0) = 0").is_err());
        assert!(Futimesat::from_str("utimes(\"/tmp/x\", NULL) = 0").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "futimesat(AT_FDCWD, NULL, NULL) = 0",
            "futimesat(AT_FDCWD, \"/tmp/tfile\", NULL) = 0",
            "futimesat(AT_FDCWD</tmp/opencode/bt2>, \"/tmp/opencode/bt2/tfile\", NULL) = 0",
            "futimesat(3, NULL, [{tv_sec=100, tv_usec=200}, {tv_sec=300, tv_usec=400}]) = 0",
            "futimesat(AT_FDCWD, \"/tmp/tfile\", [{tv_sec=100, tv_usec=200}, {tv_sec=300, tv_usec=400}]) = 0",
            "futimesat(AT_FDCWD, NULL, NULL) = -1 EFAULT",
        ] {
            assert_eq!(Futimesat::from_str(line).unwrap().to_string(), line);
        }
    }
}
