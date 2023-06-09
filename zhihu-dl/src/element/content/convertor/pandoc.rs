use super::super::document::*;
use std::path::Path;
use web_dl_base::media;

fn inline_to_text(inline: &Inline, dest: &mut String) {
    match inline {
        Inline::Text(t) => dest.push_str(t.as_str()),
        Inline::Break => dest.push('\n'),
        Inline::Code { code } => dest.push_str(code.as_str()),
        Inline::Emphasis(e) => e.iter().for_each(|i| inline_to_text(i, dest)),
        Inline::Strong(s) => s.iter().for_each(|i| inline_to_text(i, dest)),
        Inline::Math { tex_code } => dest.push_str(tex_code.as_str()),
        Inline::Note { .. } => (),
        Inline::Image { alt_text, .. } => alt_text.iter().for_each(|t| dest.push_str(t.as_str())),
        Inline::Link { description, .. } => description
            .iter()
            .for_each(|v| v.iter().for_each(|i| inline_to_text(i, dest))),
    }
}
fn text(input: &str, dest: &mut Vec<pandoc_ast::Inline>) {
    for i in input
        .chars()
        .collect::<Vec<char>>()
        .group_by(|a, b| a.is_whitespace() == b.is_whitespace())
    {
        if i.iter().any(|c| *c == '\n' || *c == '\t') {
            dest.push(pandoc_ast::Inline::SoftBreak);
        } else if i.iter().any(|c| char::is_whitespace(*c)) {
            dest.push(pandoc_ast::Inline::Space);
        } else {
            dest.push(pandoc_ast::Inline::Str(i.iter().collect()))
        }
    }
}

fn proc_image(
    alt_text: &Option<String>,
    description: &Option<Vec<Inline>>,
    src: &media::Image,
    images_store: &Path,
) -> pandoc_ast::Inline {
    pandoc_ast::Inline::Image(
        pandoc_ast::Attr::default(),
        alt_text.as_ref().map_or(Vec::default(), |alt| {
            let mut dest = Vec::new();
            text(alt.as_str(), &mut dest);
            dest
        }),
        (
            match src {
                media::Image::Url(s) => s.to_owned(),
                media::Image::Ref(r) => r
                    .hash
                    .store_path(images_store, r.extension.as_str())
                    .to_string_lossy()
                    .into_owned(),
            },
            description.as_ref().map_or(String::default(), |d| {
                let mut s = String::new();
                d.iter().for_each(|i| inline_to_text(i, &mut s));
                s
            }),
        ),
    )
}
fn proc_inline(inline: &Inline, images_store: &Path, dest: &mut Vec<pandoc_ast::Inline>) {
    match inline {
        Inline::Break => dest.push(pandoc_ast::Inline::LineBreak),
        Inline::Code { code } => dest.push(pandoc_ast::Inline::Code(
            pandoc_ast::Attr::default(),
            code.to_owned(),
        )),
        Inline::Emphasis(e) => dest.push(pandoc_ast::Inline::Emph(proc_inlines(e, images_store))),
        Inline::Image {
            alt_text,
            description,
            src,
        } => dest.push(proc_image(alt_text, description, src, images_store)),
        Inline::Link {
            description,
            target,
        } => dest.push(pandoc_ast::Inline::Link(
            pandoc_ast::Attr::default(),
            description
                .as_ref()
                .map_or(Vec::new(), |d| proc_inlines(d, images_store)),
            (target.to_owned(), String::default()),
        )),
        Inline::Math { tex_code } => dest.push(pandoc_ast::Inline::Math(
            pandoc_ast::MathType::InlineMath,
            tex_code.to_owned(),
        )),
        Inline::Note { content } => {
            dest.push(pandoc_ast::Inline::Note(proc_blocks(content, images_store)))
        }
        Inline::Strong(s) => dest.push(pandoc_ast::Inline::Strong(proc_inlines(s, images_store))),
        Inline::Text(t) => text(t, dest),
    }
}
fn proc_inlines(inlines: &Vec<Inline>, images_store: &Path) -> Vec<pandoc_ast::Inline> {
    let mut ret = Vec::new();
    inlines
        .into_iter()
        .for_each(|i| proc_inline(i, images_store, &mut ret));
    ret
}

