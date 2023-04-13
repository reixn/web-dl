use super::spec::*;
use crate::{
    driver::{container::ContainerItem, ContainerError, Driver, ItemError},
    item::{
        any::Any,
        column::{self, ColumnRef},
        user, Answer, Article, Collection, Column, Comment, Fetchable, Item, ItemContainer, Pin,
        Question, User, VoidOpt,
    },
    progress::{ContainerJob, ItemContainerProg, ItemsProg, Reporter},
    store::{BasicStoreItem, StoreItem},
};
use std::fmt::Display;
use web_dl_base::{id::HasId, storable};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to process item {kind} {id}")]
    Item {
        id: String,
        kind: &'static str,
        #[source]
        source: ItemError,
    },
    #[error("failed to load item {kind} {id}")]
    Load {
        id: String,
        kind: &'static str,
        #[source]
        source: storable::Error,
    },
    #[error("failed to process {item_kind} ({option}) in {kind} {id}")]
    Container {
        item_kind: &'static str,
        id: String,
        kind: &'static str,
        option: &'static str,
        #[source]
        source: ContainerError,
    },
    #[error("failed to process sub container {option} of {kind} {id}")]
    SubContainer {
        id: String,
        kind: &'static str,
        option: &'static str,
        #[source]
        source: Box<Self>,
    },
}

trait OptDisplay {
    fn fmt_opt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}
trait ApplyChild<I>: Default + Eq + Copy + OptDisplay {
    async fn apply_child<P: Reporter>(
        &self,
        driver: &mut Driver,
        prog: &P,
        body: &I,
    ) -> Result<(), Error>;
}

impl OptDisplay for CommentChild {
    fn fmt_opt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.child.unwrap_or(false) {
            f.write_str("+child-comment")
        } else {
            Ok(())
        }
    }
}
impl ApplyChild<Comment> for CommentChild {
    async fn apply_child<P: Reporter>(
        &self,
        driver: &mut Driver,
        prog: &P,
        body: &Comment,
    ) -> Result<(), Error> {
        if body.has_item() {
            driver
                .apply_container::<Comment, Comment, VoidOpt, _>(prog, body.id())
                .await
                .map(|_| ())
        } else {
            Ok(())
        }
    }
}

impl OptDisplay for BasicChild {
    fn fmt_opt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.comment != Default::default() {
            f.write_str("+comment")
        } else {
            Ok(())
        }
    }
}
impl ApplyChild<Answer> for BasicChild {
    async fn apply_child<P: Reporter>(
        &self,
        driver: &mut Driver,
        prog: &P,
        body: &Answer,
    ) -> Result<(), Error> {
        driver.apply_sub_container(prog, body, self.comment).await
    }
}
impl ApplyChild<Any> for BasicChild {
    async fn apply_child<P: Reporter>(
        &self,
        driver: &mut Driver,
        prog: &P,
        body: &Any,
    ) -> Result<(), Error> {
        driver.apply_sub_container(prog, body, self.comment).await
    }
}
impl ApplyChild<Article> for BasicChild {
    async fn apply_child<P: Reporter>(
        &self,
        driver: &mut Driver,
        prog: &P,
        body: &Article,
    ) -> Result<(), Error> {
        driver.apply_sub_container(prog, body, self.comment).await
    }
}
impl ApplyChild<Pin> for BasicChild {
    async fn apply_child<P: Reporter>(
        &self,
        driver: &mut Driver,
        prog: &P,
        body: &Pin,
    ) -> Result<(), Error> {
        driver.apply_sub_container(prog, body, self.comment).await
    }
}

