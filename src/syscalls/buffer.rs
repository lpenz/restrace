// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Output buffer arguments that `strace` decodes or shows as an address.

use super::parse::ParseError;
use std::fmt;

/// An output buffer argument of a system call.
///
/// `strace` replaces the argument with the value it read back from the traced
/// process when it can decode it, and shows the raw address otherwise, such
/// as after a failed call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Buffer<T> {
    /// The value `strace` read back from the buffer.
    Decoded(T),
    /// The raw address of the buffer, shown when it could not be decoded.
    Address(u64),
}

impl<T> Buffer<T> {
    /// Returns the decoded value, if the buffer was decoded.
    #[must_use]
    pub fn decoded(&self) -> Option<&T> {
        match self {
            Self::Decoded(value) => Some(value),
            Self::Address(_) => None,
        }
    }
}

impl<T: fmt::Display> fmt::Display for Buffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decoded(value) => write!(f, "{value}"),
            Self::Address(0) => f.write_str("NULL"),
            Self::Address(address) => write!(f, "0x{address:x}"),
        }
    }
}

/// A value that `strace` can decode into an output buffer argument.
pub(crate) trait BufferValue: Sized {
    /// Parses the decoded value as `strace` prints it.
    fn decode_buffer(input: &str) -> Result<Self, ParseError>;
}

/// Parses a buffer argument as [`Display`](fmt::Display) writes it: the
/// decoded value, or `NULL`/`0x`-prefixed hexadecimal for the raw address.
pub(crate) fn parse<T: BufferValue>(s: &str) -> Result<Buffer<T>, ParseError> {
    let s = s.trim();
    if s == "NULL" {
        return Ok(Buffer::Address(0));
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16)
            .map(Buffer::Address)
            .map_err(|_| ParseError::new("address", s));
    }
    T::decode_buffer(s).map(Buffer::Decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Word(String);

    impl fmt::Display for Word {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl BufferValue for Word {
        fn decode_buffer(input: &str) -> Result<Self, ParseError> {
            Ok(Word(input.trim().to_string()))
        }
    }

    fn word(s: &str) -> Word {
        Word(s.to_string())
    }

    #[test]
    fn display_decoded() {
        assert_eq!(Buffer::Decoded(word("target")).to_string(), "target");
    }

    #[test]
    fn display_address() {
        assert_eq!(Buffer::<Word>::Address(0x7ffc).to_string(), "0x7ffc");
        assert_eq!(Buffer::<Word>::Address(0).to_string(), "NULL");
    }

    #[test]
    fn parse_decoded() {
        assert_eq!(parse::<Word>("target"), Ok(Buffer::Decoded(word("target"))));
    }

    #[test]
    fn parse_address() {
        assert_eq!(parse::<Word>("0x7ffc"), Ok(Buffer::<Word>::Address(0x7ffc)));
        assert_eq!(parse::<Word>("NULL"), Ok(Buffer::<Word>::Address(0)));
        assert!(parse::<Word>("0xzz").is_err());
    }

    #[test]
    fn decoded_accessor() {
        let decoded = Buffer::Decoded(word("abc"));
        assert_eq!(decoded.decoded(), Some(&word("abc")));
        assert_eq!(Buffer::<Word>::Address(0x10).decoded(), None);
    }

    #[test]
    fn round_trip() {
        for buffer in [
            Buffer::Decoded(word("a/b")),
            Buffer::<Word>::Address(0x7ffc1234),
            Buffer::<Word>::Address(0),
        ] {
            assert_eq!(parse::<Word>(&buffer.to_string()).unwrap(), buffer);
        }
    }
}
