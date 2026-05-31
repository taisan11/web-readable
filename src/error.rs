use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("no content candidate found")]
    NoCandidate,
    #[error("extracted content was too short (minimum {min_output_text} chars)")]
    ContentTooShort { min_output_text: usize },
    #[error("dynamic extraction requires the `dynamic` feature")]
    DynamicFeatureDisabled,
    #[error("dynamic extraction timed out after {seconds:.2} seconds")]
    DynamicTimeout { seconds: f32 },
    #[error("lightpanda fetch failed: {message}")]
    LightpandaFetchFailed { message: String },
    #[cfg(feature = "dynamic")]
    #[error(transparent)]
    Cdp(#[from] chromiumoxide::error::CdpError),
}

pub type Result<T> = std::result::Result<T, ExtractError>;
