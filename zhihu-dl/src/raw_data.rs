use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use serde::{de, Deserialize, Deserializer, Serialize};
use web_dl_base::{media::Image, storable::Storable};

pub(crate) struct StrU64(pub u64);
impl<'de> Deserialize<'de> for StrU64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IntVisitor;
        impl<'de> de::Visitor<'de> for IntVisitor {
            type Value = StrU64;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("u64 as string")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v.parse() {
                    Ok(v) => Ok(StrU64(v)),
                    Err(e) => Err(E::custom(e)),
                }
            }
        }
        deserializer.deserialize_str(IntVisitor)
    }
}

pub(crate) struct FromRaw<T>(pub T);
impl<T: Default> Default for FromRaw<T> {
    fn default() -> Self {
        FromRaw(T::default())
    }
}

impl<'de> Deserialize<'de> for FromRaw<DateTime<FixedOffset>> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        i64::deserialize(deserializer).map(|v| {
            FromRaw(DateTime::from_local(
                NaiveDateTime::from_timestamp_opt(v, 0).unwrap(),
                FixedOffset::east_opt(8 * 3600).unwrap(),
            ))
        })
    }
}

impl<'de> Deserialize<'de> for FromRaw<Option<Image>> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ImgVisitor;
        impl<'de> de::Visitor<'de> for ImgVisitor {
            type Value = FromRaw<Option<Image>>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("image url")
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(FromRaw(if v.is_empty() {
                    None
                } else {
                    Some(Image::Url(v))
                }))
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(FromRaw(if v.is_empty() {
                    None
                } else {
                    Some(Image::Url(v.to_owned()))
                }))
            }
        }
        deserializer.deserialize_string(ImgVisitor)
    }
}
impl<'de> Deserialize<'de> for FromRaw<Image> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        FromRaw::<Option<Image>>::deserialize(deserializer).map(|e| FromRaw(e.0.unwrap()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Storable)]
#[store(format = "yaml")]
pub struct RawDataInfo {
    pub fetch_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Storable)]
pub struct RawData {
    #[store(path(ext = "yaml"))]
    pub info: RawDataInfo,
    pub data: serde_json::Value,
}
