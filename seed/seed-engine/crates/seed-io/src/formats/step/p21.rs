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

/// Parse a string literal with STEP escape sequence handling.
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
                // End of string - decode Unicode escapes
                let decoded = decode_step_string(&result);
                return Ok((&input[consumed..], decoded));
            }
        } else {
            result.push(c);
        }
    }

    Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char)))
}

/// Decode STEP Unicode escape sequences (\X2\HHHH...\X0\ and \X\HH).
fn decode_step_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.peek() {
                Some('X') => {
                    chars.next(); // consume 'X'
                    match chars.peek() {
                        Some('2') => {
                            // \X2\HHHH...\X0\ - UTF-16 encoding
                            chars.next(); // consume '2'
                            if chars.next() == Some('\\') {
                                // Read hex pairs until \X0\
                                let mut hex_str = String::new();
                                loop {
                                    if let Some(&c1) = chars.peek() {
                                        if c1 == '\\' {
                                            chars.next();
                                            if chars.peek() == Some(&'X') {
                                                chars.next();
                                                if chars.peek() == Some(&'0') {
                                                    chars.next();
                                                    chars.next(); // consume final '\'
                                                    break;
                                                }
                                            }
                                        } else {
                                            hex_str.push(chars.next().unwrap());
                                        }
                                    } else {
                                        break;
                                    }
                                }
                                // Decode hex pairs as UTF-16
                                let mut i = 0;
                                while i + 4 <= hex_str.len() {
                                    if let Ok(code) = u16::from_str_radix(&hex_str[i..i + 4], 16) {
                                        if let Some(ch) = char::from_u32(code as u32) {
                                            result.push(ch);
                                        }
                                    }
                                    i += 4;
                                }
                            }
                        }
                        Some('\\') => {
                            // \X\HH - extended ASCII
                            chars.next(); // consume '\'
                            let h1 = chars.next().unwrap_or('0');
                            let h2 = chars.next().unwrap_or('0');
                            let hex: String = [h1, h2].iter().collect();
                            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                                result.push(byte as char);
                            }
                        }
                        _ => {
                            result.push('\\');
                            result.push('X');
                        }
                    }
                }
                _ => {
                    result.push(c);
                    if let Some(&next) = chars.peek() {
                        result.push(chars.next().unwrap());
                    }
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Parse an enumeration value.
fn enumeration(input: &str) -> IResult<&str, String> {
    delimited(
        char('.'),
        map(take_while1(|c: char| c.is_alphanumeric() || c == '_'), String::from),
        char('.'),
    )(input)
}

/// Parse a typed value: TYPE_NAME(value) or TYPE_NAME(val1, val2, ...).
fn typed_parameter(input: &str) -> IResult<&str, StepValue> {
    let (input, type_name) = take_while1(|c: char| c.is_ascii_uppercase() || c == '_')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, values) = separated_list0(
        tuple((multispace0, char(','), multispace0)),
        step_value,
    )(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char(')')(input)?;

    // If single value, store directly; otherwise wrap in a List
    let inner_value = if values.len() == 1 {
        values.into_iter().next().unwrap()
    } else {
        StepValue::List(values)
    };

    Ok((input, StepValue::Typed {
        type_name: type_name.to_string(),
        value: Box::new(inner_value),
    }))
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
        // Typed parameter: TYPE_NAME(value) - must come before real/list
        typed_parameter,
        // Try real before integer (real is more specific)
        map(real, StepValue::Real),
        // List - handles trailing whitespace before closing paren
        delimited(
            pair(char('('), multispace0),
            map(
                terminated(
                    separated_list0(
                        tuple((multispace0, char(','), multispace0)),
                        step_value,
                    ),
                    multispace0,
                ),
                StepValue::List,
            ),
            char(')'),
        ),
    ))(input)
}

/// Parse a single typed value (TYPE_NAME ( params )).
fn typed_value(input: &str) -> IResult<&str, (String, Vec<StepValue>)> {
    let (input, _) = multispace0(input)?;
    let (input, type_name) = take_while1(|c: char| c.is_alphanumeric() || c == '_')(input)?;
    let (input, _) = multispace0(input)?;
    let (input, params) = delimited(
        pair(char('('), multispace0),
        terminated(
            separated_list0(
                tuple((multispace0, char(','), multispace0)),
                step_value,
            ),
            multispace0,
        ),
        char(')'),
    )(input)?;
    Ok((input, (type_name.to_uppercase(), params)))
}

/// Parse an entity instance line (simple or complex).
pub fn entity_instance(input: &str) -> IResult<&str, EntityInstance> {
    let (input, _) = multispace0(input)?;
    let (input, id) = entity_id(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('=')(input)?;
    let (input, _) = multispace0(input)?;

    // Check if this is a complex entity instance: =( TYPE1(...) TYPE2(...) ... )
    let (input, type_name, params) = if input.starts_with('(') {
        // Complex entity: parse multiple typed values inside parens
        let (input, _) = char('(')(input)?;
        let (input, _) = multispace0(input)?;
        let (input, typed_values) = many0(typed_value)(input)?;
        let (input, _) = multispace0(input)?;
        let (input, _) = char(')')(input)?;

        // Combine into a single entity - use first type as the primary type
        // and flatten all params into one list
        let primary_type = typed_values.first()
            .map(|(t, _)| t.clone())
            .unwrap_or_else(|| "COMPLEX".to_string());
        let all_params: Vec<StepValue> = typed_values.into_iter()
            .flat_map(|(_, p)| p)
            .collect();

        (input, primary_type, all_params)
    } else {
        // Simple entity: TYPE_NAME ( params )
        let (input, (type_name, params)) = typed_value(input)?;
        (input, type_name, params)
    };

    let (input, _) = multispace0(input)?;
    let (input, _) = char(';')(input)?;

    Ok((
        input,
        EntityInstance {
            id,
            type_name,
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

    #[test]
    fn test_crlf_line_endings() {
        let input = "DATA;\r\n#1 = CARTESIAN_POINT ( 'NONE',  ( 31.0, -64.2, -55.0 ) ) ;\r\n#2 = FACE_OUTER_BOUND ( 'NONE', #8586, .T. ) ;\r\nENDSEC;";
        let result = parse_data_section(input);
        assert!(result.is_ok(), "Failed to parse CRLF: {:?}", result);
        let (_, entities) = result.unwrap();
        assert_eq!(entities.len(), 2);
    }

    #[test]
    fn test_complex_entity() {
        let input = "DATA;\r\n#914 =( GEOMETRIC_REPRESENTATION_CONTEXT ( 3 ) GLOBAL_UNIT_ASSIGNED_CONTEXT ( ( #6059, #3996 ) ) );\r\nENDSEC;";
        let result = parse_data_section(input);
        assert!(result.is_ok(), "Failed to parse complex entity: {:?}", result);
        let (_, entities) = result.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].id, 914);
        assert_eq!(entities[0].type_name, "GEOMETRIC_REPRESENTATION_CONTEXT");
    }

    #[test]
    fn test_typed_parameter() {
        let input = "DATA;\r\n#12267 = UNCERTAINTY_MEASURE_WITH_UNIT (LENGTH_MEASURE( 1.0E-05 ), #6059, 'distance_accuracy_value', 'NONE');\r\nENDSEC;";
        let result = parse_data_section(input);
        assert!(result.is_ok(), "Failed to parse typed parameter: {:?}", result);
        let (_, entities) = result.unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].id, 12267);
    }
