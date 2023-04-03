use clap::Args;
use std::{
    fmt::{self, Display},
    io::Write,
};
use termcolor::{BufferedStandardStream, Color, ColorSpec, WriteColor};
use web_dl_base::id::{HasId, OwnedId};
use zhihu_dl::{
    driver,
    item::{
        answer::{Answer, AnswerId},
        article::{Article, ArticleId},
        collection::{Collection, CollectionId},
        column::{Column, ColumnRef},
        pin::{Pin, PinId},
        question::{Question, QuestionId},
        user::{self, User, UserId},
    },
};

pub struct Output {
    pub progress_bar: indicatif::MultiProgress,
    pub buffer: BufferedStandardStream,
}
#[allow(unused_must_use)]
impl Output {
    pub fn write_tagged(&mut self, color: Color, tag: &str, fmt: fmt::Arguments<'_>) {
        self.progress_bar.suspend(|| {
            self.buffer.set_color(ColorSpec::new().set_fg(Some(color)));
            self.buffer.write_fmt(format_args!("{:>13} ", tag));
            self.buffer.reset();
            self.buffer.write_fmt(fmt);
            self.buffer.flush();
        })
    }
    pub fn write_error(&mut self, error: anyhow::Error) {
        self.progress_bar.suspend(|| {
            self.buffer
                .set_color(ColorSpec::new().set_fg(Some(Color::Red)));
            self.buffer.write(b"error: ");
            self.buffer.reset();
            writeln!(&mut self.buffer, "{:?}", error);
            self.buffer.flush();
        })
    }
    pub fn write_warn(&mut self, fmt: fmt::Arguments<'_>) {
        self.progress_bar.suspend(|| {
            self.buffer
                .set_color(ColorSpec::new().set_fg(Some(Color::Yellow)));
            self.buffer.write(b"warning: ");
            self.buffer.reset();
            self.buffer.write_fmt(fmt);
            self.buffer.flush();
        })
    }
}

#[derive(Debug, Clone, Copy, Args)]
pub struct GetOpt {
    #[arg(long)]
    pub no_convert: bool,
    #[arg(long)]
    pub comments: bool,
}
impl Display for GetOpt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.comments {
            "with comments"
        } else {
            "no comments"
        })
    }
}
impl GetOpt {
    pub fn to_config(self) -> driver::GetConfig {
        driver::GetConfig {
            get_comments: self.comments,
            convert_html: !self.no_convert,
        }
    }
}

#[derive(Debug, Args)]
pub struct LinkOpt {
    #[arg(long)]
    pub link_absolute: bool,
    #[arg(value_hint = clap::ValueHint::AnyPath)]
    pub dest: String,
}

#[derive(Debug, Args)]
pub struct UserSpec {
    #[arg(long)]
    pub id: UserId,
    #[arg(long)]
    pub url_token: String,
}
impl OwnedId<User> for UserSpec {
    fn to_id(&self) -> <User as HasId>::Id<'_> {
        user::StoreId(self.id, self.url_token.as_str())
    }
}

#[derive(Debug, Args)]
pub struct NumId {
    #[arg(long)]
    pub id: u64,
}
impl OwnedId<Answer> for NumId {
    fn to_id(&self) -> <Answer as HasId>::Id<'_> {
        AnswerId(self.id)
    }
}
impl OwnedId<Article> for NumId {
    fn to_id(&self) -> <Article as HasId>::Id<'_> {
        ArticleId(self.id)
    }
}
impl OwnedId<Collection> for NumId {
    fn to_id(&self) -> <Collection as HasId>::Id<'_> {
        CollectionId(self.id)
    }
}
impl OwnedId<Question> for NumId {
    fn to_id(&self) -> <Question as HasId>::Id<'_> {
        QuestionId(self.id)
    }
}
impl OwnedId<Pin> for NumId {
    fn to_id(&self) -> <Pin as HasId>::Id<'_> {
        PinId(self.id)
    }
}

#[derive(Debug, Args)]
pub struct StrId {
    #[arg(long)]
    pub id: String,
}
impl OwnedId<Column> for StrId {
    fn to_id(&self) -> <Column as HasId>::Id<'_> {
        ColumnRef(self.id.as_str())
    }
}
