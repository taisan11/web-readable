use crate::dom::{
    attr_value, child_handles, collect_text, element_name, find_first_element, parse_html,
};
use crate::error::Result;
use crate::model::{DynamicOptions, ExtractOptions, MarkdownOptions};

pub fn html_fragment_to_markdown(html: &str) -> String {
    html_fragment_to_markdown_with_options(html, &MarkdownOptions::default())
}

pub fn html_fragment_to_markdown_with_options(html: &str, options: &MarkdownOptions) -> String {
    let dom = parse_html(html);
    let mut out = String::new();
    for node in fragment_children(&dom.document) {
        push_block(&node, &mut out, 0, options);
    }
    normalize_output(&out)
}

pub fn extract_to_markdown(html: &str) -> Result<String> {
    extract_to_markdown_with_options(html, &ExtractOptions::default())
}

pub fn extract_to_markdown_with_options(html: &str, options: &ExtractOptions) -> Result<String> {
    let extracted = crate::extract_with_options(html, options)?;
    Ok(prepend_title_heading(
        html_fragment_to_markdown(&extracted.content_html),
        extract_title_hint(html, &extracted.title).as_deref(),
    ))
}

pub fn extract_to_markdown_with_markdown_options(
    html: &str,
    extract_options: &ExtractOptions,
    markdown_options: &MarkdownOptions,
) -> Result<String> {
    let extracted = crate::extract_with_options(html, extract_options)?;
    Ok(prepend_title_heading(
        html_fragment_to_markdown_with_options(&extracted.content_html, markdown_options),
        extract_title_hint(html, &extracted.title).as_deref(),
    ))
}

#[cfg(feature = "dynamic")]
pub async fn extract_to_markdown_from_url(
    url: &str,
    dynamic_options: &DynamicOptions,
    extract_options: &ExtractOptions,
) -> Result<String> {
    let mut extract_options = extract_options.clone();
    if crate::dynamic::is_lightpanda_endpoint(&dynamic_options.cdp_endpoint) {
        extract_options.min_output_text = 0;
    }

    let extracted = crate::extract_from_url(url, dynamic_options, &extract_options).await?;
    Ok(prepend_title_heading(
        html_fragment_to_markdown(&extracted.content_html),
        normalize_article_title(&extracted.title).as_deref(),
    ))
}

#[cfg(feature = "dynamic")]
pub async fn extract_to_markdown_from_url_with_markdown_options(
    url: &str,
    dynamic_options: &DynamicOptions,
    extract_options: &ExtractOptions,
    markdown_options: &MarkdownOptions,
) -> Result<String> {
    let mut extract_options = extract_options.clone();
    if crate::dynamic::is_lightpanda_endpoint(&dynamic_options.cdp_endpoint) {
        extract_options.min_output_text = 0;
    }

    let extracted = crate::extract_from_url(url, dynamic_options, &extract_options).await?;
    Ok(prepend_title_heading(
        html_fragment_to_markdown_with_options(&extracted.content_html, markdown_options),
        normalize_article_title(&extracted.title).as_deref(),
    ))
}

#[cfg(not(feature = "dynamic"))]
pub async fn extract_to_markdown_from_url(
    _url: &str,
    _dynamic_options: &DynamicOptions,
    _extract_options: &ExtractOptions,
) -> Result<String> {
    Err(crate::ExtractError::DynamicFeatureDisabled)
}

#[cfg(not(feature = "dynamic"))]
pub async fn extract_to_markdown_from_url_with_markdown_options(
    _url: &str,
    _dynamic_options: &DynamicOptions,
    _extract_options: &ExtractOptions,
    _markdown_options: &MarkdownOptions,
) -> Result<String> {
    Err(crate::ExtractError::DynamicFeatureDisabled)
}

