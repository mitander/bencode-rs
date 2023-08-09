use std::num::ParseIntError;
use std::{collections::HashMap, fmt::Debug};

use nom::{
    branch::alt,
    character::complete::{char, digit1},
    error::{ErrorKind, ParseError},
    sequence::{delimited, pair},
    IResult,
};

#[derive(Debug)]
pub enum BenError<I> {
    Nom(I, ErrorKind),
    InvalidInteger(I),
    ParseIntError(I, ParseIntError),
}

impl<I> ParseError<I> for BenError<I> {
    fn from_error_kind(input: I, kind: nom::error::ErrorKind) -> Self {
        Self::Nom(input, kind)
    }
    fn append(_: I, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

impl<I> From<BenError<I>> for nom::Err<BenError<I>> {
    fn from(value: BenError<I>) -> Self {
        match value {
            value @ BenError::Nom(_, _) => Self::Error(value),
            value => Self::Failure(value),
        }
    }
}

type BenResult<'a> = IResult<&'a [u8], Value<'a>, BenError<&'a [u8]>>;

#[derive(Debug, Clone)]
pub enum Value<'a> {
    Bytes(&'a [u8]),
    Integer(i64),
    List(Vec<Self>),
    Dictionary(HashMap<&'a [u8], Self>),
}

impl<'a> Value<'a> {
    fn parse_integer(input: &'a [u8]) -> BenResult {
        let (next, item) = delimited(
            char('i'),
            alt((
                recognize(pair(char('+'), digit1)),
                recognize(pair(char('-'), digit1)),
                digit1,
            )),
            char('e'),
        )(input)?;

        let str = std::str::from_utf8(item).expect("value should be a valid integer string");

        if str.starts_with("-0") || (str.starts_with('0') && str.len() > 1) {
            Err(nom::Err::Failure(BenError::InvalidInteger(input)))?
        }

        let item: i64 = str.parse().map_err(|e| BenError::ParseIntError(next, e))?;
        Ok((next, Value::Integer(item)))
    }

    fn parse_bytes(input: &'a [u8]) -> BenResult<'a> {
        !todo!()
    }

    fn parse_list(input: &'a [u8]) -> BenResult<'a> {
        !todo!()
    }

    fn parse_dict(input: &'a [u8]) -> BenResult<'a> {
        !todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::{BenError, Value};
    use assert_matches::assert_matches;

    #[test]
    fn parse_integer() {
        let (_, v) = Value::parse_integer(b"i5e").unwrap();
        assert_matches!(v, Value::Integer(5));

        let (_, v) = Value::parse_integer(b"i1337e1:a").unwrap();
        assert_matches!(v, Value::Integer(1337));

        let (_, v) = Value::parse_integer(b"i-9e").unwrap();
        assert_matches!(v, Value::Integer(-9));

        let (_, v) = Value::parse_integer(b"i123123e").unwrap();
        assert_matches!(v, Value::Integer(123_123));
    }

    #[test]
    fn parse_integer_errors() {
        let v = Value::parse_integer(b"i-0e").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BenError::InvalidInteger(_)));

        let v = Value::parse_integer(b"i00e").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BenError::InvalidInteger(_)));

        let v = Value::parse_integer(b"i-00e").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BenError::InvalidInteger(_)));

        let v = Value::parse_integer(b"i01e").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BenError::InvalidInteger(_)));

        let v = Value::parse_integer(b"i0123e").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BenError::InvalidInteger(_)));

        let v = Value::parse_integer(b"l1i2ee").unwrap_err();
        assert_matches!(v, nom::Err::Error(BenError::Nom(..)));
    }
}
