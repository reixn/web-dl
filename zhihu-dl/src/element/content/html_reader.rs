use super::document::*;
use anyhow::Context;
use ego_tree::NodeRef;
use scraper::{node::Element, Node};
use std::collections::HashMap;
use web_dl_base::media;

fn proc_external_link(url_str: &str) -> anyhow::Result<String> {
    let v = url::Url::parse(url_str).context("failed to parse url")?;
    if v.domain() != Some("link.zhihu.com") {
        return Ok(url_str.to_string());
    }
    for (k, v) in v.query_pairs() {
        if k == "target" {
            log::debug!("converted external url `{}` to `{}`", url_str, v);
            return Ok(v.to_string());
        }
    }
    anyhow::bail!("no target url found");
}

fn proc_link_desc(root: NodeRef<'_, Node>) -> Option<Vec<Inline>> {
    let mut ret = String::new();
    for desc in root.descendants().skip(1) {
        let node = desc.value();
        if let Some(t) = node.as_text() {
            ret.extend(t.chars());
        } else if let Some(e) = node.as_element() {
            if e.name() != "span" {
                log::warn!("unknown element in link description {:?}", e);
            }
        } else if !node.is_comment() {
            log::warn!("unknown node in link description {:?}", node);
        }
    }

    if ret.is_empty() {
        None
    } else {
        Some(Vec::from([Inline::Text(ret)]))
    }
}
fn proc_link(e: &Element, root: NodeRef<'_, Node>) -> anyhow::Result<Inline> {
    Ok(Inline::Link {
        description: proc_link_desc(root),
        target: match e.attr("href") {
            Some(r) => match proc_external_link(r) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("failed to convert external url `{}` : {:?}", r, e);
                    r.to_string()
                }
            },
            None => anyhow::bail!("href not found"),
        },
    })
}

fn proc_code(root: NodeRef<'_, Node>) -> String {
    let mut ret = String::new();
    for i in root.descendants() {
        if let Some(t) = i.value().as_text() {
            ret.extend(t.chars());
        }
    }
    ret
}
fn proc_inline_img<'a>(
    e: &Element,
    src: &str,
    image_map: &HashMap<&'a str, &'a media::ImageRef>,
) -> anyhow::Result<Inline> {
    let u = url::Url::parse(src).context("failed to parse url")?;
    let alt = e.attr("alt");
    if u.domain() == Some("www.zhihu.com") && u.path() == "/equation" {
        if let Some(t) = alt {
            return Ok(Inline::Math {
                tex_code: t.to_string(),
            });
        }
        for (k, v) in u.query_pairs() {
            if k == "tex" {
                return Ok(Inline::Math {
                    tex_code: v.to_string(),
                });
            }
        }
        anyhow::bail!("tex code not found");
    }
    return Ok(Inline::Image {
        alt_text: alt.map(str::to_string),
        description: None,
        src: match image_map.get(src) {
            Some(r) => media::Image::Ref((*r).to_owned()),
            None => media::Image::Url(src.to_owned()),
        },
    });
}

fn proc_note(e: &Element) -> anyhow::Result<Inline> {
    if e.attr("data-draft-type") != Some("reference") {
        anyhow::bail!("unknown supscript");
    }
    if let Some(v) = e.attr("data-url") {
        if !v.is_empty() {
            return Ok(Inline::Note {
                content: Vec::from([Block::Plain(Vec::from([Inline::Link {
                    description: None,
                    target: match proc_external_link(v) {
                        Ok(l) => l,
                        Err(e) => {
                            log::warn!("failed to convert link `{}`: {:?}", v, e);
                            v.to_string()
                        }
                    },
                }]))]),
            });
        }
    }
    if let Some(v) = e.attr("data-text") {
        if !v.is_empty() {
            return Ok(Inline::Note {
                content: Vec::from([Block::Paragraph(Vec::from([Inline::Text(v.to_string())]))]),
            });
        }
    }
    anyhow::bail!("reference target not found");
}