fn proc_block(block: &Block, images_store: &Path) -> pandoc_ast::Block {
    match block {
        Block::BlockQuote { content } => {
            pandoc_ast::Block::BlockQuote(proc_blocks(content, images_store))
        }
        Block::CodeBlock { language, code } => pandoc_ast::Block::CodeBlock(
            language
                .as_ref()
                .map_or_else(pandoc_ast::Attr::default, |l| {
                    (String::default(), Vec::from([l.to_owned()]), Vec::default())
                }),
            code.to_owned(),
        ),
        Block::Figure {
            alt_text,
            description,
            src,
        } => pandoc_ast::Block::Para(Vec::from([proc_image(
            alt_text,
            description,
            src,
            images_store,
        )])),
        Block::Header { level, content } => pandoc_ast::Block::Header(
            *level as i64,
            pandoc_ast::Attr::default(),
            proc_inlines(content, images_store),
        ),
        Block::HorizontalRule => pandoc_ast::Block::HorizontalRule,
        Block::OrderedList { items } => pandoc_ast::Block::OrderedList(
            (
                1,
                pandoc_ast::ListNumberStyle::DefaultStyle,
                pandoc_ast::ListNumberDelim::DefaultDelim,
            ),
            items
                .into_iter()
                .map(|bs| proc_blocks(bs, images_store))
                .collect(),
        ),
        Block::Paragraph(l) => pandoc_ast::Block::Para(proc_inlines(l, images_store)),
        Block::Plain(l) => pandoc_ast::Block::Plain(proc_inlines(l, images_store)),
        Block::SimpleTable { body } => pandoc_ast::Block::Table(
            pandoc_ast::Attr::default(),
            pandoc_ast::Caption::default(),
            {
                let sp = (
                    pandoc_ast::Alignment::AlignDefault,
                    pandoc_ast::ColWidth::ColWidthDefault,
                );
                let n = body.iter().map(|r| r.len()).max().unwrap_or(0);
                vec![sp; n]
            },
            pandoc_ast::TableHead::default(),
            Vec::from([(
                pandoc_ast::Attr::default(),
                0,
                Vec::new(),
                body.into_iter()
                    .map(|r| {
                        (
                            pandoc_ast::Attr::default(),
                            r.into_iter()
                                .map(|c| {
                                    (
                                        pandoc_ast::Attr::default(),
                                        pandoc_ast::Alignment::AlignDefault,
                                        1,
                                        1,
                                        proc_blocks(c, images_store),
                                    )
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            )]),
            pandoc_ast::TableFoot::default(),
        ),
        Block::UnorderedList { items } => pandoc_ast::Block::BulletList(
            items
                .into_iter()
                .map(|i| proc_blocks(i, images_store))
                .collect(),
        ),
    }
}
fn proc_blocks(blocks: &Vec<Block>, images_store: &Path) -> Vec<pandoc_ast::Block> {
    blocks
        .into_iter()
        .map(|b| proc_block(b, images_store))
        .collect()
}

pub fn to_pandoc_ast(document: &Document, images_store: &Path) -> pandoc_ast::Pandoc {
    pandoc_ast::Pandoc {
        meta: pandoc_ast::Map::default(),
        blocks: proc_blocks(&document.data, images_store),
        pandoc_api_version: Vec::from([1, 22]),
    }
}

pub fn to_pandoc_json(document: &Document, images_store: &Path) -> String {
    to_pandoc_ast(document, images_store).to_json()
}

pub struct Pandoc;
pub struct PandocConfig<'a> {
    pub format: &'a str,
}

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("failed to prepare destination path")]
    DestPrep(
        #[from]
        #[source]
        crate::util::relative_path::DestPrepError,
    ),
    #[error("failed to spawn pandoc {command:?}")]
    CreateProcess {
        command: std::process::Command,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write input pipe")]
    WriteInput(#[source] std::io::Error),
    #[error("failed to get child process exit code")]
    WaitProcess(#[source] std::io::Error),
    #[error("pandoc exits with {0}")]
    Pandoc(std::process::ExitStatus),
}

impl super::super::Convertor for Pandoc {
    type Config<'a> = PandocConfig<'a> where Self:'a;
    type Err = ConvertError;
    fn convert<S: AsRef<std::path::Path>, P: AsRef<std::path::Path>>(
        image_store: S,
        document: &crate::element::content::document::Document,
        config: &Self::Config<'_>,
        dest: P,
    ) -> Result<(), Self::Err> {
        use crate::util::relative_path::{prepare_dest, relative_path_to};
        use std::{io::Write, process};
        let canon_dest = prepare_dest(dest.as_ref()).map_err(ConvertError::from)?;
        let image_store = relative_path_to(image_store.as_ref(), canon_dest).unwrap_or_else(|| {
            log::warn!(
                "failed to make image store `{}` relative to `{}`",
                image_store.as_ref().display(),
                dest.as_ref().display()
            );
            image_store.as_ref().to_path_buf()
        });
        let mut ch = {
            let mut cmd = process::Command::new("pandoc");
            cmd.args(["-f", "json", "-t", config.format, "-o"])
                .arg(dest.as_ref())
                .stdin(process::Stdio::piped());
            cmd.spawn().map_err(|e| ConvertError::CreateProcess {
                command: cmd,
                source: e,
            })?
        };
        let ch_stdin = ch.stdin.as_mut().unwrap();
        ch_stdin
            .write(to_pandoc_json(document, image_store.as_path()).as_bytes())
            .map_err(ConvertError::WriteInput)?;
        let r = ch.wait().map_err(ConvertError::WaitProcess)?;
        if r.success() {
            Ok(())
        } else {
            Err(ConvertError::Pandoc(r))
        }
    }
}
