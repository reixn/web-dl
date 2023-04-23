use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Hash, Serialize, Deserialize)]
#[serde(tag = "algo", content = "hash")]
pub enum HashDigest {
    #[serde(rename = "sha256")]
    Sha256(#[serde(with = "hex::serde")] [u8; 32]),
}
impl HashDigest {
    pub fn store_path(&self, parent: &Path, extension: &str) -> PathBuf {
        let mut ret = parent.to_path_buf();
        ret.push(match self {
            Self::Sha256(h) => format!("sha256-{}", base16::encode_lower(h)),
        });
        ret.set_extension(extension);
        ret
    }
}