fn proc_inline_elem<'a>(
    child: NodeRef<'_, Node>,
    e: &Element,
    image_map: &HashMap<&'a str, &'a media::ImageRef>,
) -> anyhow::Result<Inline> {
    match e.name() {
        "a" => proc_link(e, child).context("failed toprocess link"),
        "b" => Ok(Inline::Strong(proc_inlines(child, image_map))),
        "br" => Ok(Inline::Break),
        "em" | "i" => Ok(Inline::Emphasis(proc_inlines(child, image_map))),
        "code" => Ok(Inline::Code {
            code: proc_code(child),
        }),
        "sup" => proc_note(e).context("failed to process note"),
        "img" => {
            let src = e
                .attr("data-original")
                .or(e.attr("src"))
                .context("can't found image source")?;
            proc_inline_img(e, src, image_map).or_else(|err| {
                log::warn!("process image failed: {:?}", err);
                Ok(Inline::Image {
                    alt_text: None,
                    description: None,
                    src: media::Image::Url(src.to_string()),
                })
            })
        }
        n => {
            anyhow::bail!("unknown element {}", n);
        }
    }
}
fn proc_inlines<'a>(
    root: NodeRef<'_, Node>,
    image_map: &HashMap<&'a str, &'a media::ImageRef>,
) -> Vec<Inline> {
    let mut ret = Vec::new();
    for child in root.children() {
        let value = child.value();
        if let Some(t) = value.as_text() {
            ret.push(Inline::Text(t.to_string()));
            continue;
        } else if let Some(e) = value.as_element() {
            match proc_inline_elem(child, e, image_map) {
                Ok(v) => ret.push(v),
                Err(err) => {
                    log::warn!("failed to process element {:#?}: {:?}", e, err);
                }
            }
        } else if !value.is_comment() {
            log::warn!("unexpected node {:#?}", value);
        }
    }
    ret
}

fn find_elem<'a>(root: NodeRef<'a, Node>, name: &str) -> Option<(&'a Element, NodeRef<'a, Node>)> {
    for i in root.children() {
        if let Some(e) = i.value().as_element() {
            if e.name() == name {
                return Some((e, i));
            }
        }
    }
    None
}
fn proc_code_block(root: NodeRef<'_, Node>) -> anyhow::Result<Block> {
    let (_, pre_node) = find_elem(root, "pre").context("element pre not found")?;
    let (code, code_node) = find_elem(pre_node, "code").context("element code not found")?;
    Ok(Block::CodeBlock {
        language: code
            .attr("class")
            .and_then(|v| v.strip_prefix("language-"))
            .map(str::to_string),
        code: proc_code(code_node),
    })
}
fn proc_figure<'a>(
    root: NodeRef<'_, Node>,
    image_map: &HashMap<&'a str, &'a media::ImageRef>,
) -> anyhow::Result<Block> {
    let (img, _) = find_elem(root, "img").context("can't find img tag")?;
    let src = img
        .attr("data-original")
        .or(img.attr("data-actualsrc"))
        .or(img.attr("src"))
        .context("can't find image src")?;
    Ok(Block::Figure {
        alt_text: img.attr("alt").map(str::to_string),
        description: find_elem(root, "figcaption")
            .map(|(_, cap_ref)| proc_inlines(cap_ref, image_map)),
        src: match image_map.get(src) {
            Some(v) => media::Image::Ref((*v).to_owned()),
            None => media::Image::Url(src.to_string()),
        },
    })
}
fn proc_list<'a>(
    root: NodeRef<'_, Node>,
    image_map: &HashMap<&'a str, &'a media::ImageRef>,
) -> Vec<Vec<Block>> {
    let mut ret = Vec::new();
    for child in root.children() {
        let value = child.value();
        if let Some(e) = value.as_element() {
            match e.name() {
                "li" => ret.push(Vec::from([Block::Paragraph(proc_inlines(
                    child, image_map,
                ))])),
                "ul" => ret.push(Vec::from([Block::UnorderedList {
                    items: proc_list(child, image_map),
                }])),
                "ol" => ret.push(Vec::from([Block::OrderedList {
                    items: proc_list(child, image_map),
                }])),
                _ => {
                    log::warn!("ignored unknown list element {:#?}", e)
                }
            }
        } else if !value.is_comment() {
            log::warn!("ignored unknown list item {:#?}", value);
        }
    }
    ret
}
fn proc_table<'a>(
    root: NodeRef<'_, Node>,
    image_map: &HashMap<&'a str, &'a media::ImageRef>,
) -> Block {
    fn check_elem(node: &Node, name: &str, context: &str) -> bool {
        if let Some(e) = node.as_element() {
            if e.name() == name {
                return true;
            }
            log::warn!("unexpected table {} element: {:#?}", context, e);
            return false;
        }
        if !node.is_comment() {
            log::warn!("unexpected table {} node: {:#?}", context, node);
        }
        false
    }
    let mut ret = Vec::new();
    for i in root.children() {
        if check_elem(i.value(), "tbody", "body") {
            for b in i.children() {
                if check_elem(b.value(), "tr", "row") {
                    let mut row = Vec::new();
                    for j in b.children() {
                        if check_elem(j.value(), "td", "cell") {
                            row.push(Vec::from([Block::Plain(proc_inlines(j, image_map))]));
                        }
                    }
                    ret.push(row);
                }
            }
        }
    }
    Block::SimpleTable { body: ret }
}

