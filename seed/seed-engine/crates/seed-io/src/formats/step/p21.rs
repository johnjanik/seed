//! Part 21 (ISO 10303-21) physical file format parser.
//!
//! This module implements a pure Rust parser for STEP files using nom combinators.
//! Some parser functions and variants are defined for spec completeness.

#![allow(dead_code)]

use nom::{
    branch::alt,
    bytes::complete::{tag, take_until, take_while1},
    character::complete::{char, digit1, multispace0, one_of},
    combinator::{map, opt, recognize, value},
    multi::{many0, separated_list0},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};

/// A STEP entity instance.
#[derive(Debug, Clone)]
pub struct EntityInstance {
    /// Entity ID (e.g., #123)
    pub id: u64,
    /// Entity type name (e.g., "CARTESIAN_POINT")
    pub type_name: String,
    /// Entity parameters
    pub params: Vec<StepValue>,
}

/// A STEP value in a parameter list.
#[derive(Debug, Clone)]
pub enum StepValue {
    /// Integer value
    Integer(i64),
    /// Real/float value
    Real(f64),
    /// String value
    String(String),
    /// Entity reference (#123)
    Reference(u64),
    /// Enumeration (.VALUE.)
    Enum(String),
    /// List of values
    List(Vec<StepValue>),
    /// Omitted/unset value ($)
    Omitted,
    /// Derived value (*)
    Derived,
    /// Typed value (TYPE(...))
    Typed { type_name: String, value: Box<StepValue> },
}

/// Parse a STEP entity ID (#123).
fn entity_id(input: &str) -> IResult<&str, u64> {
    preceded(char('#'), map(digit1, |s: &str| s.parse().unwrap()))(input)
}

/// Parse an integer.
fn integer(input: &str) -> IResult<&str, i64> {
    map(
        recognize(pair(opt(one_of("+-")), digit1)),
        |s: &str| s.parse().unwrap(),
    )(input)
}

/// Parse a real number.
fn real(input: &str) -> IResult<&str, f64> {
    map(
        recognize(tuple((
            opt(one_of("+-")),
            digit1,
            opt(pair(char('.'), opt(digit1))), // Allow "0." without trailing digits
            opt(tuple((one_of("eE"), opt(one_of("+-")), digit1))),
        ))),
        |s: &str| s.parse().unwrap(),
    )(input)
}

/// Parse a string literal.
fn string_literal(input: &str) -> IResult<&str, String> {
    let (input, _) = char('\'')(input)?;
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    let mut consumed = 0;

    while let Some(c) = chars.next() {
        consumed += c.len_utf8();
        if c == '\'' {
            // Check for escaped quote
            if chars.peek() == Some(&'\'') {
                result.push('\'');
                chars.next();
                consumed += 1;
            } else {
                // End of string
                return Ok((&input[consumed..], result));
            }
        } else {
            result.push(c);
        }
    }

    Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char)))
}

/// Parse an enumeration value.
fn enumeration(input: &str) -> IResult<&str, String> {
    delimited(
        char('.'),
        map(take_while1(|c: char| c.is_alphanumeric() || c == '_'), String::from),
        char('.'),
    )(input)
}

/// Parse a STEP value.
fn step_value(input: &str) -> IResult<&str, StepValue> {
    let (input, _) = multispace0(input)?;

    alt((
        value(StepValue::Omitted, char('$')),
        value(StepValue::Derived, char('*')),
        map(entity_id, StepValue::Reference),
        map(enumeration, StepValue::Enum),
        map(string_literal, StepValue::String),
        // Try real before integer (real is more specific)
        map(real, StepValue::Real),
        // List
        delimited(
            char('('),
            map(
                separated_list0(
                    tuple((multispace0, char(','), multispace0)),
                    step_value,
                ),
                StepValue::List,
            ),
            char(')'),
        ),
    ))(input)
}

/// Parse an entity instance line.
pub fn entity_instance(input: &str) -> IResult<&str, EntityInstance> {
    let (input, _) = multispace0(input)?;
    let (input, id) = entity_id(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('=')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, type_name) = take_while1(|c: char| c.is_alphanumeric() || c == '_')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, params) = delimited(
        char('('),
        separated_list0(
            tuple((multispace0, char(','), multispace0)),
            step_value,
        ),
        char(')'),
    )(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char(';')(input)?;

    Ok((
        input,
        EntityInstance {
            id,
            type_name: type_name.to_uppercase(),
            params,
        },
    ))
}

/// Parse the DATA section of a STEP file.
pub fn parse_data_section(input: &str) -> IResult<&str, Vec<EntityInstance>> {
    let (input, _) = take_until("DATA;")(input)?;
    let (input, _) = tag("DATA;")(input)?;
    let (input, _) = multispace0(input)?;

    let (input, entities) = many0(terminated(entity_instance, multispace0))(input)?;

    let (input, _) = tag("ENDSEC;")(input)?;

    Ok((input, entities))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_id() {
        assert_eq!(entity_id("#123"), Ok(("", 123)));
        assert_eq!(entity_id("#1"), Ok(("", 1)));
    }

    #[test]
    fn test_real() {
        assert!((real("3.14").unwrap().1 - 3.14).abs() < 0.001);
        assert!((real("1.0E-5").unwrap().1 - 1.0e-5).abs() < 1e-10);
    }

    #[test]
    fn test_string_literal() {
        assert_eq!(string_literal("'hello'"), Ok(("", "hello".to_string())));
        assert_eq!(string_literal("'it''s'"), Ok(("", "it's".to_string())));
    }

    #[test]
    fn test_enumeration() {
        assert_eq!(enumeration(".T."), Ok(("", "T".to_string())));
        assert_eq!(enumeration(".PLANE."), Ok(("", "PLANE".to_string())));
    }

    #[test]
    fn test_entity_instance() {
        let input = "#10 = CARTESIAN_POINT('origin', (0., 0., 0.));";
        let result = entity_instance(input);
        assert!(result.is_ok(), "Parse failed: {:?}", result);
        let (_, entity) = result.unwrap();
        assert_eq!(entity.id, 10);
        assert_eq!(entity.type_name, "CARTESIAN_POINT");
        assert_eq!(entity.params.len(), 2);
    }
}
