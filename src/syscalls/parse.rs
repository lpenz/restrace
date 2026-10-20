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

/// Like [`split_args`], but also ignores commas inside bracketed groups:
/// `[ ... ]`, `{ ... }` and `( ... )`. This covers the string arrays of
/// `execve(2)`, the structures of `clone3(2)` and calls like `makedev(a, b)`.
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
            '[' | '{' | '(' if !quoted => depth += 1,
            ']' | '}' | ')' if !quoted => depth = depth.saturating_sub(1),
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

/// Removes the parenthesized description `strace` appends to an errno, such as
/// the ` (No such file or directory)` in `ENOENT (No such file or directory)`.
pub(crate) fn strip_description(input: &str) -> &str {
    match input.find(" (") {
        Some(i) => input[..i].trim_end(),
        None => input,
    }
}

/// Removes a trailing comment `strace` appends in some forms, such as
/// the ` /* date */` in timestamps.
pub(crate) fn strip_comment(input: &str) -> &str {
    match input.find(" /*") {
        Some(i) => input[..i].trim_end(),
        None => input,
    }
}

/// Splits the `<...>` associated-info annotation that `strace` appends with
/// `-y` from an argument, returning the bare argument and the annotated text
/// inside the angle brackets, if any.
///
/// `strace` formats an annotated file descriptor as `3</path/to/file>` and,
/// with `-yy`, may nest more associated info inside the brackets, such as the
/// device of a character file (`3</dev/null<char 1:3>>`). A marker that
/// `strace` appends after the closing bracket, such as the `(deleted)` of a
/// removed file, is dropped.
pub(crate) fn split_annotation(input: &str) -> (&str, Option<&str>) {
    let input = input.trim();
    let Some(open) = input.find('<') else {
        return (input, None);
    };
    let value = input[..open].trim_end();
    let rest = &input[open + 1..];
    let mut depth = 1u32;
    for (i, c) in rest.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return (value, Some(&rest[..i]));
                }
            }
            _ => {}
        }
    }
    (input, None)
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
    fn split_nested_structures_and_calls() {
        assert_eq!(
            split_args_nested("\"f\", {st_dev=makedev(0, 0x1e), st_size=8}"),
            ["\"f\"", " {st_dev=makedev(0, 0x1e), st_size=8}"]
        );
        assert_eq!(
            split_args_nested("{d_ino=1, d_name=\"a, b\"}"),
            ["{d_ino=1, d_name=\"a, b\"}"]
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

    #[test]
    fn strip_description_removes_suffix() {
        assert_eq!(
            strip_description("ENOENT (No such file or directory)"),
            "ENOENT"
        );
        assert_eq!(strip_description("EACCES (Permission denied)"), "EACCES");
        assert_eq!(strip_description("9999 (Unknown error)"), "9999");
    }

    #[test]
    fn strip_description_keeps_plain() {
        assert_eq!(strip_description("ENOENT"), "ENOENT");
        assert_eq!(strip_description("5"), "5");
        assert_eq!(strip_description(""), "");
    }

    #[test]
    fn strip_comment_removes_suffix() {
        assert_eq!(
            strip_comment("1754766014 /* 2025-08-09T20:00:14.986788270+0100 */"),
            "1754766014"
        );
        assert_eq!(strip_comment("986788270 /* ... */"), "986788270");
    }

    #[test]
    fn strip_comment_keeps_plain() {
        assert_eq!(strip_comment("1754766014"), "1754766014");
        assert_eq!(strip_comment(""), "");
    }

    #[test]
    fn split_annotation_plain() {
        assert_eq!(split_annotation("3"), ("3", None));
        assert_eq!(split_annotation("AT_FDCWD"), ("AT_FDCWD", None));
    }

    #[test]
    fn split_annotation_path() {
        assert_eq!(split_annotation("3</tmp/file>"), ("3", Some("/tmp/file")));
        assert_eq!(
            split_annotation("AT_FDCWD</home/user>"),
            ("AT_FDCWD", Some("/home/user"))
        );
    }

    #[test]
    fn split_annotation_nested_and_suffix() {
        assert_eq!(
            split_annotation("3</dev/null<char 1:3>>"),
            ("3", Some("/dev/null<char 1:3>"))
        );
        assert_eq!(
            split_annotation("3</tmp/gone>(deleted)"),
            ("3", Some("/tmp/gone"))
        );
        assert_eq!(split_annotation("3<pipe:[123]>"), ("3", Some("pipe:[123]")));
    }

    #[test]
    fn split_annotation_malformed() {
        assert_eq!(
            split_annotation("3</tmp/unclosed"),
            ("3</tmp/unclosed", None)
        );
    }
}
