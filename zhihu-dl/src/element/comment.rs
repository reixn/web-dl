use crate::{
    element::{author::Author, content::Content},
    meta::Version,
    raw_data::{FromRaw, RawData, StrU64},
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use web_dl_base::{id::HasId, media::HasImage, storable::Storable};

pub const VERSION: Version = Version { major: 1, minor: 0 };

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CommentId(pub u64);
impl Display for CommentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl<'de> Deserialize<'de> for FromRaw<CommentId> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        StrU64::deserialize(deserializer).map(|v| FromRaw(CommentId(v.0)))
    }
}

#[derive(Debug, Storable, Serialize, Deserialize)]
#[store(format = "yaml")]
pub struct CommentInfo {
    pub id: CommentId,
    pub parent_id: Option<CommentId>,
    pub author: Option<Author>,
    pub is_author: bool,
    pub child_count: u32,
    pub created_time: DateTime<FixedOffset>,
}

#[derive(Debug, Storable, HasImage, Serialize, Deserialize)]
pub struct Comment {
    #[store(path(ext = "yaml"))]
    pub version: Version,
    #[store(path(ext = "yaml"))]
    pub info: CommentInfo,
    #[has_image]
    pub content: Content,
    #[store(raw_data)]
    pub raw_data: Option<RawData>,
}
pub mod fetch;
pub use fetch::{parse_comment, Error as FetchError, RootType};

use super::content::HasContent;
impl HasId for Comment {
    const TYPE: &'static str = "comment";
    type Id<'a> = CommentId;
    fn id(&self) -> Self::Id<'_> {
        self.info.id
    }
}
impl HasContent for Comment {
    fn convert_html(&mut self) {
        self.content.convert_inline();
    }
    fn get_main_content(&self) -> Option<&'_ Content> {
        Some(&self.content)
    }
}

#[derive(Debug)]
pub struct CommentTree {
    pub node: Comment,
    pub child: Vec<CommentTree>,
}
impl CommentTree {
    pub fn from_comments(data: Vec<Comment>) -> Vec<CommentTree> {
        use std::collections::HashMap;
        let mut child: HashMap<Option<CommentId>, Vec<CommentId>> = HashMap::new();
        let mut map: HashMap<CommentId, Comment> = HashMap::new();
        for i in data {
            child.entry(i.info.parent_id).or_default().push(i.info.id);
            map.insert(i.info.id, i);
        }

        fn build_child(
            cid: Option<CommentId>,
            child: &mut HashMap<Option<CommentId>, Vec<CommentId>>,
            map: &mut HashMap<CommentId, Comment>,
        ) -> Vec<CommentTree> {
            child.remove(&cid).map_or(Vec::new(), |v| {
                v.into_iter().map(|i| build_tree(i, child, map)).collect()
            })
        }
        fn build_tree(
            id: CommentId,
            child: &mut HashMap<Option<CommentId>, Vec<CommentId>>,
            map: &mut HashMap<CommentId, Comment>,
        ) -> CommentTree {
            CommentTree {
                node: map.remove(&id).unwrap(),
                child: build_child(Some(id), child, map),
            }
        }
        build_child(None, &mut child, &mut map)
    }

    pub fn to_comments(tree: Vec<Self>) -> Vec<Comment> {
        fn write_vec(val: CommentTree, dest: &mut Vec<Comment>) {
            dest.push(val.node);
            for i in val.child {
                write_vec(i, dest);
            }
        }
        let mut ret = Vec::new();
        for i in tree {
            write_vec(i, &mut ret);
        }
        ret
    }
}
