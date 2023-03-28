use crate::store::storable;
use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::{fs, path::PathBuf};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawData {
    pub fetch_time: DateTime<Utc>,
    pub data: serde_json::Value,
}
const RAW_DATA_FILE: &str = "raw_data.json";
impl RawData {
    pub fn load(path: &PathBuf) -> storable::Result<Self> {
        use storable::*;
        serde_json::from_reader(
            fs::File::open(push_path(path, RAW_DATA_FILE))
                .map_err(|e| Error::load_error(RAW_DATA_FILE, ErrorSource::Io(e)))?,
        )
        .map_err(|e| Error::load_error(RAW_DATA_FILE, ErrorSource::Json(e)))
    }
    pub(crate) fn load_if(
        path: &PathBuf,
        load_opt: storable::LoadOpt,
    ) -> storable::Result<Option<Self>> {
        if load_opt.load_raw {
            Self::load(path).map(|v| Some(v))
        } else {
            Ok(None)
        }
    }
    pub fn store(&self, path: &PathBuf) -> storable::Result<()> {
        use storable::*;
        serde_json::to_writer_pretty(
            fs::File::create(push_path(path, RAW_DATA_FILE))
                .map_err(|e| Error::store_error(RAW_DATA_FILE, ErrorSource::Io(e)))?,
            self,
        )
        .map_err(|e| Error::store_error(RAW_DATA_FILE, ErrorSource::Json(e)))
    }
    pub(crate) fn store_option(data: &Option<RawData>, path: &PathBuf) -> storable::Result<()> {
        match data {
            Some(d) => d.store(path),
            None => Ok(()),
        }
    }
}
