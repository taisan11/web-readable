use chromiumoxide::Browser;
use chromiumoxide::cdp::browser_protocol::emulation::SetLocaleOverrideParams;
use chromiumoxide::cdp::browser_protocol::network::SetUserAgentOverrideParams;
use futures::StreamExt;
use std::{
    io::{Read, Write},
    net::TcpStream,
    process::Command,
    time::Duration,
};

use crate::error::{ExtractError, Result};
use crate::model::DynamicOptions;

const TEST_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";
const LIGHTPANDA_USER_AGENT: &str =
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";
const TEST_ACCEPT_LANGUAGE: &str = "ja-JP,ja;q=0.9";
const TEST_LOCALE: &str = "ja-JP";
const TEST_PLATFORM: &str = "MacIntel";

/// Fetches rendered HTML by connecting to an existing CDP endpoint.
pub async fn fetch_rendered_html(url: &str, options: &DynamicOptions) -> Result<String> {
    let lightpanda_endpoint = is_lightpanda_endpoint(&options.cdp_endpoint);
    if lightpanda_endpoint {
        return fetch_rendered_html_with_lightpanda(url).await;
    }

    let (mut browser, mut handler) = Browser::connect(options.cdp_endpoint.as_str()).await?;

    let handler_task = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                break;
            }
        }
    });

    let page = browser.new_page(url).await?;
    let html = async {
        apply_page_overrides(&page, lightpanda_endpoint).await?;

        wait_with_optional_timeout(options, page.reload()).await?;

        if lightpanda_endpoint {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        let mut html = wait_with_optional_timeout(options, page.content()).await?;
        if lightpanda_endpoint {
            let mut attempts = 0usize;
            while html.len() < 10_000 && attempts < 6 {
                tokio::time::sleep(Duration::from_secs(2)).await;
                html = wait_with_optional_timeout(options, page.content()).await?;
                attempts += 1;
            }
        }

        Ok::<String, ExtractError>(html)
    }
    .await?;

    let _ = browser.close().await;
    handler_task.abort();

    Ok(html)
}

async fn fetch_rendered_html_with_lightpanda(url: &str) -> Result<String> {
    let url = url.to_string();
    tokio::task::spawn_blocking(move || {
        let output = Command::new("lightpanda")
            .args(["fetch", &url, "--dump", "html", "--wait-ms", "5000"])
            .output()
            .map_err(|err| ExtractError::LightpandaFetchFailed {
                message: err.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(ExtractError::LightpandaFetchFailed {
                message: if stderr.is_empty() {
                    format!("`lightpanda fetch` exited with {}", output.status)
                } else {
                    stderr
                },
            });
        }

        String::from_utf8(output.stdout).map_err(|err| ExtractError::LightpandaFetchFailed {
            message: err.to_string(),
        })
    })
    .await
    .map_err(|err| ExtractError::LightpandaFetchFailed {
        message: err.to_string(),
    })?
}

async fn apply_page_overrides(
    page: &chromiumoxide::page::Page,
    lightpanda_endpoint: bool,
) -> Result<()> {
    let params = if lightpanda_endpoint {
        SetUserAgentOverrideParams {
            user_agent: LIGHTPANDA_USER_AGENT.to_string(),
            accept_language: None,
            platform: None,
            user_agent_metadata: None,
        }
    } else {
        SetUserAgentOverrideParams {
            user_agent: TEST_USER_AGENT.to_string(),
            accept_language: Some(TEST_ACCEPT_LANGUAGE.to_string()),
            platform: Some(TEST_PLATFORM.to_string()),
            user_agent_metadata: None,
        }
    };

    if let Err(err) = page.set_user_agent(params).await {
        if !is_unsupported_optional_cdp_error(&err) {
            return Err(err.into());
        }
    }

    if !lightpanda_endpoint {
        if let Err(err) = page
            .emulate_locale(SetLocaleOverrideParams {
                locale: Some(TEST_LOCALE.to_string()),
            })
            .await
        {
            if !is_unsupported_optional_cdp_error(&err) {
                return Err(err.into());
            }
        }
    }

    Ok(())
}

fn is_unsupported_optional_cdp_error(err: &chromiumoxide::error::CdpError) -> bool {
    matches!(
        err,
        chromiumoxide::error::CdpError::Chrome(chrome_err)
            if is_unknown_method_message(chrome_err.message.as_str())
    )
}

fn is_unknown_method_message(message: &str) -> bool {
    matches!(message, "UnknownMethod" | "Method not found")
}

fn is_lightpanda_product(product: &str) -> bool {
    product.contains("Lightpanda")
}

pub(crate) fn is_lightpanda_endpoint(endpoint: &str) -> bool {
    matches!(probe_browser_label(endpoint).as_deref(), Some(label) if is_lightpanda_product(label))
}

fn probe_browser_label(endpoint: &str) -> Option<String> {
    let authority = endpoint
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(endpoint)
        .split('/')
        .next()?;

    let mut stream = TcpStream::connect(authority).ok()?;
    stream
        .write_all(
            format!("GET /json/version HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let body = response.split_once("\r\n\r\n")?.1;

    browser_label_from_version_body(body)
}

fn browser_label_from_version_body(body: &str) -> Option<String> {
    let key = "\"Browser\"";
    let start = body.find(key)? + key.len();
    let after_colon = body[start..].find(':')? + start + 1;
    let value_start = body[after_colon..].find('"')? + after_colon + 1;
    let value_end = body[value_start..].find('"')? + value_start;
    Some(body[value_start..value_end].to_string())
}

async fn wait_with_optional_timeout<T, F>(options: &DynamicOptions, future: F) -> Result<T>
where
    F: std::future::Future<Output = chromiumoxide::error::Result<T>>,
{
    if let Some(timeout) = options.navigation_timeout {
        tokio::time::timeout(timeout, future)
            .await
            .map_err(|_| ExtractError::DynamicTimeout {
                seconds: timeout.as_secs_f32(),
            })?
            .map_err(ExtractError::from)
    } else {
        future.await.map_err(ExtractError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        browser_label_from_version_body, is_lightpanda_product, is_unknown_method_message,
    };

    #[test]
    fn recognizes_unsupported_method_messages() {
        assert!(is_unknown_method_message("UnknownMethod"));
        assert!(is_unknown_method_message("Method not found"));
        assert!(!is_unknown_method_message("SomeOtherError"));
    }

    #[test]
    fn recognizes_lightpanda_products() {
        assert!(is_lightpanda_product("Lightpanda/1.0"));
        assert!(is_lightpanda_product("Lightpanda/1.0 (Linux)"));
        assert!(!is_lightpanda_product("Chrome/148.0.0.0"));
    }

    #[test]
    fn parses_browser_label_from_version_body() {
        let body = r#"{"Browser":"Lightpanda/1.0","webSocketDebuggerUrl":"ws://127.0.0.1:9222/"}"#;
        assert_eq!(
            browser_label_from_version_body(body).as_deref(),
            Some("Lightpanda/1.0")
        );
    }
}
