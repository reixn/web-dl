use crate::{
    driver::Driver,
    item::{
        column::{self, Column},
        user::{self, User},
        Answer, Article, Collection, Pin, Question,
    },
    progress::Reporter,
    store::BasicStoreItem,
    util::relative_path::{link_to_dest, prepare_dest_parent, DestPrepError, LinkError},
};
use std::{
    fmt::Display,
    fs, io,
    path::{Path, PathBuf},
};

use super::Manifest;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to prepare destination")]
    PrepareDest(
        #[source]
        #[from]
        DestPrepError,
    ),
    #[error("failed to create directory {}",path.display())]
    CreateDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to create symlink from {} to {}", dest.display(), store_path.display())]
    Link {
        store_path: PathBuf,
        dest: PathBuf,
        #[source]
        source: LinkError,
    },
}
fn create_dir(path: &PathBuf) -> Result<(), Error> {
    if path.exists() {
        return Ok(());
    }
    fs::create_dir(path).map_err(|e| Error::CreateDir {
        path: path.to_owned(),
        source: e,
    })
}

impl Driver {
    fn link<I: BasicStoreItem, N: Display, P: Reporter>(
        &self,
        prog: &P,
        name: N,
        id: I::Id<'_>,
        path: &Path,
    ) -> Result<(), Error> {
        let mut path = path.join(I::TYPE);
        path.push(name.to_string());
        if !path.exists() {
            let sp = self.store.item_path::<I>(id);
            link_to_dest(true, &sp, &path).map_err(|e| Error::Link {
                store_path: sp,
                dest: path.clone(),
                source: e,
            })?;
            prog.link_item(I::TYPE, id, path);
        }
        Ok(())
    }
    fn link_impl<P: Reporter>(
        &self,
        prog: &P,
        mut path: PathBuf,
        manifest: &Manifest,
    ) -> Result<(), Error> {
        match manifest {
            Manifest::Branch(b) => {
                for (d, m) in b {
                    let path = path.join(d);
                    create_dir(&path)?;
                    self.link_impl(prog, path, m)?;
                }
            }
            Manifest::Leaf(l) => {
                path.push("zhihu.com");
                let path = path;
                for id in l.answer.keys() {
                    self.link::<Answer, _, _>(prog, *id, *id, &path)?;
                }
                for id in l.article.keys() {
                    self.link::<Article, _, _>(prog, *id, *id, &path)?;
                }
                for id in l.collection.keys() {
                    self.link::<Collection, _, _>(prog, *id, *id, &path)?;
                }
                for id in l.column.keys() {
                    self.link::<Column, _, _>(prog, id, column::ColumnRef(id.0.as_str()), &path)?;
                }
                for id in l.pin.keys() {
                    self.link::<Pin, _, _>(prog, *id, *id, &path)?;
                }
                for id in l.question.keys() {
                    self.link::<Question, _, _>(prog, *id, *id, &path)?;
                }
                for (url, opt) in l.user.iter() {
                    self.link::<User, _, _>(prog, url, user::StoreId(opt.id, url.as_str()), &path)?;
                }
            }
        }
        Ok(())
    }
    pub fn link_manifest<P: Reporter, Pat: AsRef<Path>>(
        &self,
        prog: &P,
        manifest: &Manifest,
        dest: Pat,
    ) -> Result<(), Error> {
        self.link_impl(
            prog,
            prepare_dest_parent(dest.as_ref()).map_err(Error::from)?,
            manifest,
        )
    }
}
