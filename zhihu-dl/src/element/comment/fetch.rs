use super::{Comment, CommentId, CommentInfo};
use crate::{
    element::{author::Author, content::Content},
    progress::{self, CommentProg, CommentsProg},
    raw_data::{self, FromRaw, RawData, RawDataInfo},
    request::Client,
};
use chrono::{DateTime, FixedOffset, Utc};
use serde::Deserialize;
use std::{
    collections::{BTreeSet, HashSet, LinkedList},
    error,
    fmt::Display,
};

#[derive(Debug, Clone, Copy)]
pub enum RootType {
    Article,
    Answer,
    Collection,
    Pin,
    Question,
}
impl Display for RootType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Article => "article",
            Self::Answer => "answer",
            Self::Collection => "collection",
            Self::Pin => "pin",
            Self::Question => "question",
        })
    }
}

#[derive(Debug)]
enum ErrorComment {
    RootComment {
        root_type: RootType,
        root_id: String,
    },
    ChildComment(CommentId),
    Comment(CommentId),
}
#[derive(Debug)]
enum ErrorSource {
    Http(reqwest::Error),
    Json(serde_json::Error),
}
#[derive(Debug)]
pub struct Error {
    comment: ErrorComment,
    source: ErrorSource,
}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let src = match self.source {
            ErrorSource::Http(_) => "Http request",
            ErrorSource::Json(_) => "Json parsing",
        };
        match &self.comment {
            ErrorComment::RootComment { root_type, root_id } => f.write_fmt(format_args!(
                "{} failed while fetching root comment in {} {}",
                src, root_type, root_id
            )),
            ErrorComment::ChildComment(c) => {
                f.write_fmt(format_args!("{} failed while fetching comment {}", src, c))
            }
            ErrorComment::Comment(c) => f.write_fmt(format_args!(
                "{} failed while fetching child comment of {}",
                src, c
            )),
        }
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.source {
            ErrorSource::Http(h) => Some(h),
            ErrorSource::Json(j) => Some(j),
        }
    }
}

pub fn parse_comment(value: RawData) -> serde_json::Result<Comment> {
    log::trace!("parsing raw data: {:#?}", value);
    #[derive(Deserialize)]
    struct Reply {
        id: FromRaw<CommentId>,
        reply_comment_id: FromRaw<CommentId>,
        author: FromRaw<Option<Author>>,
        is_author: bool,
        child_comment_count: u32,
        created_time: FromRaw<DateTime<FixedOffset>>,
        #[serde(default)]
        content: FromRaw<Content>,
    }
    serde_json::from_value(value.data.clone()).map(|dat: Reply| Comment {
        version: super::VERSION,
        info: CommentInfo {
            id: dat.id.0,
            parent_id: if dat.reply_comment_id.0 .0 == 0 {
                None
            } else {
                Some(dat.reply_comment_id.0)
            },
            author: dat.author.0,
            is_author: dat.is_author,
            child_count: dat.child_comment_count,
            created_time: dat.created_time.0,
        },
        content: dat.content.0,
        raw_data: Some(value),
    })
}
fn parse_comments(values: LinkedList<RawData>) -> Result<LinkedList<Comment>, serde_json::Error> {
    values.into_iter().map(parse_comment).try_collect()
}

