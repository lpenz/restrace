// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The [`utimes(2)`] system call.
//!
//! [`utimes(2)`]: https://man7.org/linux/man-pages/man2/utimes.2.html

use super::parse::{self, ParseError};
use super::result::OkResult;
use super::timeval::Timevals;
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Arguments and return value of the `utimes(2)` system call.
///
/// ```text
/// int utimes(const char *filename, const struct timeval times[2]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utimes {
    /// Pathname of the file to update, without the terminating NUL byte.
    ///
    /// `None` when `strace` prints `NULL`, a null pointer that the kernel
    /// rejects with `EFAULT`.
    pub pathname: Option<CString>,
    /// The access and modification times, or `None` to set them to the
    /// current time.
    pub times: Option<Timevals>,
    /// Return value: zero on success, or the errno of the failure.
    pub result: OkResult,
}

impl Utimes {
    /// Builds a `utimes(2)` record from its arguments and raw return value.
    ///
    /// `raw_result` is the value the kernel reported for the call: 0 on
    /// success, or `-errno` on failure.
    #[must_use]
    pub fn from_raw(
        pathname: Option<impl AsRef<[u8]>>,
        times: Option<Timevals>,
        raw_result: i64,
    ) -> Self {
        Self {
            pathname: pathname
                .map(|p| CString::new(p.as_ref()).expect("pathname contains NUL byte")),
            times,
            result: OkResult::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Utimes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("utimes(")?;
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
/// `utimes("/path", NULL) = 0` or with a timeval array.
impl FromStr for Utimes {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("utimes call", s);
        let (args, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let args = args.strip_prefix("utimes(").ok_or_else(syntax)?;
        let parts = parse::split_args_nested(args);
        if parts.len() != 2 {
            return Err(ParseError::new("utimes arguments", args));
        }
        let pathname = if parts[0].trim() == "NULL" {
            None
        } else {
            let p = parse::unquote(parts[0].trim(), "pathname")?;
            Some(CString::new(p).map_err(|_| ParseError::new("pathname", parts[0]))?)
        };
        let times = if parts[1].trim() == "NULL" {
            None
        } else {
            Some(Timevals::from_str(parts[1].trim())?)
        };
        Ok(Self {
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
        let ut = Utimes::from_raw(Option::<&[u8]>::None, None, 0);
        assert_eq!(ut.to_string(), "utimes(NULL, NULL) = 0");
    }

    #[test]
    fn display_with_path_and_times() {
        let ut = Utimes::from_raw(
            Some("/tmp/tfile"),
            Some(Timevals::from_raw(100, 200, 300, 400)),
            0,
        );
        assert_eq!(
            ut.to_string(),
            "utimes(\"/tmp/tfile\", [{tv_sec=100, tv_usec=200}, {tv_sec=300, tv_usec=400}]) = 0"
        );
    }

    #[test]
    fn parse_nulls() {
        let ut = Utimes::from_str("utimes(\"/tmp/tfile\", NULL) = 0").unwrap();
        assert_eq!(
            ut.pathname.as_ref().unwrap().to_string_lossy(),
            "/tmp/tfile"
        );
        assert_eq!(ut.times, None);
    }

    #[test]
    fn parse_with_times_and_comments() {
        let ut = Utimes::from_str(
            "utimes(\"/tmp/tfile\", [{tv_sec=100, tv_usec=200} /* 1970-01-01T01:01:40.000200+0100 */, {tv_sec=300, tv_usec=400} /* 1970-01-01T01:05:00.000400+0100 */]) = 0",
        )
        .unwrap();
        assert_eq!(ut.times, Some(Timevals::from_raw(100, 200, 300, 400)));
    }

    #[test]
    fn parse_failure() {
        let ut = Utimes::from_str("utimes(\"/tmp/missing\", NULL) = -1 ENOENT").unwrap();
        assert_eq!(ut.result, OkResult::Errno(Errno::ENOENT));
    }

    #[test]
    fn parse_invalid() {
        assert!(Utimes::from_str("utimes(\"/tmp/x\") = 0").is_err());
        assert!(Utimes::from_str("utimes(\"/tmp/x\", NULL, 0) = 0").is_err());
        assert!(Utimes::from_str("utime(\"/tmp/x\", NULL) = 0").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "utimes(\"/tmp/tfile\", NULL) = 0",
            "utimes(\"/tmp/tfile\", [{tv_sec=100, tv_usec=200}, {tv_sec=300, tv_usec=400}]) = 0",
            "utimes(NULL, NULL) = -1 EFAULT",
        ] {
            assert_eq!(Utimes::from_str(line).unwrap().to_string(), line);
        }
    }
}
