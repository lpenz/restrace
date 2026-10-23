// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! A byte string decoded from a buffer.

use super::buffer::BufferValue;
use super::parse::ParseError;
use std::fmt;
use std::str::FromStr;

/// A byte string that `strace` prints as a double-quoted string.
///
/// `"` and `\` are backslash-escaped, `\n`, `\r`, `\t`, `\v` and `\f` are
/// written as the symbolic escapes, and every other byte outside of printable
/// ASCII is written as `\` followed by one to three octal digits, padded to
/// three digits when the byte that follows is an octal digit. This is how
/// `strace` quotes the contents of a buffer such as the one of
/// [`read(2)`](super::read::Read), which may hold any byte, NUL included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bytes {
    /// The bytes.
    pub bytes: Vec<u8>,
    /// Whether the string was truncated with `...` by strace.
    pub truncated: bool,
}

impl Bytes {
    /// Creates a new `Bytes` value.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            truncated: false,
        }
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes)
    }
}

impl std::ops::Deref for Bytes {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl std::ops::DerefMut for Bytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bytes
    }
}

/// The byte a symbolic escape stands for, as `strace` prints it.
const fn escape_byte(c: char) -> Option<u8> {
    Some(match c {
        '\\' => b'\\',
        '"' => b'"',
        'a' => 0x07,
        'b' => 0x08,
        'f' => 0x0c,
        'n' => 0x0a,
        'r' => 0x0d,
        't' => 0x09,
        'v' => 0x0b,
        _ => return None,
    })
}

impl fmt::Display for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"")?;
        for (i, &byte) in self.bytes.iter().enumerate() {
            match byte {
                b'"' | b'\\' => write!(f, "\\{}", char::from(byte))?,
                b'\n' => f.write_str("\\n")?,
                b'\r' => f.write_str("\\r")?,
                b'\t' => f.write_str("\\t")?,
                0x0b => f.write_str("\\v")?,
                0x0c => f.write_str("\\f")?,
                0x20..=0x7e => write!(f, "{}", char::from(byte))?,
                _ => {
                    if matches!(self.bytes.get(i + 1).copied(), Some(b'0'..=b'7')) {
                        write!(f, "\\{byte:03o}")?;
                    } else {
                        write!(f, "\\{byte:o}")?;
                    }
                }
            }
        }
        f.write_str("\"")?;
        if self.truncated {
            f.write_str("...")?;
        }
        Ok(())
    }
}

impl BufferValue for Bytes {
    fn decode_buffer(input: &str) -> Result<Self, ParseError> {
        let error = || ParseError::new("byte string", input);
        let input = input.trim();
        let truncated = input.ends_with("...");
        let input = input.strip_suffix("...").unwrap_or(input);
        let body = input
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or_else(error)?;
        let mut out = Vec::with_capacity(body.len());
        let mut chars = body.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                let mut buf_arr = [0_u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf_arr).as_bytes());
                continue;
            }
            let escape = chars.next().ok_or_else(error)?;
            if let Some(byte) = escape_byte(escape) {
                out.push(byte);
                continue;
            }
            match escape {
                'x' => {
                    let hex: String = chars.by_ref().take(2).collect();
                    if hex.len() != 2 {
                        return Err(error());
                    }
                    out.push(u8::from_str_radix(&hex, 16).map_err(|_| error())?);
                }
                '0'..='7' => {
                    let mut value = escape.to_digit(8).ok_or_else(error)?;
                    for _ in 1..3 {
                        let Some(digit) = chars.clone().next().and_then(|next| next.to_digit(8))
                        else {
                            break;
                        };
                        chars.next();
                        value = value * 8 + digit;
                    }
                    out.push(u8::try_from(value).map_err(|_| error())?);
                }
                _ => return Err(error()),
            }
        }
        Ok(Self {
            bytes: out,
            truncated,
        })
    }
}

/// Parses a byte string as [`Display`](fmt::Display) writes it.
impl FromStr for Bytes {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, ParseError> {
        Self::decode_buffer(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(value: &[u8]) -> Bytes {
        Bytes::new(value.to_vec())
    }

    #[test]
    fn display_printable() {
        assert_eq!(bytes(b"hello").to_string(), "\"hello\"");
        assert_eq!(bytes(b"a, b [c]").to_string(), "\"a, b [c]\"");
    }

    #[test]
    fn display_escapes() {
        assert_eq!(bytes(b"a\"b\\c").to_string(), r#""a\"b\\c""#);
        assert_eq!(bytes(b"line\n\rt\tx").to_string(), r#""line\n\rt\tx""#);
        assert_eq!(bytes(&[0x0b, 0x0c]).to_string(), r#""\v\f""#);
    }

    #[test]
    fn display_octal() {
        assert_eq!(bytes(&[0x01]).to_string(), r#""\1""#);
        assert_eq!(bytes(&[0x0a]).to_string(), r#""\n""#);
        assert_eq!(bytes(&[0x09]).to_string(), r#""\t""#);
        assert_eq!(bytes(&[0x07]).to_string(), r#""\7""#);
        assert_eq!(bytes(&[0x1e]).to_string(), r#""\36""#);
        assert_eq!(bytes(&[0xff]).to_string(), r#""\377""#);
        assert_eq!(bytes(&[0x01, b'5']).to_string(), r#""\0015""#);
        assert_eq!(bytes(&[0x01, b'a']).to_string(), r#""\1a""#);
        assert_eq!(bytes(&[0x00]).to_string(), r#""\0""#);
    }

    #[test]
    fn display_empty() {
        assert_eq!(bytes(b"").to_string(), "\"\"");
    }

    #[test]
    fn parse_escapes() {
        assert_eq!(Bytes::from_str(r#""a\"b\\c""#).unwrap(), bytes(b"a\"b\\c"));
        assert_eq!(Bytes::from_str(r#""line\n""#).unwrap(), bytes(b"line\n"));
        assert_eq!(Bytes::from_str(r#""\v\f""#).unwrap(), bytes(&[0x0b, 0x0c]));
        assert_eq!(Bytes::from_str(r#""\0015""#).unwrap(), bytes(&[0x01, b'5']));
        assert_eq!(Bytes::from_str(r#""\377""#).unwrap(), bytes(&[0xff]));
        assert_eq!(Bytes::from_str(r#""\x41""#).unwrap(), bytes(b"A"));
        assert_eq!(Bytes::from_str("\"\"").unwrap(), bytes(b""));
    }

    #[test]
    fn parse_invalid() {
        assert!(Bytes::from_str("").is_err());
        assert!(Bytes::from_str("\"abc").is_err());
        assert!(Bytes::from_str("abc\"").is_err());
        assert!(Bytes::from_str("\"a\\q\"").is_err());
        assert!(Bytes::from_str("\"a\\x4\"").is_err());
        assert!(Bytes::from_str("\"a\\xzz\"").is_err());
        assert!(Bytes::from_str("\"a\\400\"").is_err());
        assert!(Bytes::from_str("\"a\\").is_err());
    }

    #[test]
    fn round_trip() {
        for value in [
            b"hello".to_vec(),
            b"".to_vec(),
            b"a, b".to_vec(),
            b"a\"b\\c".to_vec(),
            b"line\n\r\t".to_vec(),
            vec![0x01, 0x35],
            vec![0x01, b'a'],
            vec![0xff],
            vec![0x00],
            vec![0x00, 0x30],
        ] {
            let value = bytes(&value);
            assert_eq!(Bytes::from_str(&value.to_string()).unwrap(), value);
        }
    }
}