async fn fetch_root<I: Display, P: progress::FetchProg>(
    client: &Client,
    prog: P,
    root_type: RootType,
    id: I,
) -> Result<LinkedList<Comment>, Error> {
    log::debug!("fetching root comment for {} {}", root_type, &id);
    parse_comments(
        client
            .get_paged::<{ raw_data::Container::None }, _, _>(
                prog,
                format!(
                    "https://www.zhihu.com/api/v4/comment_v5/{}/{}/root_comment",
                    match root_type {
                        RootType::Answer => "answers",
                        RootType::Article => "articles",
                        RootType::Collection => "collections",
                        RootType::Pin => "pins",
                        RootType::Question => "questions",
                    },
                    id
                ),
            )
            .await
            .map_err(|e| Error {
                comment: ErrorComment::RootComment {
                    root_type,
                    root_id: id.to_string(),
                },
                source: ErrorSource::Http(e),
            })?,
    )
    .map_err(|e| Error {
        comment: ErrorComment::RootComment {
            root_type,
            root_id: id.to_string(),
        },
        source: ErrorSource::Json(e),
    })
}
async fn fetch_child<P: progress::FetchProg>(
    client: &Client,
    prog: P,
    id: CommentId,
) -> Result<LinkedList<Comment>, Error> {
    log::debug!("fetching child comments of {}", id);
    parse_comments(
        client
            .get_paged::<{ raw_data::Container::None }, _, _>(
                prog,
                format!(
                    "https://www.zhihu.com/api/v4/comment_v5/comment/{}/child_comment",
                    id
                ),
            )
            .await
            .map_err(|e| Error {
                comment: ErrorComment::ChildComment(id),
                source: ErrorSource::Http(e),
            })?,
    )
    .map_err(|e| Error {
        comment: ErrorComment::ChildComment(id),
        source: ErrorSource::Json(e),
    })
}
async fn fetch_comment(client: &Client, id: CommentId) -> Result<Comment, Error> {
    let http_err = |e| Error {
        comment: ErrorComment::Comment(id),
        source: ErrorSource::Http(e),
    };
    log::debug!("fetching comment {}", id);
    let data = client
        .http_client
        .get(format!(
            "https://www.zhihu.com/api/v4/comment_v5/comment/{}",
            id
        ))
        .send()
        .await
        .map_err(http_err)?
        .json()
        .await
        .map_err(http_err)?;
    parse_comment(RawData {
        data,
        info: RawDataInfo {
            fetch_time: Utc::now(),
            container: raw_data::Container::None,
        },
    })
    .map_err(|e| Error {
        comment: ErrorComment::Comment(id),
        source: ErrorSource::Json(e),
    })
}

impl Comment {
    pub async fn get<I: Display, P: progress::CommentTreeProg>(
        client: &Client,
        prog: P,
        root_type: RootType,
        id: I,
    ) -> Result<Vec<Comment>, Error> {
        let mut ret = fetch_root(client, prog.start_fetch_root(), root_type, id).await?;
        {
            let mut child_prog = prog.start_comments(ret.len() as u64);
            let mut child = LinkedList::new();
            for i in ret.iter() {
                let prog = child_prog.start_comment(&i.info.id);
                if i.info.child_count != 0 {
                    child.append(&mut fetch_child(client, prog.start_child(), i.info.id).await?);
                }
            }
            ret.append(&mut child);
        }
        {
            let _missing_prog = prog.start_fetch_missing();
            let mut exist = HashSet::with_capacity(ret.len());
            let mut missing = BTreeSet::new();
            ret.iter().for_each(|i| {
                exist.insert(i.info.id);
            });
            for i in ret.iter() {
                match i.info.parent_id {
                    Some(p) => {
                        if !exist.contains(&p) {
                            missing.insert(p);
                        }
                    }
                    None => {}
                }
            }
            while let Some(m) = missing.pop_first() {
                let c = fetch_comment(client, m).await?;
                exist.insert(m);
                match c.info.parent_id {
                    Some(p) => {
                        if !exist.contains(&p) {
                            missing.insert(p);
                        }
                    }
                    None => {}
                }
                ret.push_back(c);
            }
        }
        {
            let mut child_prog = prog.start_comments(ret.len() as u64);
            let mut count = 0;
            for i in ret.iter_mut() {
                let urls = i.content.image_urls();
                if !urls.is_empty() {
                    let prog = child_prog.start_comment(&i.info.id);
                    i.content
                        .fetch_images(client, &mut prog.start_images(urls.len() as u64), urls)
                        .await;
                    count += 1;
                } else {
                    child_prog.skip_comment()
                }
                if count == 40 {
                    use progress::Progress;
                    child_prog.sleep(client.request_interval).await;
                    count = 0;
                }
            }
        }
        Ok(ret.into_iter().collect())
    }
}
