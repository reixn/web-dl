pub mod hex {
    use serde::{de, Deserializer, Serializer};
    use std::fmt::Display;

    pub fn serialize<const N: usize, S: Serializer>(
        value: &[u8; N],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            hex::serde::serialize(value, serializer)
        } else {
            serializer.serialize_bytes(value.as_slice())
        }
    }
    pub fn deserialize<'de, const N: usize, D>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        [u8; N]: hex::FromHex,
        <[u8; N] as hex::FromHex>::Error: Display,
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            hex::serde::deserialize(deserializer)
        } else {
            struct Visitor<const N: usize>;
            impl<'de, const N: usize> de::Visitor<'de> for Visitor<N> {
                type Value = [u8; N];
                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    write!(formatter, "byte array of length {}", N)
                }
                fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    <[u8; N]>::try_from(v).map_err(E::custom)
                }
            }
            deserializer.deserialize_bytes(Visitor)
        }
    }
}
