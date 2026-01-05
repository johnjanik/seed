//! Lexer/tokenizer for Seed documents.

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::char,
    combinator::{map, opt, recognize},
    sequence::{pair, tuple},
    IResult,
};

/// Parse an identifier (starts with letter/underscore, followed by alphanumeric/underscore/hyphen).
pub fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        take_while1(|c: char| c.is_alphabetic() || c == '_'),
        take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
    ))(input)
}

/// Parse a number (integer or float, optionally negative).
pub fn number(input: &str) -> IResult<&str, f64> {
    map(
        recognize(tuple((
            opt(char('-')),
            take_while1(|c: char| c.is_ascii_digit()),
            opt(pair(char('.'), take_while1(|c: char| c.is_ascii_digit()))),
        ))),
        |s: &str| s.parse().unwrap_or(0.0),
    )(input)
}

/// Parse a unit suffix.
pub fn unit(input: &str) -> IResult<&str, &str> {
    alt((
        tag("px"),
        tag("pt"),
        tag("mm"),
        tag("cm"),
        tag("in"),
        tag("%"),
        tag("em"),
        tag("rem"),
        tag("deg"),
    ))(input)
}

/// Count leading spaces for indentation.
pub fn count_indent(line: &str) -> usize {
    line.chars().take_while(|&c| c == ' ').count()
}

/// A line of input with its indentation level.
#[derive(Debug, Clone)]
pub struct Line<'a> {
    pub indent: usize,
    pub content: &'a str,
    pub line_number: usize,
}

/// Split input into lines with indentation info.
pub fn split_lines(input: &str) -> Vec<Line<'_>> {
    input
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let trimmed = line.trim();
            // Skip empty lines and comment-only lines
            if trimmed.is_empty() || trimmed.starts_with("//") {
                None
            } else {
                Some(Line {
                    indent: count_indent(line),
                    content: trimmed,
                    line_number: i + 1,
                })
            }
        })
        .collect()
}
