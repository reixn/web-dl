use crate::meta::Version;
use serde::{Deserialize, Serialize};
use web_dl_base::{media::Image, storable::Storable};

pub const VERSION: Version = Version { major: 0, minor: 1 };

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Inline {
    Text(String),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Break,
    Math {
        tex_code: String,
    },
    Code {
        code: String,
    },
    Image {
        alt_text: Option<String>,
        description: Option<Vec<Inline>>,
        src: Image,
    },
    Link {
        description: Option<Vec<Inline>>,
        target: String,
    },
    Note {
        content: Vec<Block>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Block {
    Header {
        level: usize,
        content: Vec<Inline>,
    },
    HorizontalRule,
    Plain(Vec<Inline>),
    Paragraph(Vec<Inline>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    Figure {
        alt_text: Option<String>,
        description: Option<Vec<Inline>>,
        src: Image,
    },
    SimpleTable {
        body: Vec<Vec<Blocks>>,
    },
    BlockQuote {
        content: Blocks,
    },
    UnorderedList {
        items: Vec<Blocks>,
    },
    OrderedList {
        items: Vec<Blocks>,
    },
}
pub type Blocks = Vec<Block>;

#[derive(Debug, Clone, Storable, Serialize, Deserialize)]
#[store(format = "ron")]
pub struct Document {
    pub version: Version,
    pub data: Vec<Block>,
}
