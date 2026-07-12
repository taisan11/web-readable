use napi::bindgen_prelude::*;
use napi_derive::napi;

#[derive(Clone, Debug)]
#[napi(object)]
pub struct JsMetadata {
    pub title: Option<String>,
    pub byline: Option<String>,
    pub excerpt: Option<String>,
    pub site_name: Option<String>,
    pub lang: Option<String>,
    pub published_time: Option<String>,
}

#[derive(Clone, Debug)]
#[napi(object)]
pub struct JsExtractedContent {
    pub title: String,
    pub content_html: String,
    pub text_content: String,
    pub length: i64,
    pub score: f64,
    pub metadata: JsMetadata,
}

#[derive(Clone, Debug, Default)]
#[napi(object)]
pub struct JsExtractOptions {
    pub min_candidate_text: Option<i64>,
    pub min_output_text: Option<i64>,
    pub sibling_score_ratio: Option<f64>,
    pub include_images: Option<bool>,
    pub merge_paginated_content: Option<bool>,
    pub max_paginated_pages: Option<i64>,
}

impl JsExtractOptions {
    fn into_native(self) -> Result<crate::ExtractOptions> {
        let defaults = crate::ExtractOptions::default();
        Ok(crate::ExtractOptions {
            min_candidate_text: nonnegative_usize(self.min_candidate_text, defaults.min_candidate_text, "min_candidate_text")?,
            min_output_text: nonnegative_usize(self.min_output_text, defaults.min_output_text, "min_output_text")?,
            sibling_score_ratio: self.sibling_score_ratio.unwrap_or(defaults.sibling_score_ratio),
            include_images: self.include_images.unwrap_or(defaults.include_images),
            merge_paginated_content: self.merge_paginated_content.unwrap_or(defaults.merge_paginated_content),
            max_paginated_pages: nonnegative_usize(self.max_paginated_pages, defaults.max_paginated_pages, "max_paginated_pages")?,
        })
    }
}

fn nonnegative_usize(value: Option<i64>, default: usize, name: &str) -> Result<usize> {
    value.map(|value| usize::try_from(value).map_err(|_| Error::from_reason(format!("{name} must be non-negative")))).transpose().map(|value| value.unwrap_or(default))
}

impl From<crate::ExtractedContent> for JsExtractedContent {
    fn from(value: crate::ExtractedContent) -> Self {
        Self {
            title: value.title,
            content_html: value.content_html,
            text_content: value.text_content,
            length: value.length as i64,
            score: value.score,
            metadata: JsMetadata {
                title: value.metadata.title,
                byline: value.metadata.byline,
                excerpt: value.metadata.excerpt,
                site_name: value.metadata.site_name,
                lang: value.metadata.lang,
                published_time: value.metadata.published_time,
            },
        }
    }
}

fn to_napi_error(error: crate::ExtractError) -> Error {
    Error::from_reason(error.to_string())
}

#[napi(js_name = "extract")]
pub fn extract_js(html: String, options: Option<JsExtractOptions>) -> Result<JsExtractedContent> {
    crate::extract_with_options(&html, &options.unwrap_or_default().into_native()?)
        .map(Into::into)
        .map_err(to_napi_error)
}

#[napi(js_name = "extractToMarkdown")]
pub fn extract_to_markdown_js(html: String, options: Option<JsExtractOptions>) -> Result<String> {
    crate::extract_to_markdown_with_options(&html, &options.unwrap_or_default().into_native()?)
        .map_err(to_napi_error)
}

#[napi(js_name = "htmlFragmentToMarkdown")]
pub fn html_fragment_to_markdown_js(html: String) -> String {
    crate::html_fragment_to_markdown(&html)
}
