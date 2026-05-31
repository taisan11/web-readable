use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub byline: Option<String>,
    pub excerpt: Option<String>,
    pub site_name: Option<String>,
    pub lang: Option<String>,
    pub published_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MarkdownOptions {
    pub decode_embeds_as_urls: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedContent {
    pub title: String,
    pub content_html: String,
    pub text_content: String,
    pub length: usize,
    pub score: f64,
    pub metadata: Metadata,
}

#[derive(Debug, Clone)]
pub struct ExtractOptions {
    pub min_candidate_text: usize,
    pub min_output_text: usize,
    pub sibling_score_ratio: f64,
    pub include_images: bool,
    pub merge_paginated_content: bool,
    pub max_paginated_pages: usize,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            min_candidate_text: 120,
            min_output_text: 200,
            sibling_score_ratio: 0.22,
            include_images: true,
            merge_paginated_content: false,
            max_paginated_pages: 5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DynamicOptions {
    pub cdp_endpoint: String,
    pub wait_for_navigation: bool,
    pub navigation_timeout: Option<Duration>,
}

impl DynamicOptions {
    pub fn new(cdp_endpoint: impl Into<String>) -> Self {
        Self {
            cdp_endpoint: cdp_endpoint.into(),
            wait_for_navigation: true,
            navigation_timeout: Some(Duration::from_secs(20)),
        }
    }
}
