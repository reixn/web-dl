use crate::store::storable;
use serde::{de, Deserialize, Serialize};
use std::{
    fmt::{Debug, Display},
    path::PathBuf,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}
impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}.{}", self.major, self.minor))
    }
}
const VERSION_FILENAME: &str = "version.yaml";
impl Version {
    pub const fn is_compatible(&self, other: Version) -> bool {
        self.major == other.major
    }
    pub(crate) fn store(&self, path: &PathBuf) -> storable::Result<()> {
        storable::store_yaml(self, path, VERSION_FILENAME)
    }
    pub(crate) fn load(path: &PathBuf) -> storable::Result<Self> {
        storable::load_yaml(path, VERSION_FILENAME)
    }
}

#[derive(Debug)]
pub struct VersionMismatch {
    expect: Version,
    get: Version,
}
impl Display for VersionMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "version mismatch: {}, expected {}",
            self.get, self.expect
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MinVersion<const V: Version>(pub Version);
impl<const V: Version> Display for MinVersion<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}
impl<const V: Version> Serialize for MinVersion<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}
impl<'de, const V: Version> Deserialize<'de> for MinVersion<V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = Version::deserialize(deserializer)?;
        if V.is_compatible(v) {
            Ok(MinVersion(v))
        } else {
            Err(de::Error::custom(VersionMismatch { get: v, expect: V }))
        }
    }
}
