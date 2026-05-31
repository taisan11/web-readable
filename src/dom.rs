use std::collections::HashMap;

use html5ever::ParseOpts;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use crate::model::Metadata;

#[derive(Debug, Clone, Default)]
pub(crate) struct NodeMetrics {
    pub(crate) text_len: usize,
    pub(crate) link_text_len: usize,
    pub(crate) paragraph_count: usize,
    pub(crate) heading_count: usize,
    pub(crate) list_item_count: usize,
    pub(crate) media_count: usize,
    pub(crate) punctuation_count: usize,
}

impl NodeMetrics {
    pub(crate) fn link_density(&self) -> f64 {
        if self.text_len == 0 {
            return 0.0;
        }
        self.link_text_len as f64 / self.text_len as f64
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ElementContext {
    pub(crate) key: usize,
    pub(crate) handle: Handle,
    pub(crate) parent: Option<usize>,
    pub(crate) grandparent: Option<usize>,
    pub(crate) tag: String,
}

pub(crate) fn parse_html(html: &str) -> RcDom {
    html5ever::parse_document(RcDom::default(), ParseOpts::default()).one(html)
}

pub(crate) fn node_key(handle: &Handle) -> usize {
    std::rc::Rc::as_ptr(handle) as usize
}

pub(crate) fn child_handles(handle: &Handle) -> Vec<Handle> {
    handle.children.borrow().iter().cloned().collect()
}

pub(crate) fn element_name(handle: &Handle) -> Option<String> {
    if let NodeData::Element { name, .. } = &handle.data {
        return Some(name.local.to_string().to_ascii_lowercase());
    }
    None
}

pub(crate) fn attr_value(handle: &Handle, attr_name: &str) -> Option<String> {
    if let NodeData::Element { attrs, .. } = &handle.data {
        for attr in attrs.borrow().iter() {
            if attr.name.local.as_ref().eq_ignore_ascii_case(attr_name) {
                return Some(attr.value.to_string());
            }
        }
    }
    None
}

pub(crate) fn collect_element_contexts(root: &Handle) -> Vec<ElementContext> {
    fn walk(
        node: &Handle,
        parent: Option<usize>,
        grandparent: Option<usize>,
        out: &mut Vec<ElementContext>,
    ) {
        if let Some(tag) = element_name(node) {
            let key = node_key(node);
            out.push(ElementContext {
                key,
                handle: node.clone(),
                parent,
                grandparent,
                tag,
            });

            for child in child_handles(node) {
                walk(&child, Some(key), parent, out);
            }
            return;
        }

        for child in child_handles(node) {
            walk(&child, parent, grandparent, out);
        }
    }

    let mut out = Vec::new();
    walk(root, None, None, &mut out);
    out
}

pub(crate) fn find_first_element(root: &Handle, tag_name: &str) -> Option<Handle> {
    collect_element_contexts(root)
        .into_iter()
        .find(|ctx| ctx.tag == tag_name)
        .map(|ctx| ctx.handle)
}

pub(crate) fn build_metrics_map(root: &Handle) -> HashMap<usize, NodeMetrics> {
    fn walk(
        node: &Handle,
        in_link: bool,
        metrics: &mut HashMap<usize, NodeMetrics>,
    ) -> NodeMetrics {
        match &node.data {
            NodeData::Text { contents } => {
                let normalized = normalize_whitespace(contents.borrow().as_ref());
                if normalized.is_empty() {
                    return NodeMetrics::default();
                }
                let len = normalized.chars().count();
                let punctuation_count = count_punctuation(&normalized);
                NodeMetrics {
                    text_len: len,
                    link_text_len: if in_link { len } else { 0 },
                    punctuation_count,
                    ..NodeMetrics::default()
                }
            }
            NodeData::Element { .. } => {
                let tag = element_name(node).unwrap_or_default();
                if should_skip_for_metrics(node, &tag) {
                    let key = node_key(node);
                    metrics.insert(key, NodeMetrics::default());
                    return NodeMetrics::default();
                }

                let next_in_link = in_link || tag == "a";
                let mut out = NodeMetrics::default();
                for child in child_handles(node) {
                    let child_metrics = walk(&child, next_in_link, metrics);
                    out.text_len += child_metrics.text_len;
                    out.link_text_len += child_metrics.link_text_len;
                    out.paragraph_count += child_metrics.paragraph_count;
                    out.heading_count += child_metrics.heading_count;
                    out.list_item_count += child_metrics.list_item_count;
                    out.media_count += child_metrics.media_count;
                    out.punctuation_count += child_metrics.punctuation_count;
                }

                match tag.as_str() {
                    "p" | "pre" | "blockquote" => out.paragraph_count += 1,
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => out.heading_count += 1,
                    "li" => out.list_item_count += 1,
                    "img" | "video" | "picture" | "iframe" | "object" | "embed" | "figure" => {
                        out.media_count += 1
                    }
                    _ => {}
                }

                let key = node_key(node);
                metrics.insert(key, out.clone());
                out
            }
            _ => {
                let mut out = NodeMetrics::default();
                for child in child_handles(node) {
                    let child_metrics = walk(&child, in_link, metrics);
                    out.text_len += child_metrics.text_len;
                    out.link_text_len += child_metrics.link_text_len;
                    out.paragraph_count += child_metrics.paragraph_count;
                    out.heading_count += child_metrics.heading_count;
                    out.list_item_count += child_metrics.list_item_count;
                    out.media_count += child_metrics.media_count;
                    out.punctuation_count += child_metrics.punctuation_count;
                }
                out
            }
        }
    }

    let mut map = HashMap::new();
    let _ = walk(root, false, &mut map);
    map
}

pub(crate) fn extract_metadata(root: &Handle) -> Metadata {
    let mut metadata = Metadata::default();

    for ctx in collect_element_contexts(root) {
        match ctx.tag.as_str() {
            "html" => {
                if metadata.lang.is_none() {
                    metadata.lang = attr_value(&ctx.handle, "lang")
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty());
                }
            }
            "title" => {
                if metadata.title.is_none() {
                    let title = collect_text(&ctx.handle);
                    if !title.is_empty() {
                        metadata.title = Some(title);
                    }
                }
            }
            "meta" => {
                let content = attr_value(&ctx.handle, "content")
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty());
                if content.is_none() {
                    continue;
                }
                let content = content.unwrap_or_default();
                let name = attr_value(&ctx.handle, "name")
                    .or_else(|| attr_value(&ctx.handle, "property"))
                    .or_else(|| attr_value(&ctx.handle, "itemprop"))
                    .map(|v| v.to_ascii_lowercase())
                    .unwrap_or_default();

                match name.as_str() {
                    "author" | "article:author" | "parsely-author" => {
                        metadata.byline.get_or_insert(content);
                    }
                    "description" | "og:description" | "twitter:description" => {
                        metadata.excerpt.get_or_insert(content);
                    }
                    "og:site_name" => {
                        metadata.site_name.get_or_insert(content);
                    }
                    "article:published_time" | "pubdate" | "publishdate" | "date" => {
                        metadata.published_time.get_or_insert(content);
                    }
                    "og:title" | "twitter:title" => {
                        metadata.title.get_or_insert(content);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    metadata
}

pub(crate) fn collect_text_from_html(html: &str) -> String {
    let dom = parse_html(html);
    collect_text(&dom.document)
}

pub(crate) fn collect_text(node: &Handle) -> String {
    fn walk(node: &Handle, out: &mut String) {
        match &node.data {
            NodeData::Text { contents } => {
                let normalized = normalize_whitespace(contents.borrow().as_ref());
                if normalized.is_empty() {
                    return;
                }
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(&normalized);
            }
            NodeData::Element { .. } => {
                let tag = element_name(node).unwrap_or_default();
                if should_drop_entire_node(node, &tag, true) {
                    return;
                }
                for child in child_handles(node) {
                    walk(&child, out);
                }
            }
            _ => {
                for child in child_handles(node) {
                    walk(&child, out);
                }
            }
        }
    }

    let mut out = String::new();
    walk(node, &mut out);
    normalize_whitespace(out.trim())
}

pub(crate) fn serialize_clean_fragment(
    nodes: &[Handle],
    metrics: &HashMap<usize, NodeMetrics>,
    include_images: bool,
) -> String {
    fn walk(
        node: &Handle,
        out: &mut String,
        metrics: &HashMap<usize, NodeMetrics>,
        include_images: bool,
        in_preformatted: bool,
    ) {
        match &node.data {
            NodeData::Text { contents } => {
                let text = if in_preformatted {
                    contents.borrow().to_string()
                } else {
                    normalize_whitespace(contents.borrow().as_ref())
                };
                if !text.is_empty() {
                    out.push_str(&escape_html_text(&text));
                }
            }
            NodeData::Element { attrs, .. } => {
                let tag = element_name(node).unwrap_or_default();
                if should_drop_entire_node(node, &tag, !include_images) {
                    return;
                }

                if is_probably_noise(node, &tag, metrics) {
                    return;
                }

                out.push('<');
                out.push_str(&tag);
                for attr in attrs.borrow().iter() {
                    let name = attr.name.local.as_ref().to_ascii_lowercase();
                    if !is_allowed_attr(&tag, &name) {
                        continue;
                    }
                    let value = attr.value.to_string();
                    if !is_safe_attr_value(&name, &value) {
                        continue;
                    }
                    out.push(' ');
                    out.push_str(&name);
                    out.push_str("=\"");
                    out.push_str(&escape_html_attr(&value));
                    out.push('"');
                }
                out.push('>');

                if !is_void_tag(&tag) {
                    let next_in_preformatted =
                        in_preformatted || matches!(tag.as_str(), "pre" | "code" | "textarea");
                    for child in child_handles(node) {
                        walk(&child, out, metrics, include_images, next_in_preformatted);
                    }
                    out.push_str("</");
                    out.push_str(&tag);
                    out.push('>');
                }
            }
            _ => {
                for child in child_handles(node) {
                    walk(&child, out, metrics, include_images, in_preformatted);
                }
            }
        }
    }

    let mut out = String::new();
    for node in nodes {
        walk(node, &mut out, metrics, include_images, false);
    }
    out
}

fn should_skip_for_metrics(node: &Handle, tag: &str) -> bool {
    matches!(tag, "script" | "style" | "noscript" | "template") || is_hidden_element(node)
}

fn should_drop_entire_node(node: &Handle, tag: &str, drop_images: bool) -> bool {
    if is_hidden_element(node) {
        return true;
    }
    if drop_images && matches!(tag, "img" | "picture" | "source") {
        return true;
    }
    matches!(
        tag,
        "script"
            | "style"
            | "noscript"
            | "template"
            | "nav"
            | "aside"
            | "footer"
            | "form"
            | "button"
            | "input"
            | "textarea"
            | "select"
            | "option"
    )
}

fn is_hidden_element(node: &Handle) -> bool {
    let Some(style) = attr_value(node, "style") else {
        return attr_value(node, "hidden").is_some()
            || attr_value(node, "aria-hidden")
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
    };
    let compact = style.to_ascii_lowercase().replace(' ', "");
    compact.contains("display:none")
        || compact.contains("visibility:hidden")
        || compact.contains("opacity:0")
        || attr_value(node, "hidden").is_some()
        || attr_value(node, "aria-hidden").is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
}

fn is_probably_noise(node: &Handle, tag: &str, metrics: &HashMap<usize, NodeMetrics>) -> bool {
    if matches!(tag, "div" | "section" | "article" | "aside" | "ul" | "ol") {
        let key = node_key(node);
        if let Some(m) = metrics.get(&key) {
            if m.text_len < 40 && m.link_density() > 0.75 {
                return true;
            }
        }
    }

    let class_id = class_id_text(node);
    if class_id.contains("corner")
        || class_id.contains("author")
        || class_id.contains("bookmark")
        || class_id.contains("social")
        || class_id.contains("download")
        || class_id.contains("affiliate")
        || class_id.contains("amazon")
        || class_id.contains("promo")
        || class_id.contains("share")
        || class_id.contains("related")
        || class_id.contains("footer")
        || class_id.contains("header")
        || class_id.contains("breadcrumb")
        || class_id.contains("pagination")
        || class_id.contains("widget")
        || class_id.contains("comment")
        || class_id.contains("yeartime")
        || class_id.contains("p-category")
        || class_id.contains("urlclip")
    {
        return true;
    }

    if let Some(metrics) = metrics.get(&node_key(node))
        && looks_like_related_block(node, metrics)
    {
        return true;
    }

    false
}

fn looks_like_related_block(node: &Handle, metrics: &NodeMetrics) -> bool {
    if metrics.text_len < 24 {
        return false;
    }

    let text = collect_text(node);
    let trimmed = text.trim_start_matches(['・', '-', '–', '—', '>', '›', '»', ' ']);
    let compact = trimmed.to_ascii_lowercase();

    if trimmed.contains("この記事のタイトルとURLをコピーする")
        && (metrics.text_len < 140 || metrics.paragraph_count <= 2)
    {
        return true;
    }

    let markers = [
        "関連記事",
        "関連コンテンツ",
        "関連情報",
        "related articles",
        "related content",
        "more from",
        "see also",
    ];
    if markers
        .iter()
        .any(|marker| trimmed.starts_with(marker) || compact.starts_with(marker))
    {
        return metrics.text_len < 180
            || metrics.paragraph_count <= 2
            || metrics.link_density() > 0.35;
    }

    metrics.link_density() > 0.4
        && metrics.paragraph_count <= 2
        && (trimmed.starts_with("関連記事") || compact.contains("related"))
}

fn class_id_text(node: &Handle) -> String {
    let class = attr_value(node, "class").unwrap_or_default();
    let id = attr_value(node, "id").unwrap_or_default();
    format!("{} {}", class.to_ascii_lowercase(), id.to_ascii_lowercase())
}

fn is_allowed_attr(tag: &str, attr: &str) -> bool {
    match tag {
        "a" => matches!(attr, "href" | "title" | "rel" | "class"),
        "img" => matches!(
            attr,
            "src" | "srcset" | "alt" | "title" | "width" | "height" | "loading"
        ),
        "source" => matches!(attr, "src" | "srcset" | "type" | "media"),
        "video" => matches!(attr, "src" | "controls" | "poster" | "preload"),
        "audio" => matches!(attr, "src" | "controls" | "preload"),
        "iframe" => matches!(
            attr,
            "src"
                | "title"
                | "loading"
                | "width"
                | "height"
                | "referrerpolicy"
                | "allow"
                | "allowfullscreen"
                | "sandbox"
                | "data-content"
                | "data-card-url"
                | "data-url"
                | "data-permalink"
                | "class"
        ),
        "time" => attr == "datetime",
        "blockquote" | "q" => matches!(attr, "cite" | "class"),
        "figure" | "div" | "section" | "article" | "span" => {
            matches!(attr, "lang" | "dir" | "class")
        }
        _ => matches!(attr, "lang" | "dir"),
    }
}

fn is_safe_attr_value(attr: &str, value: &str) -> bool {
    if matches!(attr, "href" | "src" | "srcset") {
        let lower = value.trim().to_ascii_lowercase();
        if lower.starts_with("javascript:") || lower.starts_with("data:text/html") {
            return false;
        }
    }
    true
}

fn is_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn count_punctuation(text: &str) -> usize {
    text.chars()
        .filter(|ch| {
            matches!(
                ch,
                '.' | ',' | '!' | '?' | ';' | ':' | '。' | '、' | '！' | '？' | '；' | '：'
            )
        })
        .count()
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn escape_html_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html_attr(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
