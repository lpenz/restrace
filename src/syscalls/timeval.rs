// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! A `struct timeval` array, used by `utimes(2)` and `futimesat(2)`.

use super::parse::{self, ParseError};
use std::fmt;
use std::str::FromStr;

/// A `struct timeval`, as `strace` prints it:
/// `{tv_sec=123, tv_usec=456789}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeval {
    /// Seconds.
    pub tv_sec: i64,
    /// Microseconds.
    pub tv_usec: i64,
}

impl fmt::Display for Timeval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{tv_sec={}, tv_usec={}}}", self.tv_sec, self.tv_usec)
    }
}

impl FromStr for Timeval {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = parse::strip_comment(s.trim()).trim();
        let syntax = || ParseError::new("timeval", s);
        if !s.starts_with('{') || !s.ends_with('}') {
            return Err(syntax());
        }
        let inner = &s[1..s.len() - 1];
        let mut tv_sec = None;
        let mut tv_usec = None;
        for part in parse::split_args(inner) {
            let part = part.trim();
            let (key, value) = part.split_once('=').ok_or_else(syntax)?;
            let value = parse::strip_comment(value.trim()).trim();
            match key.trim() {
                "tv_sec" => tv_sec = Some(value.parse::<i64>().map_err(|_| syntax())?),
                "tv_usec" => tv_usec = Some(value.parse::<i64>().map_err(|_| syntax())?),
                _ => return Err(syntax()),
            }
        }
        Ok(Self {
            tv_sec: tv_sec.ok_or_else(syntax)?,
            tv_usec: tv_usec.ok_or_else(syntax)?,
        })
    }
}

/// The two-element `struct timeval` array passed to `utimes(2)` and
/// `futimesat(2)`: the access and modification times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timevals([Timeval; 2]);

impl Timevals {
    /// Builds the array from raw values.
    #[must_use]
    pub fn from_raw(a_sec: i64, a_usec: i64, m_sec: i64, m_usec: i64) -> Self {
        Self([
            Timeval {
                tv_sec: a_sec,
                tv_usec: a_usec,
            },
            Timeval {
                tv_sec: m_sec,
                tv_usec: m_usec,
            },
        ])
    }
}

impl fmt::Display for Timevals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.0[0], self.0[1])
    }
}

impl FromStr for Timevals {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("timevals", s);
        if !s.starts_with('[') || !s.ends_with(']') {
            return Err(syntax());
        }
        let inner = &s[1..s.len() - 1];
        let parts = parse::split_args_nested(inner);
        if parts.len() != 2 {
            return Err(syntax());
        }
        Ok(Self([
            Timeval::from_str(parts[0].trim())?,
            Timeval::from_str(parts[1].trim())?,
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_timeval() {
        let tv = Timeval {
            tv_sec: 123,
            tv_usec: 456789,
        };
        assert_eq!(tv.to_string(), "{tv_sec=123, tv_usec=456789}");
    }

    #[test]
    fn display_timevals() {
        let tvs = Timevals::from_raw(100, 200, 300, 400);
        assert_eq!(
            tvs.to_string(),
            "[{tv_sec=100, tv_usec=200}, {tv_sec=300, tv_usec=400}]"
        );
    }

    #[test]
    fn parse_timeval() {
        assert_eq!(
            Timeval::from_str("{tv_sec=123, tv_usec=456789}").unwrap(),
            Timeval {
                tv_sec: 123,
                tv_usec: 456789,
            }
        );
    }

    #[test]
    fn parse_timevals_with_comments() {
        let tvs = Timevals::from_str(
            "[{tv_sec=100, tv_usec=200} /* 1970-01-01T01:01:40.000200+0100 */, {tv_sec=300, tv_usec=400} /* 1970-01-01T01:05:00.000400+0100 */]",
        )
        .unwrap();
        assert_eq!(tvs, Timevals::from_raw(100, 200, 300, 400));
    }

    #[test]
    fn parse_invalid() {
        assert!(Timeval::from_str("{tv_sec=1}").is_err());
        assert!(Timeval::from_str("{tv_sec=1, tv_nsec=2}").is_err());
        assert!(Timeval::from_str("{tv_sec=x, tv_usec=2}").is_err());
        assert!(Timeval::from_str("1, 2").is_err());
        assert!(Timevals::from_str("[{tv_sec=1, tv_usec=2}]").is_err());
        assert!(Timevals::from_str("{tv_sec=1, tv_usec=2}").is_err());
    }

    #[test]
    fn round_trip() {
        for tvs in [
            Timevals::from_raw(0, 0, 0, 0),
            Timevals::from_raw(100, 200, 300, 400),
            Timevals::from_raw(-1, 999999, 1_700_000_000, 123456),
        ] {
            assert_eq!(Timevals::from_str(&tvs.to_string()).unwrap(), tvs);
        }
    }
}
