// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Formatting and parsing of named bit-flag arguments.
//!
//! Several system calls take an integer argument whose bits `strace` prints
//! by name, joined by `|`. A value of zero is printed as `0` when no named
//! bit carries meaning on its own (unlike `O_RDONLY`, which
//! [`OpenFlags`](super::open::OpenFlags) names explicitly). When at least one
//! named bit is set, remaining unnamed bits are printed in hexadecimal. When
//! no named bit is set, `strace` prints the unnamed bits as hexadecimal
//! followed by a comment naming the flag family, such as
//! `0x2000 /* AT_??? */`; `dflt` reproduces that comment.

use super::parse::{self, ParseError};
use std::fmt;

/// Writes `value` as the names in `names` joined by `|`, or `0` when no bit
/// is set. Unnamed bits are appended as `0x`-prefixed hexadecimal; when no
/// named bit matched at all, `dflt` is appended in a `/* ... */` comment, as
/// `strace` does.
pub(crate) fn display(
    f: &mut fmt::Formatter<'_>,
    value: u32,
    names: &[(u32, &str)],
    dflt: &str,
) -> fmt::Result {
    if value == 0 {
        return f.write_str("0");
    }
    let mut separator = "";
    let mut known = 0u32;
    for &(bit, name) in names {
        if bit != 0 && value & bit == bit {
            write!(f, "{separator}{name}")?;
            separator = "|";
            known |= bit;
        }
    }
    let unknown = value & !known;
    if unknown != 0 {
        if known == 0 && !dflt.is_empty() {
            write!(f, "0x{unknown:x} /* {dflt} */")?;
        } else {
            write!(f, "{separator}0x{unknown:x}")?;
        }
    }
    Ok(())
}

/// Parses `s` as [`display`] writes it: the names in `names` joined by `|`,
/// `0`, or `0x`-prefixed hexadecimal for unnamed bits. A trailing
/// `/* ... */` comment, as `strace` adds for unnamed bits, is ignored.
pub(crate) fn parse(s: &str, names: &[(u32, &str)], part: &'static str) -> Result<u32, ParseError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ParseError::new(part, s));
    }
    if s == "0" {
        return Ok(0);
    }
    let mut value = 0u32;
    for token in s.split('|') {
        let token = parse::strip_comment(token.trim());
        if let Some(&(bit, _)) = names.iter().find(|&&(_, name)| name == token) {
            value |= bit;
            continue;
        }
        let parsed = if let Some(hex) = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
        {
            u32::from_str_radix(hex, 16)
        } else {
            token.parse::<u32>()
        };
        value |= parsed.map_err(|_| ParseError::new(part, token))?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAMES: &[(u32, &str)] = &[(0o400, "AT_SYMLINK_FOLLOW"), (0o1000, "AT_EMPTY_PATH")];

    /// Formats a value with the test [`NAMES`] into a `String`.
    fn to_string(value: u32) -> String {
        struct Wrap(u32);
        impl fmt::Display for Wrap {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                display(f, self.0, NAMES, "AT_???")
            }
        }
        Wrap(value).to_string()
    }

    #[test]
    fn display_zero() {
        assert_eq!(to_string(0), "0");
    }

    #[test]
    fn display_named() {
        assert_eq!(to_string(0o400), "AT_SYMLINK_FOLLOW");
        assert_eq!(to_string(0o400 | 0o1000), "AT_SYMLINK_FOLLOW|AT_EMPTY_PATH");
    }

    #[test]
    fn display_unknown() {
        assert_eq!(to_string(0o400 | 0o1), "AT_SYMLINK_FOLLOW|0x1");
    }

    #[test]
    fn display_unknown_only() {
        assert_eq!(to_string(0x2000), "0x2000 /* AT_??? */");
    }

    #[test]
    fn parse_zero_and_named() {
        assert_eq!(parse("0", NAMES, "flags"), Ok(0));
        assert_eq!(parse("AT_SYMLINK_FOLLOW", NAMES, "flags"), Ok(0o400));
        assert_eq!(
            parse("AT_SYMLINK_FOLLOW|AT_EMPTY_PATH", NAMES, "flags"),
            Ok(0o400 | 0o1000)
        );
        assert_eq!(parse("0x1", NAMES, "flags"), Ok(1));
        assert!(parse("AT_BOGUS", NAMES, "flags").is_err());
        assert!(parse("", NAMES, "flags").is_err());
        assert!(parse("AT_SYMLINK_FOLLOW|", NAMES, "flags").is_err());
    }

    #[test]
    fn parse_unknown_only_comment() {
        assert_eq!(parse("0x2000 /* AT_??? */", NAMES, "flags"), Ok(0x2000));
        assert_eq!(
            parse("AT_SYMLINK_FOLLOW|0x1 /* AT_??? */", NAMES, "flags"),
            Ok(0o400 | 0o1)
        );
    }
}
