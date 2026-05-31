#[cfg(feature = "dynamic")]
use std::collections::HashSet;

#[cfg(any(test, feature = "dynamic"))]
use crate::dom::{attr_value, collect_element_contexts};
use crate::dom::{
    collect_text, collect_text_from_html, extract_metadata, find_first_element, parse_html,
    serialize_clean_fragment,
};
use crate::error::{ExtractError, Result};
use crate::model::{ExtractOptions, ExtractedContent};
use crate::scoring::score_content;
#[cfg(any(test, feature = "dynamic"))]
use url::Url;

pub(crate) fn extract_from_html(html: &str, options: &ExtractOptions) -> Result<ExtractedContent> {
    let dom = parse_html(html);
    let mut metadata = extract_metadata(&dom.document);
    let scoring = match score_content(&dom.document, options) {
        Ok(scoring) => scoring,
        Err(ExtractError::NoCandidate) => {
            return extract_from_fallback_root(&dom.document, &mut metadata, options);
        }
        Err(err) => return Err(err),
    };

    let content_html = serialize_clean_fragment(
        &scoring.selected_nodes,
        &scoring.metrics,
        options.include_images,
    );
    let text_content = collect_text_from_html(&content_html);
    let length = text_content.chars().count();

    if length < options.min_output_text {
        return Err(ExtractError::ContentTooShort {
            min_output_text: options.min_output_text,
        });
    }

    let title = metadata
        .title
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| infer_title_from_content(&content_html));
    metadata.title = Some(title.clone());

    Ok(ExtractedContent {
        title,
        content_html,
        text_content,
        length,
        score: scoring.top_score,
        metadata,
    })
}

#[cfg(feature = "dynamic")]
pub(crate) async fn extract_paginated_from_url(
    start_url: &str,
    first_html: &str,
    dynamic_options: &crate::model::DynamicOptions,
    options: &ExtractOptions,
) -> Result<ExtractedContent> {
    let mut page_options = options.clone();
    page_options.min_candidate_text = 0;
    page_options.min_output_text = 0;
    page_options.merge_paginated_content = false;

    let mut pages = Vec::new();
    let mut current_url = start_url.to_string();
    let mut current_html = first_html.to_string();
    let mut visited = HashSet::new();
    visited.insert(normalize_url_for_tracking(start_url));

    let page_limit = options.max_paginated_pages.max(1);
    for page_index in 0..page_limit {
        let extracted = extract_from_html(&current_html, &page_options)?;
        pages.push(extracted);

        if page_index + 1 >= page_limit {
            break;
        }

        let Some(base_url) = Url::parse(&current_url).ok() else {
            break;
        };
        let Some(next_url) = find_next_page_url(&current_html, &base_url) else {
            break;
        };

        let next_key = normalize_url_for_tracking(next_url.as_str());
        if !visited.insert(next_key) {
            break;
        }

        current_url = next_url.to_string();
        current_html = crate::dynamic::fetch_rendered_html(&current_url, dynamic_options).await?;
    }

    merge_paginated_pages(&pages, options.min_output_text)
}

fn extract_from_fallback_root(
    root: &markup5ever_rcdom::Handle,
    metadata: &mut crate::model::Metadata,
    options: &ExtractOptions,
) -> Result<ExtractedContent> {
    let Some(fallback_root) = find_first_element(root, "body")
        .or_else(|| find_first_element(root, "main"))
        .or_else(|| find_first_element(root, "article"))
    else {
        return Err(ExtractError::NoCandidate);
    };

    let text_content = collect_text(&fallback_root);
    let length = text_content.chars().count();

    if length < options.min_output_text {
        return Err(ExtractError::ContentTooShort {
            min_output_text: options.min_output_text,
        });
    }

    let content_html = wrap_text_as_html(&text_content);
    let title = metadata
        .title
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| infer_title_from_content(&content_html));
    metadata.title = Some(title.clone());

    Ok(ExtractedContent {
        title,
        content_html,
        text_content,
        length,
        score: 0.0,
        metadata: metadata.clone(),
    })
}

