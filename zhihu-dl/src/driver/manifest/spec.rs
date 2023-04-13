use crate::item::{AnswerId, ArticleId, CollectionId, ColumnId, PinId, QuestionId, UserId};
use serde::{Deserialize, Serialize};
use std::collections::{btree_map, BTreeMap};

mod option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<T: Serialize, S: Serializer>(
        value: &Option<T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(v) => v.serialize(serializer),
            None => ().serialize(serializer),
        }
    }
    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<T>, D::Error> {
        T::deserialize(deserializer).map(Option::Some)
    }
}

pub trait Mergable {
    fn merge(&mut self, other: Self);
}
impl Mergable for bool {
    fn merge(&mut self, other: Self) {
        *self |= other;
    }
}
impl<I: Mergable> Mergable for Option<I> {
    fn merge(&mut self, other: Self) {
        if let Some(v2) = other {
            match self {
                Some(v) => v.merge(v2),

                None => *self = Some(v2),
            }
        }
    }
}
impl<K: Ord, V: Mergable> Mergable for BTreeMap<K, V> {
    fn merge(&mut self, other: Self) {
        for (k, v) in other.into_iter() {
            match self.entry(k) {
                btree_map::Entry::Occupied(mut o) => {
                    o.get_mut().merge(v);
                }
                btree_map::Entry::Vacant(o) => {
                    o.insert(v);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ItemOption<C: Serialize + serde::de::DeserializeOwned> {
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub child: Option<C>,
}
impl<C: Mergable + Serialize + serde::de::DeserializeOwned> Mergable for ItemOption<C> {
    fn merge(&mut self, other: Self) {
        self.child.merge(other.child);
    }
}

macro_rules! child_opt {
  ($n:ident, $(($i:ident: $t:ty)),+) => {
      #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
      pub struct $n {
          $(#[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
          pub $i: Option<$t>,)+
      }
      impl Mergable for $n {
          fn merge(&mut self, other: Self) {
              $(self.$i.merge(other.$i);)+
          }
      }
  };
}

child_opt!(BasicChild, (comment: CommentChild));
pub type AnswerChild = BasicChild;
pub type ArticleChild = BasicChild;
pub type AnyChild = BasicChild;
child_opt!(CollectionChild, (item: AnyChild), (comment: CommentChild));
child_opt!(ColumnChild, (regular: AnyChild), (pinned: AnyChild));
child_opt!(CommentChild, (child: bool));
pub type PinChild = BasicChild;
child_opt!(
    QuestionChild,
    (comment: CommentChild),
    (answer: AnswerChild)
);
child_opt!(
    UserCollection,
    (created: CollectionChild),
    (liked: CollectionChild)
);
child_opt!(
    UserChild,
    (answer: AnswerChild),
    (article: ArticleChild),
    (collection: UserCollection),
    (column: ColumnChild),
    (pin: PinChild),
    (question: QuestionChild)
);
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UserOption {
    pub id: UserId,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub container: Option<bool>,
    #[serde(default, with = "option", skip_serializing_if = "Option::is_none")]
    pub child: Option<UserChild>,
}
impl Mergable for UserOption {
    fn merge(&mut self, other: Self) {
        self.container.merge(other.container);
        self.child.merge(other.child);
    }
}

macro_rules! leaf {
    ($(($n:ident: $k:ty, $v:ty)),+) => {
        #[derive(Debug, Clone, Default, Serialize, Deserialize)]
        pub struct ManifestLeaf {
            $(#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
              pub $n: BTreeMap<$k, $v>,)+
        }
        impl Mergable for ManifestLeaf {
            fn merge(&mut self, other: Self) {
                $(self.$n.merge(other.$n);)+
            }
        }
    };
}
leaf! {
    (answer: AnswerId, ItemOption<AnswerChild>),
    (article: ArticleId, ItemOption<ArticleChild>),
    (collection: CollectionId, ItemOption<CollectionChild>),
    (column: ColumnId, ItemOption<ColumnChild>),
    (pin: PinId, ItemOption<PinChild>),
    (question: QuestionId, ItemOption<QuestionChild>),
    (user: String, UserOption)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Manifest {
    Leaf(ManifestLeaf),
    Branch(BTreeMap<String, Manifest>),
}
impl Manifest {
    pub(super) fn merged_leaf(&self) -> ManifestLeaf {
        fn merge_leaf(manifest: &Manifest, dest: &mut ManifestLeaf) {
            match manifest {
                Manifest::Leaf(l) => dest.merge(l.to_owned()),
                Manifest::Branch(b) => b.values().for_each(|v| merge_leaf(v, dest)),
            }
        }
        let mut ret = ManifestLeaf::default();
        merge_leaf(self, &mut ret);
        ret
    }
}
