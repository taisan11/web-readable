use std::process::ExitCode;
#[cfg(feature = "dynamic")]
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
};

#[cfg(feature = "dynamic")]
use web_readable::{
    DynamicOptions, ExtractOptions, MarkdownOptions,
    extract_to_markdown_from_url_with_markdown_options,
};

#[cfg(feature = "dynamic")]
fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(url) = args.next() else {
        eprintln!("usage: web_readable <url> <cdp-endpoint> [expected-markdown-file]");
        return ExitCode::from(1);
    };
    let Some(cdp_endpoint) = args.next() else {
        eprintln!("usage: web_readable <url> <cdp-endpoint> [expected-markdown-file]");
        return ExitCode::from(1);
    };
    let expected_markdown_file = args.next();
    if args.next().is_some() {
        eprintln!("usage: web_readable <url> <cdp-endpoint> [expected-markdown-file]");
        return ExitCode::from(1);
    }

    let cdp_endpoint = match resolve_cdp_endpoint(&cdp_endpoint) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("failed to initialize runtime: {err}");
            return ExitCode::from(1);
        }
    };

    let result = runtime.block_on(async {
        let dynamic = DynamicOptions::new(cdp_endpoint);
        extract_to_markdown_from_url_with_markdown_options(
            &url,
            &dynamic,
            &ExtractOptions::default(),
            &MarkdownOptions {
                decode_embeds_as_urls: true,
            },
        )
        .await
    });

    match result {
        Ok(markdown) => {
            if let Some(expected_markdown_file) = expected_markdown_file {
                match compare_markdown_with_file(&markdown, &expected_markdown_file) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(err) => {
                        eprintln!("{err}");
                        ExitCode::from(1)
                    }
                }
            } else {
                println!("{markdown}");
                ExitCode::SUCCESS
            }
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

#[cfg(feature = "dynamic")]
fn resolve_cdp_endpoint(input: &str) -> std::result::Result<String, String> {
    if input.starts_with("ws://") || input.starts_with("wss://") {
        return Ok(input.to_string());
    }

    let address = input
        .strip_prefix("http://")
        .or_else(|| input.strip_prefix("https://"))
        .unwrap_or(input);

    let mut stream = TcpStream::connect(address)
        .map_err(|err| format!("failed to connect to CDP server `{address}`: {err}"))?;
    stream
        .write_all(
            format!("GET /json/version HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .map_err(|err| format!("failed to query CDP server `{address}`: {err}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|err| format!("failed to read CDP response from `{address}`: {err}"))?;

    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .ok_or_else(|| format!("invalid CDP response from `{address}`"))?;

    json_string_field(body, "webSocketDebuggerUrl").ok_or_else(|| {
        format!("CDP server `{address}` did not return webSocketDebuggerUrl in /json/version")
    })
}

#[cfg(feature = "dynamic")]
fn json_string_field(body: &str, key: &str) -> Option<String> {
    let key = format!("\"{key}\"");
    let start = body.find(&key)? + key.len();
    let after_colon = body[start..].find(':')? + start + 1;
    let value_start = body[after_colon..]
        .find('"')
        .map(|index| after_colon + index + 1)?;
    let value_end = body[value_start..].find('"')? + value_start;
    Some(body[value_start..value_end].replace("\\/", "/"))
}

#[cfg(feature = "dynamic")]
fn compare_markdown_with_file(markdown: &str, expected_markdown_file: &str) -> Result<(), String> {
    let expected = fs::read_to_string(expected_markdown_file).map_err(|err| {
        format!("failed to read expected markdown file `{expected_markdown_file}`: {err}")
    })?;

    let actual = normalize_markdown_for_compare(markdown);
    let expected = normalize_markdown_for_compare(&expected);

    if actual == expected {
        return Ok(());
    }

    let diff = summarize_markdown_difference(&actual, &expected);
    Err(format!(
        "markdown output did not match `{expected_markdown_file}`{diff}"
    ))
}

#[cfg(feature = "dynamic")]
fn normalize_markdown_for_compare(input: &str) -> String {
    input
        .replace("\r\n", "\n")
        .trim_end_matches('\n')
        .to_string()
}

#[cfg(feature = "dynamic")]
fn summarize_markdown_difference(actual: &str, expected: &str) -> String {
    let actual_lines: Vec<&str> = actual.lines().collect();
    let expected_lines: Vec<&str> = expected.lines().collect();
    let max_len = actual_lines.len().max(expected_lines.len());

    for index in 0..max_len {
        let actual_line = actual_lines.get(index).copied().unwrap_or("");
        let expected_line = expected_lines.get(index).copied().unwrap_or("");
        if actual_line != expected_line {
            return format!(
                "\nfirst difference at line {}:\n  expected: {}\n  actual:   {}",
                index + 1,
                expected_line,
                actual_line
            );
        }
    }

    String::from("\ncontent differs after normalization")
}

#[cfg(not(feature = "dynamic"))]
fn main() -> ExitCode {
    eprintln!("web_readable CLI requires the `dynamic` feature");
    ExitCode::from(1)
}

#[cfg(all(test, feature = "dynamic"))]
mod tests {
    use super::{compare_markdown_with_file, resolve_cdp_endpoint};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn leaves_websocket_endpoint_untouched() {
        let endpoint = "ws://127.0.0.1:9222/devtools/browser/test";
        assert_eq!(
            resolve_cdp_endpoint(endpoint).expect("ws endpoint should pass through"),
            endpoint
        );
    }

    #[test]
    fn compares_markdown_to_file_contents() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("web_readable_expected_{suffix}.md"));
        fs::write(&path, "hello\nworld\n").expect("write temp file");

        let ok = compare_markdown_with_file("hello\nworld", path.to_str().expect("utf8 path"));
        assert!(ok.is_ok());

        fs::write(&path, "hello\nmars\n").expect("overwrite temp file");
        let err = compare_markdown_with_file("hello\nworld", path.to_str().expect("utf8 path"));
        assert!(err.is_err());

        let _ = fs::remove_file(path);
    }
}