#[cfg(any(test, feature = "dynamic"))]
fn merge_paginated_pages(
    pages: &[ExtractedContent],
    min_output_text: usize,
) -> Result<ExtractedContent> {
    let Some(first) = pages.first() else {
        return Err(ExtractError::NoCandidate);
    };

    let content_html = format!(
        "<div>{}</div>",
        pages
            .iter()
            .map(|page| page.content_html.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    let text_content = pages
        .iter()
        .map(|page| page.text_content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let length = text_content.chars().count();

    if length < min_output_text {
        return Err(ExtractError::ContentTooShort { min_output_text });
    }

    Ok(ExtractedContent {
        title: first.title.clone(),
        content_html,
        text_content,
        length,
        score: first.score,
        metadata: first.metadata.clone(),
    })
}

#[cfg(any(test, feature = "dynamic"))]
fn find_next_page_url(root: &str, base_url: &Url) -> Option<Url> {
    let dom = parse_html(root);
    find_next_page_url_in_dom(&dom.document, base_url)
}

#[cfg(any(test, feature = "dynamic"))]
fn find_next_page_url_in_dom(root: &markup5ever_rcdom::Handle, base_url: &Url) -> Option<Url> {
    let mut candidates = Vec::new();

    for ctx in collect_element_contexts(root) {
        match ctx.tag.as_str() {
            "link" => {
                let Some(rel) = attr_value(&ctx.handle, "rel") else {
                    continue;
                };
                if !rel
                    .split_whitespace()
                    .any(|value| value.eq_ignore_ascii_case("next"))
                {
                    continue;
                }
                let Some(href) = attr_value(&ctx.handle, "href") else {
                    continue;
                };
                if let Some(url) = resolve_pagination_url(base_url, &href) {
                    return Some(url);
                }
            }
            "a" => {
                let Some(href) = attr_value(&ctx.handle, "href") else {
                    continue;
                };
                let score = pagination_candidate_score(&ctx.handle);
                if score <= 0 {
                    continue;
                }
                if let Some(url) = resolve_pagination_url(base_url, &href) {
                    candidates.push((score, url));
                }
            }
            _ => {}
        }
    }

    candidates
        .into_iter()
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, url)| url)
}

#[cfg(any(test, feature = "dynamic"))]
fn pagination_candidate_score(handle: &markup5ever_rcdom::Handle) -> i32 {
    let mut combined = String::new();
    for key in ["rel", "aria-label", "title", "class", "id"] {
        if let Some(value) = attr_value(handle, key) {
            combined.push(' ');
            combined.push_str(&value);
        }
    }
    combined.push(' ');
    combined.push_str(&collect_text(handle));
    let text = combined.to_ascii_lowercase();

    if text.contains("prev")
        || text.contains("previous")
        || text.contains("前へ")
        || text.contains("前ページ")
    {
        return 0;
    }

    let mut score = 0;
    if text.contains("next")
        || text.contains("older")
        || text.contains("続き")
        || text.contains("次へ")
        || text.contains("次ページ")
    {
        score += 100;
    }
    if text.contains("pagination") || text.contains("pager") {
        score += 20;
    }
    if matches!(text.trim(), ">" | "›" | "»" | "→" | "next") {
        score += 25;
    }
    if let Some(href) = attr_value(handle, "href") {
        let href = href.to_ascii_lowercase();
        if href.contains("page=") || href.contains("/page/") {
            score += 15;
        }
    }
    score
}

#[cfg(any(test, feature = "dynamic"))]
fn resolve_pagination_url(base_url: &Url, href: &str) -> Option<Url> {
    let trimmed = href.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("javascript:") || lower.starts_with("data:") {
        return None;
    }

    let resolved = base_url.join(trimmed).ok()?;
    if same_origin(base_url, &resolved) && !same_page_without_fragment(base_url, &resolved) {
        Some(resolved)
    } else {
        None
    }
}

#[cfg(any(test, feature = "dynamic"))]
fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.domain() == right.domain()
        && left.port_or_known_default() == right.port_or_known_default()
}

#[cfg(any(test, feature = "dynamic"))]
fn same_page_without_fragment(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.domain() == right.domain()
        && left.port_or_known_default() == right.port_or_known_default()
        && left.path() == right.path()
        && left.query() == right.query()
}

#[cfg(feature = "dynamic")]
fn normalize_url_for_tracking(url: &str) -> String {
    let mut normalized = url.trim().to_string();
    if let Some(fragment) = normalized.find('#') {
        normalized.truncate(fragment);
    }
    normalized
}

fn wrap_text_as_html(text: &str) -> String {
    format!("<p>{}</p>", escape_html_text(text))
}

fn escape_html_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn infer_title_from_content(content_html: &str) -> String {
    for tag in ["h1", "h2"] {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        if let (Some(start), Some(end)) = (content_html.find(&open), content_html.find(&close))
            && end > start + open.len()
        {
            let raw = &content_html[start + open.len()..end];
            let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                return text;
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::{find_next_page_url, merge_paginated_pages};
    use crate::model::ExtractedContent;
    use url::Url;

    #[test]
    fn detects_rel_next_links() {
        let html = r#"
            <html>
              <head>
                <link rel="next" href="/articles?page=2">
              </head>
              <body><article><p>page 1</p></article></body>
            </html>
        "#;
        let base = Url::parse("https://example.com/articles?page=1").expect("valid URL");
        let next = find_next_page_url(html, &base).expect("next page should be found");
        assert_eq!(next.as_str(), "https://example.com/articles?page=2");
    }

    #[test]
    fn merges_paginated_pages_in_order() {
        let pages = vec![
            ExtractedContent {
                title: "Part 1".to_string(),
                content_html: "<p>one</p>".to_string(),
                text_content: "one".to_string(),
                length: 3,
                score: 10.0,
                metadata: Default::default(),
            },
            ExtractedContent {
                title: "Part 2".to_string(),
                content_html: "<p>two</p>".to_string(),
                text_content: "two".to_string(),
                length: 3,
                score: 9.0,
                metadata: Default::default(),
            },
        ];

        let merged = merge_paginated_pages(&pages, 0).expect("merge should work");
        assert_eq!(merged.title, "Part 1");
        assert!(merged.content_html.contains("<p>one</p>"));
        assert!(merged.content_html.contains("<p>two</p>"));
        assert_eq!(merged.text_content, "one\ntwo");
        assert_eq!(merged.length, 7);
    }
}
