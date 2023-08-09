use std::num::ParseIntError;
use std::{collections::HashMap, fmt::Debug};

use nom::combinator::eof;
use nom::multi::many0;
use nom::{
    branch::alt,
    bytes::complete::take,
    character::complete::{char, digit1},
    combinator::recognize,
    error::{ErrorKind, ParseError},
    multi::many_till,
    sequence::{delimited, pair, preceded},
    Err, IResult,
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

    pub fn parse(input: &[u8]) -> Result<Vec<Value>, Err<BencodeError<&[u8]>>> {
        let (next, result) = many0(alt((
            Value::parse_bytes,
            Value::parse_integer,
            Value::parse_list,
            Value::parse_dict,
        )))(input)?;

        let _ = eof(next)?;
        Ok(result)
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
    fn parse_list() {
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
    fn parse_list_errors() {
        let v = Value::parse_list(b"123").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_list(b"l123").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse_list(b"li1e").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));
    }

    #[test]
    fn parse() {
        let v = Value::parse(b"d3:foo3:bar5:hello5:worlde").unwrap();
        let v = v.first().unwrap();
        assert_matches!(v, Value::Dictionary(_));

        if let Value::Dictionary(dict) = v {
            let v = dict.get(b"foo".as_slice()).unwrap();
            assert_matches!(*v, Value::Bytes(b"bar"));

            let v = dict.get(b"hello".as_slice()).unwrap();
            assert_matches!(*v, Value::Bytes(b"world"));
        }

        let (_, v) = Value::parse_dict(b"d4:spaml1:a1:bee").unwrap();
        assert_matches!(v, Value::Dictionary(_));

        if let Value::Dictionary(dict) = v {
            let v = dict.get(b"spam".as_slice()).unwrap();
            assert_matches!(*v, Value::List(_));
        }
    }

    #[test]
    fn parse_errors() {
        let v = Value::parse(b"123").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));

        let v = Value::parse(b"d3:foo3:bar5:hello5:world").unwrap_err();
        assert_matches!(v, nom::Err::Error(BencodeError::Nom(..)));
    }

    #[test]
    fn test_parse_torrent() {
        let data = Value::parse(include_bytes!("../test-assets/test.torrent")).unwrap();
        assert_eq!(data.len(), 1);

        let v = data.first().unwrap();
        assert_matches!(*v, Value::Dictionary(_));

        if let Value::Dictionary(dict) = v {
            let info = dict.get(b"info".as_slice()).unwrap();
            assert_matches!(*info, Value::Dictionary(_));
            if let Value::Dictionary(info) = info {
                let v = info.get(b"length".as_slice()).unwrap();
                assert_matches!(*v, Value::Integer(655360000));
                let v = info.get(b"name".as_slice()).unwrap();
                assert_matches!(*v, Value::Bytes(b"debian-mac-12.1.0-amd64-netinst.iso"));
            }

            let announce = dict.get(b"announce".as_slice()).unwrap();
            assert_matches!(*announce, Value::Bytes(_));

            if let Value::Bytes(announce) = *announce {
                let str = std::str::from_utf8(announce).unwrap();
                assert_eq!(str, "http://bttracker.debian.org:6969/announce");
            }

            let created_by = dict.get(b"created by".as_slice()).unwrap();
            assert_matches!(created_by, Value::Bytes(_));

            if let Value::Bytes(created_by) = *created_by {
                let str = std::str::from_utf8(created_by).unwrap();
                assert_eq!(str, "mktorrent 1.1");
            }
        }
    }
}
