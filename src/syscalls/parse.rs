// Copyright (C) 2026 Leandro Lisboa Penz <lpenz@lpenz.org>
// This file is subject to the terms and conditions defined in
// file 'LICENSE', which is part of this source code package.

//! Parsing of the representations produced by the `Display` implementations.
//!
//! Every system call type that formats itself also parses itself back, so
//! that trace output can be read in again.

use std::error::Error;
use std::fmt;

/// Error returned when a system call representation cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    part: &'static str,
    input: String,
}

impl ParseError {
    pub(crate) fn new(part: &'static str, input: impl Into<String>) -> Self {
        Self {
            part,
            input: input.into(),
        }
    }

    /// Returns the name of the part that failed to parse.
    #[must_use]
    pub fn part(&self) -> &'static str {
        self.part
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {}: {:?}", self.part, self.input)
    }
}

impl Error for ParseError {}

/// Splits a comma-separated argument list, ignoring commas inside quotes.
pub(crate) fn split_args(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (i, c) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ',' if !quoted => {
                parts.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&input[start..]);
    parts
}

/// Like [`split_args`], but also ignores commas inside `[ ... ]` groups,
/// such as the string arrays of `execve(2)`.
pub(crate) fn split_args_nested(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0u32;
    for (i, c) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '[' if !quoted => depth += 1,
            ']' if !quoted => depth = depth.saturating_sub(1),
            ',' if !quoted && depth == 0 => {
                parts.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&input[start..]);
    parts
}

/// Parses a double-quoted string as formatted by the `{:?}` placeholder.
pub(crate) fn unquote(input: &str, part: &'static str) -> Result<String, ParseError> {
    let error = || ParseError::new(part, input);
    let body = input
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or_else(error)?;
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('0') => out.push('\0'),
            Some('x') => {
                let hex: String = chars.by_ref().take(2).collect();
                let byte = u8::from_str_radix(&hex, 16).map_err(|_| error())?;
                if hex.len() != 2 || !byte.is_ascii() {
                    return Err(error());
                }
                out.push(char::from(byte));
            }
            Some('u') => {
                if chars.next() != Some('{') {
                    return Err(error());
                }
                let mut hex = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    hex.push(c);
                }
                if !closed || hex.is_empty() || hex.len() > 6 {
                    return Err(error());
                }
                let code = u32::from_str_radix(&hex, 16).map_err(|_| error())?;
                out.push(char::from_u32(code).ok_or_else(error)?);
            }
            _ => return Err(error()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_plain() {
        assert_eq!(split_args("a, b, c"), ["a", " b", " c"]);
    }

    #[test]
    fn split_quoted_comma() {
        assert_eq!(split_args("\"a, b\", O_RDONLY"), ["\"a, b\"", " O_RDONLY"]);
    }

    #[test]
    fn split_quoted_escaped_quote() {
        assert_eq!(split_args("\"a\\\",b\", c"), ["\"a\\\",b\"", " c"]);
    }

    #[test]
    fn split_nested_arrays() {
        assert_eq!(
            split_args_nested("\"p\", [\"a\", \"b\"], [\"c\"]"),
            ["\"p\"", " [\"a\", \"b\"]", " [\"c\"]"]
        );
    }

    #[test]
    fn split_nested_quoted_comma() {
        assert_eq!(
            split_args_nested("\"p\", [\"a, b\", \"c\"]"),
            ["\"p\"", " [\"a, b\", \"c\"]"]
        );
    }

    #[test]
    fn unquote_plain() {
        assert_eq!(unquote("\"/tmp/file\"", "pathname").unwrap(), "/tmp/file");
    }

    #[test]
    fn unquote_empty() {
        assert_eq!(unquote("\"\"", "pathname").unwrap(), "");
    }

    #[test]
    fn unquote_escapes() {
        assert_eq!(
            unquote("\"a\\\\b\\\"c\\nd\\x41\\u{263a}\"", "pathname").unwrap(),
            "a\\b\"c\ndA\u{263a}"
        );
    }

    #[test]
    fn unquote_rejects_unquoted() {
        assert!(unquote("/tmp/file", "pathname").is_err());
    }

    #[test]
    fn unquote_rejects_bad_escape() {
        assert!(unquote("\"\\q\"", "pathname").is_err());
        assert!(unquote("\"\\x4\"", "pathname").is_err());
        assert!(unquote("\"\\u{}\"", "pathname").is_err());
        assert!(unquote("\"\\u{41\"", "pathname").is_err());
    }

    #[test]
    fn error_display() {
        let err = ParseError::new("flags", "O_BOGUS");
        assert_eq!(err.to_string(), r#"invalid flags: "O_BOGUS""#);
        assert_eq!(err.part(), "flags");
    }
}
