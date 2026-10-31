// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! The environment array argument of `execve(2)` and `execveat(2)`.

use super::parse::{self, ParseError};
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// Writes a string array as strace does: `["first", "second"]`.
pub(crate) fn write_str_array(f: &mut fmt::Formatter<'_>, values: &[CString]) -> fmt::Result {
    f.write_str("[")?;
    for (i, value) in values.iter().enumerate() {
        if i != 0 {
            f.write_str(", ")?;
        }
        write!(f, "{:?}", value.to_string_lossy())?;
    }
    f.write_str("]")
}

/// Parses a string array as [`write_str_array`] writes it: a bracket-delimited,
/// comma-separated list of double-quoted strings.
pub(crate) fn parse_str_array(input: &str, part: &'static str) -> Result<Vec<CString>, ParseError> {
    let error = || ParseError::new(part, input);
    let body = input
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(error)?;
    if body.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut values = Vec::new();
    for item in parse::split_args(body) {
        let value = parse::unquote(item.trim(), part)?;
        values.push(CString::new(value).map_err(|_| error())?);
    }
    Ok(values)
}

/// The environment array of `execve(2)` and `execveat(2)`.
///
/// `strace` prints every variable when it decodes the array, and otherwise
/// abbreviates it to the address of the array and the number of variables it
/// holds, such as `0x7ffc38cbdec8 /* 63 vars */`. The abbreviated form is the
/// default when `-v` is not passed; `-e abbrev=none` disables it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvVars {
    /// The full environment, as `strace` prints it with `-v`.
    Full(Vec<CString>),
    /// The abbreviated form: the address of the array and the number of
    /// variables it holds.
    Abbreviated {
        /// The address of the `envp` array.
        address: u64,
        /// The number of environment variables in the array.
        count: usize,
    },
    /// The array is the null pointer, which the kernel treats as an empty
    /// environment.
    Null,
}

impl EnvVars {
    /// Returns the environment variables, if `strace` printed the array in
    /// full.
    ///
    /// A null array yields an empty slice, because the kernel treats it as an
    /// empty environment.
    #[must_use]
    pub fn full(&self) -> Option<&[CString]> {
        match self {
            Self::Full(values) => Some(values),
            Self::Null => Some(&[]),
            Self::Abbreviated { .. } => None,
        }
    }
}

impl From<Vec<CString>> for EnvVars {
    fn from(values: Vec<CString>) -> Self {
        Self::Full(values)
    }
}

impl fmt::Display for EnvVars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full(values) => write_str_array(f, values),
            Self::Abbreviated { address, count } => {
                write!(f, "0x{address:x} /* {count} vars */")
            }
            Self::Null => f.write_str("NULL"),
        }
    }
}

/// Parses an environment array as [`Display`](fmt::Display) writes it: the
/// decoded `["VAR=value"]` list, or the abbreviated
/// `0x<address> /* <count> vars */` form.
impl FromStr for EnvVars {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        if s == "NULL" {
            return Ok(Self::Null);
        }
        if s.starts_with('[') {
            return parse_str_array(s, "envp").map(Self::Full);
        }
        let error = || ParseError::new("envp", s);
        let (address, rest) = s.split_once(" /* ").ok_or_else(error)?;
        let address = address
            .strip_prefix("0x")
            .or_else(|| address.strip_prefix("0X"))
            .ok_or_else(error)?;
        let address = u64::from_str_radix(address, 16).map_err(|_| error())?;
        let count = rest.strip_suffix(" vars */").ok_or_else(error)?;
        let count = count.parse::<usize>().map_err(|_| error())?;
        Ok(Self::Abbreviated { address, count })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn display_full() {
        let env = EnvVars::Full(vec![c("A=1"), c("B=2")]);
        assert_eq!(env.to_string(), "[\"A=1\", \"B=2\"]");
    }

    #[test]
    fn display_abbreviated() {
        let env = EnvVars::Abbreviated {
            address: 0x7ffc38cbdec8,
            count: 63,
        };
        assert_eq!(env.to_string(), "0x7ffc38cbdec8 /* 63 vars */");
        assert_eq!(EnvVars::Null.to_string(), "NULL");
    }

    #[test]
    fn parse_full() {
        assert_eq!(
            EnvVars::from_str("[\"A=1\", \"B=2\"]").unwrap(),
            EnvVars::Full(vec![c("A=1"), c("B=2")])
        );
        assert_eq!(EnvVars::from_str("[]").unwrap(), EnvVars::Full(vec![]));
    }

    #[test]
    fn parse_abbreviated() {
        assert_eq!(
            EnvVars::from_str("0x7ffc38cbdec8 /* 63 vars */").unwrap(),
            EnvVars::Abbreviated {
                address: 0x7ffc38cbdec8,
                count: 63,
            }
        );
        assert_eq!(
            EnvVars::from_str("0x0 /* 0 vars */").unwrap(),
            EnvVars::Abbreviated {
                address: 0,
                count: 0
            }
        );
    }

    #[test]
    fn parse_null() {
        assert_eq!(EnvVars::from_str("NULL").unwrap(), EnvVars::Null);
        assert_eq!(EnvVars::Null.full(), Some(&[][..]));
    }

    #[test]
    fn parse_invalid() {
        assert!(EnvVars::from_str("0x7ffc38cbdec8").is_err());
        assert!(EnvVars::from_str("7ffc38cbdec8 /* 1 vars */").is_err());
        assert!(EnvVars::from_str("0xzz /* 1 vars */").is_err());
        assert!(EnvVars::from_str("0x1 /* one vars */").is_err());
        assert!(EnvVars::from_str("0x1 /* 1 vars").is_err());
        assert!(EnvVars::from_str("").is_err());
    }

    #[test]
    fn full_accessor() {
        let full = EnvVars::Full(vec![c("A=1")]);
        assert_eq!(full.full().unwrap(), &[c("A=1")]);
        let abbreviated = EnvVars::Abbreviated {
            address: 1,
            count: 2,
        };
        assert_eq!(abbreviated.full(), None);
    }

    #[test]
    fn round_trip() {
        for env in [
            EnvVars::Full(vec![]),
            EnvVars::Full(vec![c("A=1"), c("B=2")]),
            EnvVars::Abbreviated {
                address: 0x7ffc38cbdec8,
                count: 63,
            },
            EnvVars::Abbreviated {
                address: 0,
                count: 0,
            },
            EnvVars::Null,
        ] {
            assert_eq!(EnvVars::from_str(&env.to_string()).unwrap(), env);
        }
    }
}
