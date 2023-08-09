use std::num::ParseIntError;
use std::{collections::HashMap, fmt::Debug};

use nom::{
    branch::alt,
    bytes::complete::take,
    character::complete::{char, digit1},
    combinator::recognize,
    error::{ErrorKind, ParseError},
    multi::many_till,
    sequence::{delimited, pair, preceded},
    IResult,
};

#[derive(Debug)]
pub enum BencodeError<I> {
    Nom(I, ErrorKind),
    InvalidInteger(I),
    ParseIntError(I, ParseIntError),
    InvalidBytesLength(I),
}

impl<I> ParseError<I> for BencodeError<I> {
    fn from_error_kind(input: I, kind: nom::error::ErrorKind) -> Self {
        Self::Nom(input, kind)
    }
    fn append(_: I, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

impl<I> From<BencodeError<I>> for nom::Err<BencodeError<I>> {
    fn from(value: BencodeError<I>) -> Self {
        match value {
            value @ BencodeError::Nom(_, _) => Self::Error(value),
            value => Self::Failure(value),
        }
    }
}

type BencodeResult<'a> = IResult<&'a [u8], Value<'a>, BencodeError<&'a [u8]>>;

#[derive(Debug, Clone)]
pub enum Value<'a> {
    Bytes(&'a [u8]),
    Integer(i64),
    List(Vec<Self>),
    Dictionary(HashMap<&'a [u8], Self>),
}

impl<'a> Value<'a> {
    fn parse_integer(input: &'a [u8]) -> BencodeResult {
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
            Err(nom::Err::Failure(BencodeError::InvalidInteger(input)))?
        }

        let item: i64 = str
            .parse()
            .map_err(|e| BencodeError::ParseIntError(next, e))?;
        Ok((next, Value::Integer(item)))
    }

    fn parse_bytes(input: &'a [u8]) -> BencodeResult<'a> {
        let (input, length) = digit1(input)?;
        let (input, _) = char(':')(input)?;

        let length = std::str::from_utf8(length).expect("value should be a valid integer string");
        let length: u64 = length
            .parse()
            .map_err(|e| BencodeError::ParseIntError(input, e))?;

        if length == 0 {
            Err(BencodeError::InvalidBytesLength(input))?;
        }

        let (next, item) = take(length)(input)?;
        Ok((next, Value::Bytes(item)))
    }

    fn parse_list(input: &'a [u8]) -> BencodeResult<'a> {
        let (next, item) = preceded(
            char('l'),
            many_till(
                alt((
                    Self::parse_bytes,
                    Self::parse_integer,
                    Self::parse_list,
                    Self::parse_dict,
                )),
                char('e'),
            ),
        )(input)?;
        Ok((next, Value::List(item.0)))
    }

    fn parse_dict(input: &'a [u8]) -> BencodeResult<'a> {
        let (next, value) = preceded(
            char('d'),
            many_till(
                pair(
                    Self::parse_bytes,
                    alt((
                        Self::parse_bytes,
                        Self::parse_integer,
                        Self::parse_list,
                        Self::parse_dict,
                    )),
                ),
                char('e'),
            ),
        )(input)?;

        let data = value.0.into_iter().map(|x| {
            if let Value::Bytes(key) = x.0 {
                (key, x.1)
            } else {
                unreachable!()
            }
        });
        Ok((next, Value::Dictionary(data.collect())))
    }
}
#[cfg(test)]
mod tests {
    use crate::{BencodeError, Value};
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
        assert_matches!(v, nom::Err::Failure(BencodeError::InvalidInteger(_)));

        let v = Value::parse_integer(b"i00e").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BencodeError::InvalidInteger(_)));

        let v = Value::parse_integer(b"i-00e").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BencodeError::InvalidInteger(_)));

        let v = Value::parse_integer(b"i01e").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BencodeError::InvalidInteger(_)));

        let v = Value::parse_integer(b"i0123e").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BencodeError::InvalidInteger(_)));

        let v = Value::parse_integer(b"l1i2ee").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));
    }

    #[test]
    fn parse_bytes() {
        let (_, v) = Value::parse_bytes(b"2:qt").unwrap();
        assert_matches!(v, Value::Bytes(b"qt"));

        let (_, v) = Value::parse_bytes(b"2:rust").unwrap();
        assert_matches!(v, Value::Bytes(b"ru"));

        let (_, v) = Value::parse_bytes(b"3:joker").unwrap();
        assert_matches!(v, Value::Bytes(b"jok"));

        let (_, v) = Value::parse_bytes(b"4:forest").unwrap();
        assert_matches!(v, Value::Bytes(b"fore"));
    }

    #[test]
    fn parse_bytes_errors() {
        let v = Value::parse_bytes(b"error").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_bytes(b"x:error").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_bytes(b"7:error").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_bytes(b"-1:error").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_bytes(b"-0:error").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_bytes(b"00:error").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BencodeError::InvalidBytesLength(_)));

        let v = Value::parse_bytes(b"0:error").unwrap_err();
        assert_matches!(v, nom::Err::Failure(BencodeError::InvalidBytesLength(_)));
    }

    #[test]
    fn parse_dict() {
        let (_, v) = Value::parse_dict(b"d3:bar4:spam3:fooli42eee").unwrap();
        assert_matches!(v, Value::Dictionary(_));

        if let Value::Dictionary(dict) = v {
            let v = dict.get(b"bar".as_slice()).unwrap();
            assert_matches!(*v, Value::Bytes(b"spam"));

            let v = dict.get(b"foo".as_slice()).unwrap();
            assert_matches!(*v, Value::List(_));

            if let Value::List(v) = v {
                let mut it = v.iter();
                let x = it.next().unwrap();
                assert_matches!(*x, Value::Integer(42));
            }
        }
    }

    #[test]
    fn parse_dict_errors() {
        let v = Value::parse_dict(b"123").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_dict(b"d123").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_dict(b"d3:bar4:spam3:fooi42e").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_dict(b"d:bar4:spam3:fooi42e").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));
    }

    #[test]
    fn test_parse_list() {
        let (_, v) = Value::parse_list(b"l4:spami42eli9ei50eed3:bar4:spameee").unwrap();
        assert_matches!(v, Value::List(_));

        if let Value::List(list) = v {
            let mut it = list.iter();

            let x = it.next().unwrap();
            assert_matches!(*x, Value::Bytes(b"spam"));

            let x = it.next().unwrap();
            assert_matches!(*x, Value::Integer(42));

            let x = it.next().unwrap();
            assert_matches!(*x, Value::List(_));

            if let Value::List(list) = x {
                let mut it = list.iter();

                let x = it.next().unwrap();
                assert_matches!(*x, Value::Integer(9));

                let x = it.next().unwrap();
                assert_matches!(*x, Value::Integer(50));
            }

            let x = it.next().unwrap();
            assert_matches!(*x, Value::Dictionary(_));

            if let Value::Dictionary(dict) = x {
                let v = dict.get(b"bar".as_slice()).unwrap();
                assert_matches!(*v, Value::Bytes(b"spam"));
            }
        }

        // empty list should be parsable
        let (_, v) = Value::parse_list(b"le").unwrap();
        assert_matches!(v, Value::List(_));
    }

    #[test]
    fn test_parse_list_errors() {
        let v = Value::parse_list(b"123").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_list(b"l123").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_list(b"li1e").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));
    }
}