fn fragment_children(document: &markup5ever_rcdom::Handle) -> Vec<markup5ever_rcdom::Handle> {
    for child in child_handles(document) {
        if element_name(&child).as_deref() == Some("html") {
            for grandchild in child_handles(&child) {
                if element_name(&grandchild).as_deref() == Some("body") {
                    return child_handles(&grandchild);
                }
            }
        }
    }
    child_handles(document)
}

fn push_block(
    node: &markup5ever_rcdom::Handle,
    out: &mut String,
    indent: usize,
    options: &MarkdownOptions,
) {
    if let Some(text) = render_block(node, indent, options) {
        append_block(out, &text);
    }
}

fn render_block(
    node: &markup5ever_rcdom::Handle,
    indent: usize,
    options: &MarkdownOptions,
) -> Option<String> {
    if options.decode_embeds_as_urls
        && let Some(url) = render_embedded_url(node)
    {
        return Some(url);
    }

    match &node.data {
        markup5ever_rcdom::NodeData::Text { contents } => {
            let text = normalize_inline_text(contents.borrow().as_ref());
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        }
        markup5ever_rcdom::NodeData::Element { .. } => {
            let tag = element_name(node).unwrap_or_default();
            match tag.as_str() {
                "script" | "style" | "noscript" | "template" => None,
                "br" => Some("  \n".to_string()),
                "hr" => Some(format!("{}---", indent_prefix(indent))),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level = tag[1..].parse::<usize>().unwrap_or(1).clamp(1, 6);
                    let body = render_inline_children(node, options);
                    if body.trim().is_empty() {
                        None
                    } else {
                        Some(format!(
                            "{}{} {}",
                            indent_prefix(indent),
                            "#".repeat(level),
                            body.trim()
                        ))
                    }
                }
                "p" => {
                    let body = render_inline_children(node, options).trim().to_string();
                    if body.is_empty() {
                        None
                    } else {
                        Some(format!("{}{}", indent_prefix(indent), body))
                    }
                }
                "blockquote" => render_blockquote(node, indent, options),
                "ul" => Some(render_list(node, indent, false, options)),
                "ol" => Some(render_list(node, indent, true, options)),
                "li" => Some(render_list_item(node, indent, options)),
                "pre" => Some(render_code_block(node, indent)),
                "table" => render_table(node, indent, options),
                "figure" | "section" | "article" | "main" | "div" | "body" => {
                    render_container(node, indent, options)
                }
                "math" => Some(render_math(node)),
                "img" => Some(render_image(node)),
                "a" => {
                    let body = render_inline_children(node, options).trim().to_string();
                    Some(render_link(node, &body, options))
                }
                "strong" | "b" => {
                    let body = render_inline_children(node, options);
                    if body.trim().is_empty() {
                        None
                    } else {
                        Some(format!("**{}**", body.trim()))
                    }
                }
                "em" | "i" | "cite" => {
                    let body = render_inline_children(node, options);
                    if body.trim().is_empty() {
                        None
                    } else {
                        Some(format!("*{}*", body.trim()))
                    }
                }
                "code" => Some(render_inline_code(node)),
                "span" | "small" | "sup" | "sub" | "mark" | "del" | "ins" | "u" | "q" | "time"
                | "abbr" => Some(render_inline_children(node, options)),
                "thead" | "tbody" | "tfoot" => render_container(node, indent, options),
                "tr" | "td" | "th" => Some(render_inline_children(node, options)),
                _ => {
                    let body = render_inline_children(node, options);
                    if body.trim().is_empty() {
                        render_container(node, indent, options)
                    } else {
                        Some(body)
                    }
                }
            }
        }
        _ => None,
    }
}

