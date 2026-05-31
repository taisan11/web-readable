mod dom;
#[cfg(feature = "dynamic")]
mod dynamic;
mod error;
mod extractor;
pub mod markdown;
mod model;
mod scoring;

pub use error::{ExtractError, Result};
pub use markdown::{
    extract_to_markdown, extract_to_markdown_from_url,
    extract_to_markdown_from_url_with_markdown_options, extract_to_markdown_with_markdown_options,
    extract_to_markdown_with_options, html_fragment_to_markdown,
    html_fragment_to_markdown_with_options,
};
pub use model::{DynamicOptions, ExtractOptions, ExtractedContent, MarkdownOptions, Metadata};

/// Extracts article-like main content from an HTML document.
pub fn extract(html: &str) -> Result<ExtractedContent> {
    extract_with_options(html, &ExtractOptions::default())
}

/// Extracts article-like main content from an HTML document with custom options.
pub fn extract_with_options(html: &str, options: &ExtractOptions) -> Result<ExtractedContent> {
    extractor::extract_from_html(html, options)
}

#[cfg(feature = "dynamic")]
pub use dynamic::fetch_rendered_html;

/// Fetches a page via CDP and extracts article-like main content.
#[cfg(feature = "dynamic")]
pub async fn extract_from_url(
    url: &str,
    dynamic_options: &DynamicOptions,
    extract_options: &ExtractOptions,
) -> Result<ExtractedContent> {
    let html = dynamic::fetch_rendered_html(url, dynamic_options).await?;
    if extract_options.merge_paginated_content {
        extractor::extract_paginated_from_url(url, &html, dynamic_options, extract_options).await
    } else {
        extract_with_options(&html, extract_options)
    }
}

/// Returns an error when the crate is built without the `dynamic` feature.
#[cfg(not(feature = "dynamic"))]
pub async fn extract_from_url(
    _url: &str,
    _dynamic_options: &DynamicOptions,
    _extract_options: &ExtractOptions,
) -> Result<ExtractedContent> {
    Err(ExtractError::DynamicFeatureDisabled)
}