impl OptDisplay for CollectionChild {
    fn fmt_opt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}",
            if self.comment.is_some() {
                "+comment"
            } else {
                ""
            },
            if self.item.is_some() { "+item" } else { "" }
        )
    }
}
impl ApplyChild<Collection> for CollectionChild {
    async fn apply_child<P: Reporter>(
        &self,
        driver: &mut Driver,
        prog: &P,
        body: &Collection,
    ) -> Result<(), Error> {
        driver
            .apply_sub_container::<Collection, VoidOpt, Comment, _, _>(prog, body, self.comment)
            .await?;
        driver
            .apply_sub_container::<Collection, VoidOpt, Any, _, _>(prog, body, self.item)
            .await
    }
}

impl OptDisplay for ColumnChild {
    fn fmt_opt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}",
            if self.pinned.is_some() {
                "+pinned-item"
            } else {
                ""
            },
            if self.regular.is_some() {
                "+regular-item"
            } else {
                ""
            }
        )
    }
}
impl ApplyChild<Column> for ColumnChild {
    async fn apply_child<P: Reporter>(
        &self,
        driver: &mut Driver,
        prog: &P,
        body: &Column,
    ) -> Result<(), Error> {
        driver
            .apply_sub_container::<_, column::Pinned, Any, _, _>(prog, body, self.pinned)
            .await?;
        driver
            .apply_sub_container::<_, column::Regular, Any, _, _>(prog, body, self.regular)
            .await
    }
}

impl OptDisplay for QuestionChild {
    fn fmt_opt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}",
            if self.comment.is_some() {
                "+comment"
            } else {
                ""
            },
            if self.answer.is_some() { "+answer" } else { "" }
        )
    }
}
impl ApplyChild<Question> for QuestionChild {
    async fn apply_child<P: Reporter>(
        &self,
        driver: &mut Driver,
        prog: &P,
        body: &Question,
    ) -> Result<(), Error> {
        driver
            .apply_sub_container::<Question, VoidOpt, Answer, _, _>(prog, body, self.answer)
            .await?;
        driver.apply_sub_container(prog, body, self.comment).await
    }
}

impl OptDisplay for UserChild {
    fn fmt_opt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.answer.is_some() {
            f.write_str("+answer ")?;
        }
        if self.article.is_some() {
            f.write_str("+article ")?;
        }
        if let Some(col) = self.collection {
            if col.created.is_some() {
                f.write_str("+created-collection ")?;
            }
            if col.liked.is_some() {
                f.write_str("+liked-collection ")?;
            }
        }
        if self.column.is_some() {
            f.write_str("+column ")?;
        }
        if self.pin.is_some() {
            f.write_str("+pin ")?;
        }
        Ok(())
    }
}
impl ApplyChild<User> for UserChild {
    async fn apply_child<P: Reporter>(
        &self,
        driver: &mut Driver,
        prog: &P,
        body: &User,
    ) -> Result<(), Error> {
        driver
            .apply_sub_container::<_, VoidOpt, Answer, _, _>(prog, body, self.answer)
            .await?;
        driver
            .apply_sub_container::<_, VoidOpt, Article, _, _>(prog, body, self.article)
            .await?;
        driver
            .apply_sub_container::<_, user::Created, Collection, _, _>(
                prog,
                body,
                self.collection.and_then(|v| v.created),
            )
            .await?;
        driver
            .apply_sub_container::<_, user::Liked, Collection, _, _>(
                prog,
                body,
                self.collection.and_then(|v| v.liked),
            )
            .await?;
        driver
            .apply_sub_container::<_, VoidOpt, Column, _, _>(prog, body, self.column)
            .await?;
        driver
            .apply_sub_container::<_, VoidOpt, Pin, _, _>(prog, body, self.pin)
            .await
    }
}

struct ShowOpt<I>(I);
impl<I: OptDisplay> Display for ShowOpt<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt_opt(f)
    }
}

