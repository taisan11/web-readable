use web_readable::{
    ExtractOptions, MarkdownOptions, extract_to_markdown_with_markdown_options,
    extract_to_markdown_with_options, html_fragment_to_markdown,
    html_fragment_to_markdown_with_options,
};

#[test]
fn converts_html_fragment_to_commonmark_like_markdown() {
    let html = include_str!("fixtures/markdown_mathml.html");
    let markdown = html_fragment_to_markdown(html);

    assert!(markdown.contains("# Markdown 変換テスト"));
    assert!(markdown.contains("**強調**"));
    assert!(markdown.contains("*斜体*"));
    assert!(markdown.contains("[リンク](https://example.com)"));
    assert!(markdown.contains("$x + y$"));
    assert!(markdown.contains("$$"));
    assert!(markdown.contains("- ひとつめ"));
    assert!(markdown.contains("```"));
}

#[test]
fn extracts_article_content_as_markdown() {
    let html = include_str!("fixtures/article_semantic.html");
    let markdown = extract_to_markdown_with_options(html, &ExtractOptions::default())
        .expect("extract should work");

    assert!(markdown.contains("# Rust で本文抽出を実装する"));
    assert!(markdown.contains("本文です。"));
    assert!(!markdown.contains("関連記事"));
}

#[test]
fn extracts_impress_news_article_like_markdown() {
    let html = include_str!("fixtures/impress_news_2110950.html");
    let markdown = extract_to_markdown_with_options(html, &ExtractOptions::default())
        .expect("extract should work");

    assert!(
        markdown
            .contains("# 「Microsoft Edge」に16件の脆弱性、深刻度「Critical」の致命的なものも2件")
    );
    assert!(markdown.contains("「Microsoft Edge」v148.0.3967.83"));
    assert!(markdown.contains("2026年5月22日 16:49"));
    assert!(markdown.contains("「Microsoft Edge」v148.0.3967.83"));
    assert!(markdown.contains("CVE-2026-9110"));
    assert!(markdown.contains("CVE-2026-9126"));
    assert!(!markdown.contains("窓の杜から"));
    assert!(!markdown.contains("Amazonで購入"));
    assert!(!markdown.contains("「Microsoft Edge」関連商品"));
    assert!(!markdown.contains("ニュース"));
    assert!(!markdown.contains("樽井 秀人"));
}

#[test]
fn decodes_embed_cards_to_urls_when_requested() {
    let html = r#"
        <blockquote class="twitter-tweet">
          <a href="https://x.com/example/status/123">tweet</a>
        </blockquote>
        <div class="link-card">
          <a href="https://example.com/cards/42">
            <span>Card title</span>
          </a>
        </div>
    "#;
    let markdown = html_fragment_to_markdown_with_options(
        html,
        &MarkdownOptions {
            decode_embeds_as_urls: true,
        },
    );

    assert!(markdown.contains("https://x.com/example/status/123"));
    assert!(markdown.contains("https://example.com/cards/42"));
    assert!(!markdown.contains("twitter-tweet"));
}

#[test]
fn extracts_zenn_style_iframe_embed_as_url() {
    let html = r#"
        <html>
          <body>
            <article>
              <p>本文
                <span class="embed-block zenn-embedded zenn-embedded-card">
                  <iframe data-content="https%3A%2F%2Fadventar.org%2Fcalendars%2F11697"></iframe>
                </span>
                続き
              </p>
            </article>
          </body>
        </html>
    "#;
    let markdown = extract_to_markdown_with_markdown_options(
        html,
        &ExtractOptions {
            min_candidate_text: 0,
            min_output_text: 0,
            ..ExtractOptions::default()
        },
        &MarkdownOptions {
            decode_embeds_as_urls: true,
        },
    )
    .expect("extract should work");

    assert!(markdown.contains("https://adventar.org/calendars/11697"));
}

#[test]
fn preserves_multiline_code_blocks_in_markdown_output() {
    let html = r#"
        <html>
          <body>
            <article>
              <p>Example</p>
              <pre><code>fn main() {
    println!("hello");
}</code></pre>
            </article>
          </body>
        </html>
    "#;

    let markdown = extract_to_markdown_with_options(
        html,
        &ExtractOptions {
            min_candidate_text: 0,
            min_output_text: 0,
            ..ExtractOptions::default()
        },
    )
    .expect("extract should work");

    assert!(markdown.contains("```"));
    assert!(markdown.contains("fn main() {\n    println!(\"hello\");\n}"));
}

#[test]
fn strips_gigazine_style_boilerplate_from_article_markdown() {
    let html = include_str!("fixtures/gigazine_like_article.html");
    let markdown = extract_to_markdown_with_markdown_options(
        html,
        &ExtractOptions {
            min_candidate_text: 0,
            min_output_text: 0,
            ..ExtractOptions::default()
        },
        &MarkdownOptions {
            decode_embeds_as_urls: true,
        },
    )
    .expect("extract should work");

    assert!(
        markdown
            .contains("# 漏洩したGoogle APIキーは削除後も約23分間使える可能性があると研究者が確認")
    );
    assert!(markdown.contains(r"[!\[\](https://i.gzn.jp/img/2026/05/22/google-api-key-remain-live/00_m.png)](https://i.gzn.jp/img/2026/05/22/google-api-key-remain-live/00.png)"));
    assert!(markdown.contains(
        "Google API keys keep working after you delete them long enough to be exploited"
    ));
    assert!(markdown.contains("https://www.youtube.com/watch?v=CJd6AkOaiJo"));
    assert!(!markdown.contains("2026年05月22日 20時00分"));
    assert!(!markdown.contains("[セキュリティ](/news/C14/)"));
    assert!(!markdown.contains("この記事のタイトルとURLをコピーする"));
    assert!(!markdown.contains("関連記事"));
    assert!(!markdown.contains("関連コンテンツ"));
}