fn render_container(
    node: &markup5ever_rcdom::Handle,
    indent: usize,
    options: &MarkdownOptions,
) -> Option<String> {
    let mut parts = Vec::new();
    for child in child_handles(node) {
        if let Some(text) = render_block(&child, indent, options) {
            let text = text.trim().to_string();
            if !text.is_empty() {
                parts.push(text);
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

fn render_inline_children(node: &markup5ever_rcdom::Handle, options: &MarkdownOptions) -> String {
    let mut out = String::new();
    for child in child_handles(node) {
        let part = render_inline(&child, options);
        if part.is_empty() {
            continue;
        }
        if needs_inline_space(&out, &part) {
            out.push(' ');
        }
        out.push_str(&part);
    }
    out
}

fn render_inline(node: &markup5ever_rcdom::Handle, options: &MarkdownOptions) -> String {
    if options.decode_embeds_as_urls
        && let Some(url) = render_embedded_url(node)
    {
        return format!("\n{url}");
    }

    match &node.data {
        markup5ever_rcdom::NodeData::Text { contents } => {
            escape_markdown_text(&normalize_inline_text(contents.borrow().as_ref()))
        }
        markup5ever_rcdom::NodeData::Element { .. } => {
            let tag = element_name(node).unwrap_or_default();
            match tag.as_str() {
                "br" => "  \n".to_string(),
                "script" | "style" | "noscript" | "template" => String::new(),
                "a" => {
                    let body = render_inline_children(node, options).trim().to_string();
                    render_link(node, &body, options)
                }
                "strong" | "b" => {
                    let body = render_inline_children(node, options).trim().to_string();
                    if body.is_empty() {
                        String::new()
                    } else {
                        format!("**{}**", body)
                    }
                }
                "em" | "i" | "cite" => {
                    let body = render_inline_children(node, options).trim().to_string();
                    if body.is_empty() {
                        String::new()
                    } else {
                        format!("*{}*", body)
                    }
                }
                "code" => render_inline_code(node),
                "img" => render_image(node),
                "math" => render_math(node),
                "span" | "small" | "sup" | "sub" | "mark" | "del" | "ins" | "u" | "q" | "time"
                | "abbr" | "mn" | "mi" | "mo" | "mtext" | "ms" | "mrow" | "mfenced" | "mfrac"
                | "msup" | "msub" | "msubsup" | "msqrt" | "mroot" | "munderover" | "munder"
                | "mover" | "semantics" | "annotation" | "annotation-xml" | "mathml" | "mstyle"
                | "merror" | "mpadded" | "mphantom" | "mtable" | "mtr" | "mtd" => {
                    render_inline_children(node, options)
                }
                _ => {
                    let body = render_inline_children(node, options);
                    if body.is_empty() {
                        collect_text(node)
                    } else {
                        body
                    }
                }
            }
        }
        _ => String::new(),
    }
}

fn render_list(
    node: &markup5ever_rcdom::Handle,
    indent: usize,
    ordered: bool,
    options: &MarkdownOptions,
) -> String {
    let mut out = Vec::new();
    let mut index = 1usize;
    for child in child_handles(node) {
        if element_name(&child).as_deref() != Some("li") {
            continue;
        }
        let marker = if ordered {
            let marker = format!("{}.", index);
            index += 1;
            marker
        } else {
            "-".to_string()
        };
        out.push(render_list_item_with_marker(
            &child, indent, &marker, options,
        ));
    }
    out.join("\n")
}

fn render_list_item(
    node: &markup5ever_rcdom::Handle,
    indent: usize,
    options: &MarkdownOptions,
) -> String {
    render_list_item_with_marker(node, indent, "-", options)
}

fn render_list_item_with_marker(
    node: &markup5ever_rcdom::Handle,
    indent: usize,
    marker: &str,
    options: &MarkdownOptions,
) -> String {
    let mut body_parts = Vec::new();
    for child in child_handles(node) {
        if let Some(text) = render_block(&child, indent + 2, options) {
            let text = text.trim().to_string();
            if !text.is_empty() {
                body_parts.push(text);
            }
        }
    }

    if body_parts.is_empty() {
        body_parts.push(render_inline_children(node, options).trim().to_string());
    }

    let content = body_parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if content.is_empty() {
        return String::new();
    }

    let prefix = format!("{}{} ", indent_prefix(indent), marker);
    indent_multiline(&content, &prefix, indent + marker.len() + 1)
}

fn render_blockquote(
    node: &markup5ever_rcdom::Handle,
    indent: usize,
    options: &MarkdownOptions,
) -> Option<String> {
    let inner = render_container(node, indent + 2, options)?;
    let mut quoted = String::new();
    for line in inner.lines() {
        if line.trim().is_empty() {
            quoted.push_str(">\n");
        } else {
            quoted.push_str("> ");
            quoted.push_str(line);
            quoted.push('\n');
        }
    }
    Some(quoted.trim_end().to_string())
}

fn render_code_block(node: &markup5ever_rcdom::Handle, indent: usize) -> String {
    let mut code_text = String::new();
    for child in child_handles(node) {
        if element_name(&child).as_deref() == Some("code") {
            code_text = render_raw_text(&child);
            break;
        }
    }
    if code_text.is_empty() {
        code_text = render_raw_text(node);
    }
    let code_text = code_text.trim_matches('\n');
    let fence = code_fence(code_text);
    let mut out = String::new();
    out.push_str(&indent_prefix(indent));
    out.push_str(&fence);
    out.push('\n');
    out.push_str(code_text);
    out.push('\n');
    out.push_str(&indent_prefix(indent));
    out.push_str(&fence);
    out
}

fn render_image(node: &markup5ever_rcdom::Handle) -> String {
    let alt = attr_value(node, "alt").unwrap_or_default();
    let src = attr_value(node, "src").unwrap_or_default();
    let title = attr_value(node, "title")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    if src.is_empty() {
        return alt;
    }

    let mut out = format!("![{}]({}", escape_brackets(&alt), src);
    if let Some(title) = title {
        out.push(' ');
        out.push('"');
        out.push_str(&escape_quotes(&title));
        out.push('"');
    }
    out.push(')');
    out
}

fn render_link(node: &markup5ever_rcdom::Handle, body: &str, options: &MarkdownOptions) -> String {
    let href = attr_value(node, "href").unwrap_or_default();
    if href.is_empty() {
        return body.to_string();
    }
    if options.decode_embeds_as_urls && is_card_like_node(node) {
        if let Some(url) = normalize_embed_url(&href) {
            return url;
        }
    }
    let text = if body.trim().is_empty() {
        href.clone()
    } else {
        body.trim().to_string()
    };
    format!("[{}]({})", escape_brackets(&text), href)
}

fn render_embedded_url(node: &markup5ever_rcdom::Handle) -> Option<String> {
    if !is_card_like_node(node) {
        return None;
    }

    embedded_url_from_node(node).or_else(|| find_descendant_embedded_url(node))
}

fn is_card_like_node(node: &markup5ever_rcdom::Handle) -> bool {
    let tag = element_name(node).unwrap_or_default();
    let class_id = class_id_text(node);
    let has_embed_marker = class_id.contains("card")
        || class_id.contains("embed")
        || class_id.contains("twitter-tweet")
        || class_id.contains("instagram-media")
        || class_id.contains("tiktok-embed")
        || class_id.contains("link-card")
        || attr_value(node, "data-content").is_some()
        || attr_value(node, "data-card-url").is_some()
        || attr_value(node, "data-url").is_some()
        || attr_value(node, "data-permalink").is_some();

    match tag.as_str() {
        "a" | "iframe" => has_embed_marker,
        "blockquote" | "figure" | "div" | "section" | "article" | "span" => has_embed_marker,
        _ => false,
    }
}

fn class_id_text(node: &markup5ever_rcdom::Handle) -> String {
    let class = attr_value(node, "class").unwrap_or_default();
    let id = attr_value(node, "id").unwrap_or_default();
    format!("{} {}", class.to_ascii_lowercase(), id.to_ascii_lowercase())
}

fn embedded_url_from_node(node: &markup5ever_rcdom::Handle) -> Option<String> {
    let tag = element_name(node).unwrap_or_default();
    match tag.as_str() {
        "a" => attr_value(node, "href").and_then(|href| normalize_embed_url(&href)),
        "iframe" => attr_value(node, "data-content")
            .and_then(|value| decode_percent_encoded_url(&value))
            .or_else(|| attr_value(node, "data-card-url"))
            .or_else(|| attr_value(node, "data-url"))
            .or_else(|| attr_value(node, "data-permalink"))
            .and_then(|url| normalize_embed_url(&url)),
        _ => attr_value(node, "data-card-url")
            .or_else(|| attr_value(node, "data-url"))
            .or_else(|| attr_value(node, "data-permalink"))
            .or_else(|| attr_value(node, "href"))
            .and_then(|url| normalize_embed_url(&url)),
    }
}

fn find_descendant_embedded_url(node: &markup5ever_rcdom::Handle) -> Option<String> {
    for child in child_handles(node) {
        if let Some(url) = embedded_url_from_node(&child) {
            return Some(url);
        }
        if let Some(url) = find_descendant_embedded_url(&child) {
            return Some(url);
        }
    }
    None
}

fn normalize_embed_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("javascript:") || lower.starts_with("data:text/html") {
        return None;
    }
    Some(trimmed.to_string())
}

fn decode_percent_encoded_url(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_value(bytes[i + 1])?;
                let lo = hex_value(bytes[i + 2])?;
                decoded.push((hi << 4) | lo);
                i += 3;
            }
            byte => {
                decoded.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(decoded)
        .ok()
        .and_then(|url| normalize_embed_url(&url))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn render_inline_code(node: &markup5ever_rcdom::Handle) -> String {
    let text = render_raw_text(node).trim().to_string();
    if text.is_empty() {
        return String::new();
    }
    let fence = inline_code_fence(&text);
    format!("{fence}{text}{fence}")
}

fn render_math(node: &markup5ever_rcdom::Handle) -> String {
    let display = attr_value(node, "display")
        .map(|v| v.trim().eq_ignore_ascii_case("block"))
        .unwrap_or(false);
    let tex = render_math_node(node).trim().to_string();
    if tex.is_empty() {
        return String::new();
    }
    if display {
        format!("$$\n{}\n$$", tex)
    } else {
        format!("${}$", tex)
    }
}

fn render_math_node(node: &markup5ever_rcdom::Handle) -> String {
    match &node.data {
        markup5ever_rcdom::NodeData::Text { contents } => {
            escape_tex_text(&normalize_inline_text(contents.borrow().as_ref()))
        }
        markup5ever_rcdom::NodeData::Element { .. } => {
            let tag = element_name(node).unwrap_or_default();
            match tag.as_str() {
                "math" | "mrow" | "semantics" | "mstyle" | "mpadded" | "mphantom" => {
                    join_math_children(node)
                }
                "mi" | "mn" | "mtext" => escape_tex_text(&collect_text(node)),
                "mo" => render_math_operator(&collect_text(node)),
                "ms" => format!("\\text{{{}}}", escape_tex_text(&collect_text(node))),
                "mfrac" => {
                    let parts = child_handles(node);
                    let num = parts.get(0).map(render_math_node).unwrap_or_default();
                    let den = parts.get(1).map(render_math_node).unwrap_or_default();
                    format!("\\frac{{{}}}{{{}}}", num, den)
                }
                "msup" => render_math_power(node, false),
                "msub" => render_math_power(node, true),
                "msubsup" => {
                    let parts = child_handles(node);
                    let base = parts.get(0).map(render_math_node).unwrap_or_default();
                    let sub = parts.get(1).map(render_math_node).unwrap_or_default();
                    let sup = parts.get(2).map(render_math_node).unwrap_or_default();
                    format!("{}^{{{}}}_{{{}}}", base, sup, sub)
                }
                "msqrt" => format!("\\sqrt{{{}}}", join_math_children(node)),
                "mroot" => {
                    let parts = child_handles(node);
                    let base = parts.get(0).map(render_math_node).unwrap_or_default();
                    let root = parts.get(1).map(render_math_node).unwrap_or_default();
                    format!("\\sqrt[{}]{{{}}}", root, base)
                }
                "mfenced" => render_math_fenced(node),
                "munderover" => render_math_under_over(node, true, true),
                "munder" => render_math_under_over(node, true, false),
                "mover" => render_math_under_over(node, false, true),
                "mtable" => render_math_table(node),
                "mtr" | "mtd" | "thead" | "tbody" | "tfoot" => join_math_children(node),
                "annotation" => {
                    let encoding = attr_value(node, "encoding").unwrap_or_default();
                    if encoding.to_ascii_lowercase().contains("tex") {
                        collect_text(node)
                    } else {
                        join_math_children(node)
                    }
                }
                "annotation-xml" => join_math_children(node),
                _ => join_math_children(node),
            }
        }
        _ => String::new(),
    }
}

fn render_math_power(node: &markup5ever_rcdom::Handle, is_sub: bool) -> String {
    let parts = child_handles(node);
    let base = parts.get(0).map(render_math_node).unwrap_or_default();
    let script = parts.get(1).map(render_math_node).unwrap_or_default();
    if is_sub {
        format!("{}_{{{}}}", base, script)
    } else {
        format!("{}^{{{}}}", base, script)
    }
}

fn render_math_fenced(node: &markup5ever_rcdom::Handle) -> String {
    let open = attr_value(node, "open").unwrap_or_else(|| "(".to_string());
    let close = attr_value(node, "close").unwrap_or_else(|| ")".to_string());
    let body = join_math_children(node);
    format!("\\left{}{}\\right{}", open, body, close)
}

fn render_math_under_over(node: &markup5ever_rcdom::Handle, under: bool, over: bool) -> String {
    let parts = child_handles(node);
    let base = parts.first().map(render_math_node).unwrap_or_default();
    let under_node = parts.get(1).map(render_math_node).unwrap_or_default();
    let over_node = parts.get(2).map(render_math_node).unwrap_or_default();
    match (under, over) {
        (true, true) => format!(
            "\\overset{{{}}}{{\\underset{{{}}}{{{}}}}}",
            over_node, under_node, base
        ),
        (true, false) => format!("\\underset{{{}}}{{{}}}", under_node, base),
        (false, true) => format!("\\overset{{{}}}{{{}}}", over_node, base),
        (false, false) => base,
    }
}

fn render_math_table(node: &markup5ever_rcdom::Handle) -> String {
    let mut rows = Vec::new();
    for child in child_handles(node) {
        if element_name(&child).as_deref() != Some("mtr") {
            continue;
        }
        let mut cols = Vec::new();
        for cell in child_handles(&child) {
            if element_name(&cell).as_deref() != Some("mtd") {
                continue;
            }
            cols.push(join_math_children(&cell));
        }
        if !cols.is_empty() {
            rows.push(cols.join(" & "));
        }
    }
    if rows.is_empty() {
        join_math_children(node)
    } else {
        format!("\\begin{{matrix}} {} \\end{{matrix}}", rows.join(" \\\\ "))
    }
}

fn join_math_children(node: &markup5ever_rcdom::Handle) -> String {
    let mut out = String::new();
    for child in child_handles(node) {
        let part = render_math_node(&child);
        if part.is_empty() {
            continue;
        }
        if needs_math_space(&out, &part) {
            out.push(' ');
        }
        out.push_str(&part);
    }
    out
}

fn render_math_operator(text: &str) -> String {
    let trimmed = text.trim();
    match trimmed {
        "(" | "[" | "{" | "⟨" => trimmed.to_string(),
        ")" | "]" | "}" | "⟩" => trimmed.to_string(),
        "," | ";" | ":" => trimmed.to_string(),
        "+" | "-" | "=" | "*" | "/" | "×" | "÷" | "±" | "∓" | "<" | ">" | "≤" | "≥" | "≠" | "≈"
        | "∧" | "∨" | "∑" | "∏" | "∫" => trimmed.to_string(),
        _ => escape_tex_text(trimmed),
    }
}

fn needs_math_space(left: &str, right: &str) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let left_last = left.chars().rev().find(|ch| !ch.is_whitespace());
    let right_first = right.chars().find(|ch| !ch.is_whitespace());
    match (left_last, right_first) {
        (Some(l), Some(r)) => {
            let no_space_after = matches!(l, '(' | '[' | '{' | '⟨' | '|' | '^' | '_');
            let no_space_before =
                matches!(r, ')' | ']' | '}' | '⟩' | '|' | ',' | ';' | ':' | '^' | '_');
            !(no_space_after || no_space_before)
        }
        _ => false,
    }
}

fn render_raw_text(node: &markup5ever_rcdom::Handle) -> String {
    match &node.data {
        markup5ever_rcdom::NodeData::Text { contents } => contents.borrow().to_string(),
        _ => {
            let mut out = String::new();
            for child in child_handles(node) {
                out.push_str(&render_raw_text(&child));
            }
            out
        }
    }
}

fn code_fence(text: &str) -> String {
    "`".repeat(backtick_fence_len(text, 3))
}

fn inline_code_fence(text: &str) -> String {
    "`".repeat(backtick_fence_len(text, 1))
}

fn backtick_fence_len(text: &str, min_len: usize) -> usize {
    let mut longest = 0usize;
    let mut current = 0usize;
    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest.saturating_add(1).max(min_len)
}

fn indent_prefix(indent: usize) -> String {
    " ".repeat(indent)
}

fn indent_multiline(text: &str, prefix: &str, hanging_indent: usize) -> String {
    let mut out = String::new();
    let mut lines = text.lines().peekable();
    if let Some(first) = lines.next() {
        out.push_str(prefix);
        out.push_str(first.trim_end());
    }
    for line in lines {
        out.push('\n');
        if line.trim().is_empty() {
            out.push_str(&" ".repeat(hanging_indent));
        } else {
            out.push_str(&" ".repeat(hanging_indent));
            out.push_str(line.trim_end());
        }
    }
    out
}

fn append_block(out: &mut String, block: &str) {
    let block = block.trim();
    if block.is_empty() {
        return;
    }
    if !out.is_empty() && !out.ends_with("\n\n") {
        if out.ends_with('\n') {
            out.push('\n');
        } else {
            out.push_str("\n\n");
        }
    }
    out.push_str(block);
    out.push_str("\n\n");
}

fn normalize_output(out: &str) -> String {
    out.trim().replace("\n\n\n", "\n\n")
}

fn prepend_title_heading(markdown: String, title: Option<&str>) -> String {
    let Some(title) = title.map(str::trim).filter(|title| !title.is_empty()) else {
        return markdown;
    };

    let heading = format!("# {title}");
    let first_non_empty = markdown.lines().find(|line| !line.trim().is_empty());
    if first_non_empty.is_some_and(|line| line.trim() == heading) {
        return markdown;
    }

    let mut out = String::new();
    out.push_str(&heading);
    out.push_str("\n\n");
    out.push_str(markdown.trim());
    out.push('\n');
    normalize_output(&out)
}

fn extract_title_hint(html: &str, fallback: &str) -> Option<String> {
    title_from_html(html).or_else(|| normalize_article_title(fallback))
}

fn title_from_html(html: &str) -> Option<String> {
    let dom = parse_html(html);
    for tag in ["h1", "h2"] {
        let Some(node) = find_first_element(&dom.document, tag) else {
            continue;
        };
        let title = collect_text(&node).trim().to_string();
        if !title.is_empty() {
            return Some(title);
        }
    }
    None
}

fn normalize_article_title(title: &str) -> Option<String> {
    let title = title.trim();
    if title.is_empty() {
        return None;
    }

    for separator in [" - ", " | ", " — "] {
        if let Some((left, right)) = title.rsplit_once(separator)
            && right.chars().count() <= 24
        {
            let stripped = left.trim();
            if !stripped.is_empty() {
                return Some(stripped.to_string());
            }
        }
    }

    Some(title.to_string())
}

fn normalize_inline_text(text: &str) -> String {
    let has_leading = text.chars().next().is_some_and(char::is_whitespace);
    let has_trailing = text.chars().last().is_some_and(char::is_whitespace);
    let mut normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return normalized;
    }
    if has_leading {
        normalized.insert(0, ' ');
    }
    if has_trailing {
        normalized.push(' ');
    }
    normalized
}

fn escape_markdown_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '!' | '|'
            | '>' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn escape_brackets(input: &str) -> String {
    input.replace('[', "\\[").replace(']', "\\]")
}

fn escape_quotes(input: &str) -> String {
    input.replace('"', "\\\"")
}

fn escape_tex_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' | '{' | '}' | '$' | '#' | '%' | '&' | '_' | '^' | '~' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn needs_inline_space(left: &str, right: &str) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    if right.starts_with('\n') || left.ends_with('\n') {
        return false;
    }
    let left_last = left.chars().rev().find(|ch| !ch.is_whitespace());
    let right_first = right.chars().find(|ch| !ch.is_whitespace());
    match (left_last, right_first) {
        (Some(l), Some(r)) => {
            let left_open = matches!(l, '(' | '[' | '{' | '/' | '\\' | '`' | '*' | '_');
            let right_close = matches!(
                r,
                ')' | ']' | '}' | ',' | '.' | ';' | ':' | '/' | '\\' | '`' | '*' | '_'
            );
            !(left_open || right_close)
        }
        _ => false,
    }
}

