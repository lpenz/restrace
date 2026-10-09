// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! A generic system call, for calls without a dedicated decoder.
//!
//! [`Syscall`] holds a vector of [`Arg`] values and a [`Ret`] result, so that
//! any system call representation can be read and written even when no
//! dedicated type parses it.

use super::errno::Errno;
use super::parse::{self, ParseError};
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// A system call argument value whose meaning is unknown to the decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// An unsigned value, printed in decimal.
    Unsigned(u64),
    /// A signed value, printed in decimal.
    ///
    /// Non-negative values print without a sign, so they parse back as
    /// [`Arg::Unsigned`] and not as this variant.
    Signed(i64),
    /// A value printed in hexadecimal with a `0x` prefix.
    Hex(u64),
    /// A string, printed double-quoted.
    Str(CString),
    /// An array of arguments, printed bracket-delimited and comma-separated.
    Array(Vec<Arg>),
}

impl fmt::Display for Arg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsigned(value) => write!(f, "{value}"),
            Self::Signed(value) => write!(f, "{value}"),
            Self::Hex(value) => write!(f, "0x{value:x}"),
            Self::Str(value) => write!(f, "{:?}", value.to_string_lossy()),
            Self::Array(values) => {
                f.write_str("[")?;
                for (i, value) in values.iter().enumerate() {
                    if i != 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{value}")?;
                }
                f.write_str("]")
            }
        }
    }
}

/// Parses an argument as [`Display`](fmt::Display) writes it: a double-quoted
/// string, a `0x`-prefixed hexadecimal number, a negative decimal, an
/// unsigned decimal, or a bracket-delimited array of arguments.
impl FromStr for Arg {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("argument", s);
        if s.starts_with('"') {
            let value = parse::unquote(s, "argument")?;
            return CString::new(value).map(Self::Str).map_err(|_| error());
        }
        if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            return u64::from_str_radix(hex, 16)
                .map(Self::Hex)
                .map_err(|_| error());
        }
        if s.starts_with('-') {
            return s.parse::<i64>().map(Self::Signed).map_err(|_| error());
        }
        if let Some(body) = s.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            if body.trim().is_empty() {
                return Ok(Self::Array(Vec::new()));
            }
            let values = parse::split_args_nested(body)
                .into_iter()
                .map(|part| Self::from_str(part.trim()))
                .collect::<Result<_, _>>()?;
            return Ok(Self::Array(values));
        }
        s.parse::<u64>().map(Self::Unsigned).map_err(|_| error())
    }
}

/// The return value of a generic system call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ret {
    /// The value the call returned on success.
    Value(i64),
    /// The errno that caused the failure.
    Errno(Errno),
}

impl Ret {
    /// Converts a raw return value into a [`Ret`].
    ///
    /// The kernel reports errors as `-errno` in the range -1..=-4095;
    /// anything else counts as a successful value.
    #[must_use]
    pub fn from_raw(raw: i64) -> Self {
        if (-4095..=-1).contains(&raw) {
            Self::Errno(Errno(-raw as i32))
        } else {
            Self::Value(raw)
        }
    }
}

impl fmt::Display for Ret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(value) => write!(f, "{value}"),
            Self::Errno(errno) => write!(f, "-1 {errno}"),
        }
    }
}

/// Parses a return value as [`Display`](fmt::Display) writes it: a decimal
/// value, or `-1` followed by the errno.
impl FromStr for Ret {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let error = || ParseError::new("return value", s);
        if let Some(errno) = s.strip_prefix("-1 ") {
            return Errno::from_str(errno.trim())
                .map(Self::Errno)
                .map_err(|_| error());
        }
        s.parse::<i64>().map(Self::Value).map_err(|_| error())
    }
}

/// A system call with a vector of generic arguments and a generic return
/// value, used for calls that no dedicated type parses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syscall {
    /// Name of the system call.
    pub name: String,
    /// Argument values, in call order.
    pub args: Vec<Arg>,
    /// Return value: the value on success, or the errno on failure.
    pub result: Ret,
}

impl Syscall {
    /// Builds a generic system call record from its name, its raw argument
    /// words and its raw return value.
    ///
    /// The argument words are stored as [`Arg::Hex`], the form the kernel
    /// registers provide, and `raw_result` is converted with
    /// [`Ret::from_raw`].
    #[must_use]
    pub fn from_raw(name: impl Into<String>, args: Vec<u64>, raw_result: i64) -> Self {
        Self {
            name: name.into(),
            args: args.into_iter().map(Arg::Hex).collect(),
            result: Ret::from_raw(raw_result),
        }
    }
}

impl fmt::Display for Syscall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.name)?;
        for (i, arg) in self.args.iter().enumerate() {
            if i != 0 {
                f.write_str(", ")?;
            }
            write!(f, "{arg}")?;
        }
        write!(f, ") = {}", self.result)
    }
}

