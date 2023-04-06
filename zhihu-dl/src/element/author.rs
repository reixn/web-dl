use crate::{
    meta::{MinVersion, Version},
    raw_data::FromRaw,
};
use serde::{de, Deserialize, Serialize};
use std::{
    fmt::{Debug, Display},
    str::FromStr,
};
use web_dl_base::utils::bytes;

pub const VERSION: Version = Version { major: 1, minor: 1 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserType {
    People,
    Organization,
}
impl<'de> Deserialize<'de> for FromRaw<UserType> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum Reply {
            #[serde(rename = "people")]
            People,
            #[serde(rename = "organization")]
            Org,
        }
        Reply::deserialize(deserializer).map(|v| match v {
            Reply::People => FromRaw(UserType::People),
            Reply::Org => FromRaw(UserType::Organization),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UserId(#[serde(with = "bytes")] pub [u8; 16]);
impl Debug for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        bytes::fmt(&self.0, f)
    }
}
impl Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        bytes::fmt(&self.0, f)
    }
}
impl FromStr for UserId {
    type Err = bytes::DecodeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        bytes::decode_bytes(s).map(UserId)
    }
}

impl<'de> Deserialize<'de> for FromRaw<Option<UserId>> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IdVisitor;
        impl<'de> de::Visitor<'de> for IdVisitor {
            type Value = FromRaw<Option<UserId>>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("user_id")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v == "0" {
                    return Ok(FromRaw(None));
                }
                match bytes::decode_bytes(v) {
                    Ok(d) => Ok(FromRaw(Some(UserId(d)))),
                    Err(e) => Err(de::Error::custom(e)),
                }
            }
        }
        deserializer.deserialize_str(IdVisitor)
    }
}
impl<'de> Deserialize<'de> for FromRaw<UserId> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        FromRaw::<Option<UserId>>::deserialize(deserializer).map(|d| FromRaw(d.0.unwrap()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub version: MinVersion<VERSION>,
    pub id: UserId,
    pub name: String,
    pub url_token: Option<String>,
    pub user_type: UserType,
    pub headline: String,
}
impl<'de> Deserialize<'de> for FromRaw<Option<Author>> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Reply {
            id: FromRaw<Option<UserId>>,
            name: String,
            user_type: FromRaw<UserType>,
            url_token: Option<String>,
            headline: String,
        }
        Reply::deserialize(deserializer).map(|dat| {
            FromRaw(match dat.id.0 {
                Some(i) => Some(Author {
                    version: MinVersion(VERSION),
                    id: i,
                    name: dat.name,
                    url_token: dat.url_token,
                    user_type: dat.user_type.0,
                    headline: dat.headline,
                }),
                None => None,
            })
        })
    }
}
impl<'de> Deserialize<'de> for FromRaw<Author> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        FromRaw::<Option<Author>>::deserialize(deserializer)
            .map(|v| FromRaw(v.0.expect("unknown author")))
    }
}
