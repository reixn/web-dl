pub mod if_readable {
    use serde::{Deserializer, Serializer};

    pub fn serialize<T: serde_bytes::Serialize, S: Serializer>(
        value: &Option<T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_none()
        } else {
            serde_bytes::serialize(value, serializer)
        }
    }
    pub fn deserialize<'de, T: serde_bytes::Deserialize<'de>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<T>, D::Error> {
        serde_bytes::deserialize(deserializer)
    }
}