fn render_table(
    node: &markup5ever_rcdom::Handle,
    indent: usize,
    options: &MarkdownOptions,
) -> Option<String> {
    let mut rows = Vec::new();
    for child in child_handles(node) {
        if let Some(tag) = element_name(&child) {
            if tag == "tr" {
                rows.push(render_table_row(&child, options));
                continue;
            }
            if tag == "thead" || tag == "tbody" || tag == "tfoot" {
                for row in child_handles(&child) {
                    if element_name(&row).as_deref() == Some("tr") {
                        rows.push(render_table_row(&row, options));
                    }
                }
            }
        }
    }

    if rows.is_empty() {
        return render_container(node, indent, options);
    }

    let header = rows.remove(0);
    let mut out = String::new();
    out.push_str(&indent_prefix(indent));
    out.push_str("| ");
    out.push_str(&header.join(" | "));
    out.push_str(" |\n");
    out.push_str(&indent_prefix(indent));
    out.push_str("| ");
    out.push_str(&vec!["---"; header.len()].join(" | "));
    out.push_str(" |\n");
    for row in rows {
        out.push_str(&indent_prefix(indent));
        out.push_str("| ");
        out.push_str(&row.join(" | "));
        out.push_str(" |\n");
    }
    Some(out.trim_end().to_string())
}

fn render_table_row(node: &markup5ever_rcdom::Handle, options: &MarkdownOptions) -> Vec<String> {
    let mut cells = Vec::new();
    for child in child_handles(node) {
        if let Some(tag) = element_name(&child) {
            if tag == "td" || tag == "th" {
                let text = render_inline_children(&child, options).trim().to_string();
                cells.push(text);
            }
        }
    }
    cells
}
