use serde::de;
use std::{
    fmt::{self, Display, Formatter},
    mem::MaybeUninit,
};

#[derive(Debug, Copy, Clone)]
pub enum DecodeError {
    InvalidLength { expected: usize, len: usize },
    InvalidChar { pos: usize, char: u8 },
}
impl Display for DecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::InvalidLength { expected, len } => f.write_fmt(format_args!(
                "invalid base16 length: {}, expected {}",
                len, expected
            )),
            DecodeError::InvalidChar { pos, char } => f.write_fmt(format_args!(
                "invalid base16 char: {}, expected {}",
                char, pos
            )),
        }
    }
}
impl std::error::Error for DecodeError {}

pub fn decode_bytes<const N: usize>(input: &str) -> Result<[u8; N], DecodeError> {
    let input = input.as_bytes();
    if input.len() != N * 2 {
        return Err(DecodeError::InvalidLength {
            expected: N * 2,
            len: input.len(),
        });
    }
    let mut buf: [MaybeUninit<u8>; N] = MaybeUninit::uninit_array();
    for i in 0..N {
        let h = base16::decode_byte(input[2 * i]).ok_or(DecodeError::InvalidChar {
            pos: 2 * i,
            char: input[2 * i],
        })?;
        let l = base16::decode_byte(input[2 * i + 1]).ok_or(DecodeError::InvalidChar {
            pos: 2 * i + 1,
            char: input[2 * i + 1],
        })?;
        buf[i].write((h << 4) | l);
    }
    Ok(unsafe { MaybeUninit::array_assume_init(buf) })
}

pub fn fmt<const N: usize>(val: &[u8; N], f: &mut Formatter<'_>) -> fmt::Result {
    for i in val {
        f.write_fmt(format_args!("{:02x}", i))?
    }
    Ok(())
}
pub fn serialize<const N: usize, S>(val: &[u8; N], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if serializer.is_human_readable() {
        serializer.serialize_str(base16::encode_lower(val).as_str())
    } else {
        serializer.serialize_bytes(val)
    }
}
pub fn deserialize<'de, const N: usize, D>(deserializer: D) -> Result<[u8; N], D::Error>
where
    D: de::Deserializer<'de>,
{
    struct ByteVisitor<const N: usize>;
    impl<'d, const N: usize> de::Visitor<'d> for ByteVisitor<N> {
        type Value = [u8; N];
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_fmt(format_args!("{} bits hash digest", N * 8))
        }
        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            decode_bytes(v).map_err(de::Error::custom)
        }
        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.len() != N {
                Err(de::Error::invalid_length(v.len(), &self))
            } else {
                let mut buf: [MaybeUninit<u8>; N] = MaybeUninit::uninit_array();
                MaybeUninit::write_slice(&mut buf, v);
                Ok(unsafe { MaybeUninit::array_assume_init(buf) })
            }
        }
    }
    if deserializer.is_human_readable() {
        deserializer.deserialize_str(ByteVisitor::<N>)
    } else {
        deserializer.deserialize_bytes(ByteVisitor::<N>)
    }
}