fn try_proc_block_elem<'a>(
    child: NodeRef<'_, Node>,
    e: &Element,
    image_map: &HashMap<&'a str, &'a media::ImageRef>,
) -> anyhow::Result<Block> {
    match e.name() {
        // link card
        "a" => Ok(Block::Paragraph(Vec::from([
            proc_link(e, child).context("failed to process link card")?
        ]))),
        "div" => {
            if let Some("highlight") = e.attr("class") {
                proc_code_block(child).context("failed to process code block")
            } else {
                anyhow::bail!("unknown div {:#?}", e);
            }
        }
        "figure" => proc_figure(child, image_map),
        "blockquote" => Ok(Block::BlockQuote {
            content: Vec::from([Block::Paragraph(proc_inlines(child, image_map))]),
        }),
        "ul" => Ok(Block::UnorderedList {
            items: proc_list(child, image_map),
        }),
        "ol" => Ok(Block::OrderedList {
            items: proc_list(child, image_map),
        }),
        "p" => Ok(Block::Paragraph(proc_inlines(child, image_map))),
        _ => {
            anyhow::bail!("unknown element");
        }
    }
}
fn proc_block<'a>(
    root: NodeRef<'_, Node>,
    image_map: &HashMap<&'a str, &'a media::ImageRef>,
) -> Vec<Block> {
    let mut ret = Vec::new();
    for child in root.children() {
        let value = child.value();
        if let Some(e) = value.as_element() {
            ret.push(match e.name() {
                "h1" => Block::Header {
                    level: 1,
                    content: proc_inlines(child, image_map),
                },
                "h2" => Block::Header {
                    level: 2,
                    content: proc_inlines(child, image_map),
                },
                "h3" => Block::Header {
                    level: 3,
                    content: proc_inlines(child, image_map),
                },
                "h4" => Block::Header {
                    level: 4,
                    content: proc_inlines(child, image_map),
                },
                "h5" => Block::Header {
                    level: 5,
                    content: proc_inlines(child, image_map),
                },
                "h6" => Block::Header {
                    level: 6,
                    content: proc_inlines(child, image_map),
                },
                "hr" => Block::HorizontalRule,
                "ul" => Block::UnorderedList {
                    items: proc_list(child, image_map),
                },
                "ol" => Block::OrderedList {
                    items: proc_list(child, image_map),
                },
                "p" => Block::Paragraph(proc_inlines(child, image_map)),
                "table" => proc_table(child, image_map),
                _ => match try_proc_block_elem(child, e, image_map) {
                    Ok(v) => v,
                    Err(err) => {
                        log::warn!("failed to process element {:#?}: {:?}", e, err);
                        continue;
                    }
                },
            })
        } else if !value.is_comment() {
            log::warn!("unexpected node {:#?}", value);
        }
    }
    ret
}

pub fn from_raw_html<'a>(
    input: &str,
    image_map: &HashMap<&'a str, &'a media::ImageRef>,
) -> Document {
    Document {
        version: VERSION,
        data: proc_block(
            *scraper::Html::parse_fragment(input).root_element(),
            image_map,
        ),
    }
}

pub fn from_raw_html_inline<'a>(
    input: &str,
    image_map: &HashMap<&'a str, &'a media::ImageRef>,
) -> Document {
    Document {
        version: VERSION,
        data: Vec::from([Block::Plain(proc_inlines(
            *scraper::Html::parse_fragment(input).root_element(),
            image_map,
        ))]),
    }
}
