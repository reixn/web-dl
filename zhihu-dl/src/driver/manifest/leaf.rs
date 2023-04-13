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
impl Error {
    pub(super) fn sub_container<IC: ItemContainer<O, I>, O, I: Item>(
        container: &IC,
        source: Self,
    ) -> Self {
        Self::SubContainer {
            id: container.id().to_string(),
            kind: I::TYPE,
            option: IC::OPTION_NAME,
            source: Box::new(source),
        }
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
    async fn apply_comment_container<IC: ItemContainer<VoidOpt, Comment>, P: Reporter>(
        &mut self,
        prog: &P,
        body: &IC,
        child: Option<CommentChild>,
    ) -> Result<(), Error> {
        let child = match child {
            Some(c) => c,
            None => return Ok(()),
        };
        if !body.has_item() {
            return Ok(());
        }
        let roots = self
            .apply_container::<IC, Comment, VoidOpt, _>(prog, body.id())
            .await?;
        if let Some(true) = child.child {
            let prog = prog.start_item_container::<Comment, VoidOpt, IC, _, _>(
                "Processing",
                "",
                body.id(),
                Some("+child-comment"),
            );
            {
                let mut p = prog.start_items(roots.len() as u64);
                for i in &roots {
                    if i.value.has_item() {
                        let _p_i = ItemsProg::start_item(&mut p, Comment::TYPE, i.value.id());
                        self.apply_container::<Comment, Comment, VoidOpt, _>(&prog, i.value.id())
                            .await
                            .map_err(|e| Error::sub_container(body, e))?;
                    } else {
                        p.skip_item()
                    }
                }
            }
            prog.finish("Processed", Some(roots.len()), body.id());
        }
        Ok(())
    }
    async fn apply_basic_child<I: ItemContainer<VoidOpt, Comment>, P: Reporter>(
        &mut self,
        prog: &P,
        body: &I,
        child: BasicChild,
    ) -> Result<(), Error> {
        self.apply_comment_container::<I, _>(prog, body, child.comment)
            .await
    }
    async fn apply_basic_container<IC, O, I, P>(
        &mut self,
        prog: &P,
        body: &IC,
        child: Option<BasicChild>,
    ) -> Result<(), Error>
    where
        I: Item + ItemContainer<VoidOpt, Comment>,
        IC: ItemContainer<O, I>,
        P: Reporter,
    {
        let child = match child {
            Some(c) => c,
            None => return Ok(()),
        };
        if !body.has_item() {
            return Ok(());
        }
        let roots = self.apply_container::<IC, I, O, _>(prog, body.id()).await?;
        if child != Default::default() {
            let prog = prog.start_item_container::<Comment, VoidOpt, I, _, _>(
                "Processing",
                "",
                body.id(),
                Some("+comment"),
            );
            {
                let mut p = prog.start_items(roots.len() as u64);
                for i in &roots {
                    let _p_i = ItemsProg::start_item(&mut p, I::TYPE, i.value.id());
                    self.apply_basic_child::<I, _>(&prog, &i.value, child)
                        .await
                        .map_err(|e| Error::sub_container(body, e))?;
                }
            }
            prog.finish("Processed", Some(roots.len()), body.id());
        }
        Ok(())
    }
    async fn apply_collection_child<P: Reporter>(
        &mut self,
        prog: &P,
        body: &Collection,
        child: CollectionChild,
    ) -> Result<(), Error> {
        self.apply_comment_container::<Collection, _>(prog, body, child.comment)
            .await?;
        self.apply_basic_container::<Collection, VoidOpt, Any, _>(prog, body, child.item)
            .await
    }
    async fn apply_collection_container<IC: ItemContainer<O, Collection>, O, P: Reporter>(
        &mut self,
        prog: &P,
        body: &IC,
        child: Option<CollectionChild>,
    ) -> Result<(), Error> {
        let child = match child {
            Some(v) => v,
            None => return Ok(()),
        };
        if !body.has_item() {
            return Ok(());
        }
        let roots = self
            .apply_container::<IC, Collection, O, _>(prog, body.id())
            .await?;
        if child != Default::default() {
            let prog = prog.start_item_container::<Collection, O, IC, _, _>(
                "Processing",
                "",
                body.id(),
                Some(format_args!(
                    "{} {}",
                    if child.comment.is_some() {
                        "+comment"
                    } else {
                        ""
                    },
                    if child.item.is_some() { "+item" } else { "" }
                )),
            );
            {
                let mut p = prog.start_items(roots.len() as u64);
                for i in &roots {
                    let _p_i = ItemsProg::start_item(&mut p, Collection::TYPE, i.value.id());
                    self.apply_collection_child(&prog, &i.value, child)
                        .await
                        .map_err(|e| Error::sub_container(body, e))?;
                }
            }
            prog.finish("Processed", Some(roots.len()), body.id());
        }
        Ok(())
    }
    async fn apply_column_child<P: Reporter>(
        &mut self,
        prog: &P,
        body: &Column,
        child: ColumnChild,
    ) -> Result<(), Error> {
        self.apply_basic_container::<_, column::Pinned, Any, _>(prog, body, child.pinned)
            .await?;
        self.apply_basic_container::<_, column::Regular, Any, _>(prog, body, child.regular)
            .await
    }
    async fn apply_column_container<IC: ItemContainer<O, Column>, O, P: Reporter>(
        &mut self,
        prog: &P,
        body: &IC,
        child: Option<ColumnChild>,
    ) -> Result<(), Error> {
        let child = match child {
            Some(v) => v,
            None => return Ok(()),
        };
        if !body.has_item() {
            return Ok(());
        }
        let roots = self
            .apply_container::<IC, Column, O, _>(prog, body.id())
            .await?;
        if child != Default::default() {
            let prog = prog.start_item_container::<Column, O, IC, _, _>(
                "Processing",
                "",
                body.id(),
                Some(format_args!(
                    "{} {}",
                    if child.pinned.is_some() {
                        "+pinned-item"
                    } else {
                        ""
                    },
                    if child.regular.is_some() {
                        "+regular-item"
                    } else {
                        ""
                    }
                )),
            );
            {
                let mut p = prog.start_items(roots.len() as u64);
                for i in &roots {
                    let _p_i = ItemsProg::start_item(&mut p, Column::TYPE, i.value.id());
                    self.apply_column_child(&prog, &i.value, child)
                        .await
                        .map_err(|e| Error::sub_container(body, e))?;
                }
            }
            prog.finish("Processed", Some(roots.len()), body.id());
        }
        Ok(())
    }
    async fn apply_question_child<P: Reporter>(
        &mut self,
        prog: &P,
        body: &Question,
        child: QuestionChild,
    ) -> Result<(), Error> {
        self.apply_basic_container::<Question, VoidOpt, Answer, _>(prog, body, child.answer)
            .await?;
        self.apply_comment_container(prog, body, child.comment)
            .await
    }
    async fn apply_item<I: Fetchable + Item + BasicStoreItem, O: Default + Eq, P: Reporter>(
        &mut self,
        prog: &P,
        option: O,
        id: I::Id<'_>,
    ) -> Result<Option<I>, Error> {
        if !<I as StoreItem>::in_store(id, &self.store).on_server {
            return Ok(None);
        }
        let v = self
            .get_item::<I, _>(prog, id)
            .await
            .map_err(|e| Error::Item {
                id: id.to_string(),
                kind: I::TYPE,
                source: e,
            })?;
        Ok(if option == O::default() {
            None
        } else {
            Some(match v {
                Some(v) => v,
                None => self
                    .store
                    .get_object::<I>(id, Default::default())
                    .map_err(|e| Error::Load {
                        id: id.to_string(),
                        kind: I::TYPE,
                        source: e,
                    })?,
            })
        })
    }

    pub async fn apply_manifest_leaf<P: Reporter>(
        &mut self,
        prog: &P,
        leaf: &ManifestLeaf,
    ) -> Result<(), Error> {
        for (id, opt) in &leaf.answer {
            if let Some(ans) = self.apply_item::<Answer, _, _>(prog, *opt, *id).await? {
                if let Some(child) = opt.child {
                    self.apply_basic_child(prog, &ans, child).await?;
                }
            }
        }
        for (id, opt) in &leaf.article {
            if let Some(art) = self.apply_item::<Article, _, _>(prog, *opt, *id).await? {
                if let Some(child) = opt.child {
                    self.apply_basic_child(prog, &art, child).await?;
                }
            }
        }
        for (id, opt) in &leaf.collection {
            if let Some(col) = self.apply_item::<Collection, _, _>(prog, *opt, *id).await? {
                if let Some(child) = opt.child {
                    self.apply_collection_child(prog, &col, child).await?;
                }
            }
        }
        for (id, opt) in &leaf.column {
            if let Some(col) = self
                .apply_item::<Column, _, _>(prog, *opt, ColumnRef(id.0.as_str()))
                .await?
            {
                if let Some(child) = opt.child {
                    self.apply_column_child(prog, &col, child).await?;
                }
            }
        }
        for (id, opt) in &leaf.pin {
            if let Some(p) = self.apply_item::<Pin, _, _>(prog, *opt, *id).await? {
                if let Some(child) = opt.child {
                    self.apply_basic_child(prog, &p, child).await?;
                }
            }
        }
        for (id, opt) in &leaf.question {
            if let Some(q) = self.apply_item::<Question, _, _>(prog, *opt, *id).await? {
                if let Some(child) = opt.child {
                    self.apply_question_child(prog, &q, child).await?;
                }
            }
        }
        for (url_token, opt) in &leaf.user {
            let store_id = user::StoreId(opt.id, url_token.as_str());
            if let Some(u) = self
                .apply_item::<User, _, _>(prog, opt.child, store_id)
                .await?
            {
                if let Some(child) = opt.child {
                    self.apply_basic_container::<_, VoidOpt, Answer, _>(prog, &u, child.answer)
                        .await?;
                    self.apply_basic_container::<_, VoidOpt, Article, _>(prog, &u, child.article)
                        .await?;
                    self.apply_collection_container::<_, user::Created, _>(
                        prog,
                        &u,
                        child.collection.and_then(|v| v.created),
                    )
                    .await?;
                    self.apply_collection_container::<_, user::Liked, _>(
                        prog,
                        &u,
                        child.collection.and_then(|v| v.liked),
                    )
                    .await?;
                    self.apply_column_container::<_, VoidOpt, _>(prog, &u, child.column)
                        .await?;
                    self.apply_basic_container::<_, VoidOpt, Pin, _>(prog, &u, child.pin)
                        .await?;
                }
            }
        }
        Ok(())
    }
}
