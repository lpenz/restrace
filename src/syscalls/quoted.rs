// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! A quoted string value decoded from a buffer.

use super::buffer::BufferValue;
use super::parse::{self, ParseError};
use std::ffi::CString;
use std::fmt;
use std::str::FromStr;

/// A string that `strace` prints as a double-quoted, escaped C string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quoted(pub CString);

impl Quoted {
    /// Creates a new `Quoted` value, failing if `value` contains an interior
    /// NUL byte.
    pub fn new(value: impl Into<Vec<u8>>) -> Result<Self, ParseError> {
        CString::new(value)
            .map(Self)
            .map_err(|_| ParseError::new("quoted string", ""))
    }
}

impl fmt::Display for Quoted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0.to_string_lossy())
    }
}

impl FromStr for Quoted {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        Self::decode_buffer(s)
    }
}

impl BufferValue for Quoted {
    fn decode_buffer(input: &str) -> Result<Self, ParseError> {
        let s = parse::unquote(input.trim(), "quoted string")?;
        CString::new(s)
            .map(Self)
            .map_err(|_| ParseError::new("quoted string", input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_parse() {
        let q = Quoted::new(b"/etc/hostname").unwrap();
        let rendered = q.to_string();
        assert_eq!(rendered, "\"/etc/hostname\"");
        assert_eq!(Quoted::from_str(&rendered).unwrap(), q);
    }

    #[test]
    fn round_trip_simple() {
        let q = Quoted::new(b"/tmp/file").unwrap();
        assert_eq!(Quoted::from_str(&q.to_string()).unwrap(), q);
    }
}