/// Parses a line as [`Display`](fmt::Display) writes it:
/// `name(arg, ...) = value` or `... = -1 ENOENT`.
impl FromStr for Syscall {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        let s = s.trim();
        let syntax = || ParseError::new("syscall call", s);
        let (head, result) = s.rsplit_once(") = ").ok_or_else(syntax)?;
        let open = head.find('(').ok_or_else(syntax)?;
        let name = &head[..open];
        if name.is_empty() || name.contains(char::is_whitespace) {
            return Err(ParseError::new("syscall name", name));
        }
        let body = &head[open + 1..];
        let args = if body.trim().is_empty() {
            Vec::new()
        } else {
            parse::split_args_nested(body)
                .into_iter()
                .map(|part| Arg::from_str(part.trim()))
                .collect::<Result<_, _>>()?
        };
        Ok(Self {
            name: name.to_string(),
            args,
            result: Ret::from_str(result)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_success() {
        let syscall = Syscall::from_raw("cacheflush", vec![0x1000, 0x20, 3], 0);
        assert_eq!(syscall.to_string(), "cacheflush(0x1000, 0x20, 0x3) = 0");
        assert_eq!(syscall.result, Ret::Value(0));
    }

    #[test]
    fn display_failure() {
        let syscall = Syscall::from_raw("syscall_0x123", vec![], -2);
        assert_eq!(syscall.to_string(), "syscall_0x123() = -1 ENOENT");
        assert_eq!(syscall.result, Ret::Errno(Errno::ENOENT));
    }

    #[test]
    fn display_typed_arguments() {
        let syscall = Syscall {
            name: "mystery".to_string(),
            args: vec![
                Arg::Unsigned(42),
                Arg::Signed(-7),
                Arg::Hex(0xdead),
                Arg::Str(CString::new("a, b").unwrap()),
            ],
            result: Ret::Value(-1),
        };
        assert_eq!(
            syscall.to_string(),
            "mystery(42, -7, 0xdead, \"a, b\") = -1"
        );
    }

    #[test]
    fn parse_arguments() {
        assert_eq!(Arg::from_str("42"), Ok(Arg::Unsigned(42)));
        assert_eq!(Arg::from_str("-7"), Ok(Arg::Signed(-7)));
        assert_eq!(Arg::from_str("0xdead"), Ok(Arg::Hex(0xdead)));
        assert_eq!(
            Arg::from_str("\"a, b\""),
            Ok(Arg::Str(CString::new("a, b").unwrap()))
        );
        assert_eq!(
            Arg::from_str("[1, \"a\", 0x2]"),
            Ok(Arg::Array(vec![
                Arg::Unsigned(1),
                Arg::Str(CString::new("a").unwrap()),
                Arg::Hex(2),
            ]))
        );
        assert_eq!(Arg::from_str("[]"), Ok(Arg::Array(Vec::new())));
        assert!(Arg::from_str("").is_err());
        assert!(Arg::from_str("0xzz").is_err());
        assert!(Arg::from_str("null").is_err());
    }

    #[test]
    fn parse_success() {
        let syscall = Syscall::from_str("cacheflush(0x1000, 0x20, 0x3) = 0").unwrap();
        assert_eq!(syscall.name, "cacheflush");
        assert_eq!(
            syscall.args,
            vec![Arg::Hex(0x1000), Arg::Hex(0x20), Arg::Hex(0x3)]
        );
        assert_eq!(syscall.result, Ret::Value(0));
    }

    #[test]
    fn parse_nested_and_quoted() {
        let syscall =
            Syscall::from_str("mystery(1, -2, \"a, b\", [\"x\", \"y\"]) = -1 EINVAL").unwrap();
        assert_eq!(syscall.args[0], Arg::Unsigned(1));
        assert_eq!(syscall.args[1], Arg::Signed(-2));
        assert_eq!(syscall.args[2], Arg::Str(CString::new("a, b").unwrap()));
        assert_eq!(syscall.result, Ret::Errno(Errno::EINVAL));
    }

    #[test]
    fn parse_empty_arguments() {
        let syscall = Syscall::from_str("mystery() = 3").unwrap();
        assert!(syscall.args.is_empty());
        assert_eq!(syscall.result, Ret::Value(3));
    }

    #[test]
    fn parse_invalid() {
        assert!(Syscall::from_str("mystery(0x1) 0").is_err());
        assert!(Syscall::from_str("mystery 0x1) = 0").is_err());
        assert!(Syscall::from_str("(0x1) = 0").is_err());
        assert!(Syscall::from_str("my stery(0x1) = 0").is_err());
        assert!(Syscall::from_str("mystery(0xzz) = 0").is_err());
        assert!(Syscall::from_str("").is_err());
    }

    #[test]
    fn round_trip() {
        for line in [
            "cacheflush(0x1000, 0x20, 0x3) = 0",
            "syscall_0x123() = -1 ENOENT",
            "mystery(42, -7, 0xdead, \"a, b\") = -1",
            "mystery(1, 0x2, \"quote\\\"here\", [\"x\", \"y\"]) = 4294967296",
        ] {
            assert_eq!(Syscall::from_str(line).unwrap().to_string(), line);
        }
    }
}
