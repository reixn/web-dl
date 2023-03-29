use crate::bytes;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Hash, Serialize, Deserialize)]
#[serde(tag = "algo", content = "hash")]
pub enum HashDigest {
    #[serde(rename = "sha256")]
    Sha256(#[serde(with = "bytes")] [u8; 32]),
}
impl HashDigest {
    pub fn store_path(&self, parent: &PathBuf, extension: &str) -> PathBuf {
        let mut ret = parent.clone();
        ret.push(match self {
            Self::Sha256(h) => format!("sha256-{}", base16::encode_lower(h)),
        });
        ret.set_extension(extension);
        ret
    }
}