impl Driver {
    async fn apply_container<IC: ItemContainer<O, I>, I: Item, O, P: Reporter>(
        &mut self,
        prog: &P,
        id: IC::Id<'_>,
    ) -> Result<Vec<ContainerItem<I>>, Error> {
        self.update_container::<IC, I, O, _>(prog, id)
            .await
            .map_err(|e| Error::Container {
                item_kind: I::TYPE,
                id: id.to_string(),
                kind: IC::TYPE,
                option: IC::OPTION_NAME,
                source: e,
            })
    }
    async fn apply_sub_container<IC, O, I, Opt, P>(
        &mut self,
        prog: &P,
        body: &IC,
        child: Option<Opt>,
    ) -> Result<(), Error>
    where
        IC: ItemContainer<O, I>,
        I: Item,
        P: Reporter,
        Opt: ApplyChild<I>,
    {
        let child = match child {
            Some(c) => c,
            None => return Ok(()),
        };
        if !body.has_item() {
            return Ok(());
        }
        let roots = self.apply_container::<IC, I, O, _>(prog, body.id()).await?;
        if child != Opt::default() {
            let prog = prog.start_item_container::<I, O, IC, _, _>(
                "Processing",
                "",
                body.id(),
                Some(ShowOpt(child)),
            );
            {
                let mut p = prog.start_items(roots.len() as u64);
                for i in &roots {
                    let _p_i = ItemsProg::start_item(&mut p, I::TYPE, i.value.id());
                    child
                        .apply_child(self, &prog, &i.value)
                        .await
                        .map_err(|e| Error::SubContainer {
                            id: body.id().to_string(),
                            kind: IC::TYPE,
                            option: IC::OPTION_NAME,
                            source: Box::new(e),
                        })?;
                }
            }
            prog.finish("Processed", Some(roots.len()), body.id());
        }
        Ok(())
    }
    async fn apply_basic<I, P, Opt>(
        &mut self,
        prog: &P,
        id: I::Id<'_>,
        child: Option<Opt>,
    ) -> Result<(), Error>
    where
        P: Reporter,
        I: Fetchable + Item + BasicStoreItem,
        Opt: ApplyChild<I> + Default + Copy,
    {
        if !<I as StoreItem>::in_store(id, &self.store).on_server {
            return Ok(());
        }
        let v = self
            .get_item::<I, _>(prog, id)
            .await
            .map_err(|e| Error::Item {
                id: id.to_string(),
                kind: I::TYPE,
                source: e,
            })?;
        if let Some(child) = child {
            if child != Opt::default() {
                let v = match v {
                    Some(v) => v,
                    None => self
                        .store
                        .get_object::<I>(id, Default::default())
                        .map_err(|e| Error::Load {
                            id: id.to_string(),
                            kind: I::TYPE,
                            source: e,
                        })?,
                };
                child.apply_child(self, prog, &v).await?;
            }
        }
        Ok(())
    }

    pub async fn apply_manifest_leaf<P: Reporter>(
        &mut self,
        prog: &P,
        leaf: &ManifestLeaf,
    ) -> Result<(), Error> {
        for (id, opt) in &leaf.answer {
            self.apply_basic::<Answer, _, _>(prog, *id, opt.child)
                .await?;
        }
        for (id, opt) in &leaf.article {
            self.apply_basic::<Article, _, _>(prog, *id, opt.child)
                .await?;
        }
        for (id, opt) in &leaf.collection {
            self.apply_basic::<Collection, _, _>(prog, *id, opt.child)
                .await?;
        }
        for (id, opt) in &leaf.column {
            self.apply_basic::<Column, _, _>(prog, ColumnRef(id.0.as_str()), opt.child)
                .await?;
        }
        for (id, opt) in &leaf.pin {
            self.apply_basic::<Pin, _, _>(prog, *id, opt.child).await?;
        }
        for (id, opt) in &leaf.question {
            self.apply_basic::<Question, _, _>(prog, *id, opt.child)
                .await?;
        }
        for (url_token, opt) in &leaf.user {
            self.apply_basic(prog, user::StoreId(opt.id, url_token.as_str()), opt.child)
                .await?;
        }
        Ok(())
    }
}
