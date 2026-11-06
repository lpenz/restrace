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

/// Like [`split_args_nested`], but also ignores commas inside bracketed groups:
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

/// Removes the parenthesized description `strace` appends to an errno, such as
/// the ` (No such file or directory)` in `ENOENT (No such file or directory)`.
pub(crate) fn strip_description(input: &str) -> &str {
    match input.find(" (") {
        Some(i) => input[..i].trim_end(),
        None => input,
    }
}

/// Like [`split_annotation`], but keeps the annotation verbatim: the returned
/// suffix starts at the opening `<` and runs to the end of the input, so it
/// includes any marker `strace` appends after the closing bracket, such as the
/// `(deleted)` of a removed file.
///
/// Returns the bare value and the suffix, or the whole input and `None` when
/// there is no balanced annotation.
pub(crate) fn split_annotation_verbatim(input: &str) -> (&str, Option<&str>) {
    let input = input.trim();
    let Some(open) = input.find('<') else {
        return (input, None);
    };
    let rest = &input[open + 1..];
    let mut depth = 1u32;
    for c in rest.chars() {
        match c {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return (input[..open].trim_end(), Some(&input[open..]));
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
    fn split_annotation_verbatim_plain() {
        assert_eq!(split_annotation_verbatim("3"), ("3", None));
        assert_eq!(split_annotation_verbatim(" 3 "), ("3", None));
    }

    #[test]
    fn split_annotation_verbatim_keeps_suffix() {
        assert_eq!(
            split_annotation_verbatim("10</dev/null>"),
            ("10", Some("</dev/null>"))
        );
        assert_eq!(
            split_annotation_verbatim("3</dev/null<char 1:3>>"),
            ("3", Some("</dev/null<char 1:3>>"))
        );
        assert_eq!(
            split_annotation_verbatim("3</tmp/gone>(deleted)"),
            ("3", Some("</tmp/gone>(deleted)"))
        );
    }

    #[test]
    fn split_annotation_verbatim_malformed() {
        assert_eq!(
            split_annotation_verbatim("3</tmp/unclosed"),
            ("3</tmp/unclosed", None)
        );
    }
}
