use crate::raw_data::RawData;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OtherInfo {
    pub id: u64,
    #[serde(rename = "type")]
    pub item_type: String,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtherItem {
    pub info: Option<OtherInfo>,
    pub raw_data: RawData,
}
impl PartialOrd for OtherItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;
        match self.info.cmp(&other.info) {
            Ordering::Equal => {
                if self.raw_data == other.raw_data {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            a => Some(a),
        }
    }
}
impl OtherItem {
    pub(crate) fn warn(&self) {
        match &self.info {
            Some(it) => {
                log::warn!("skipped unrecognized object ({} {})", it.item_type, it.id)
            }
            None => log::warn!("skipped unrecognized object (unknown id,type)"),
        }
        log::trace!("ignored unknown object: {:#?}", self.raw_data);
    }
}
