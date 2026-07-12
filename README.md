# web-readable

`web-readable` は、Web ページから本文本体を抽出して Markdown に変換する Rust ライブラリです。  
現時点では以下が実装済みです。

- `html5ever` による HTML 解析
- HTML5 のセマンティクスを重視したスコアリング抽出
- `main` / `article` / `section` などを優先する候補選定
- 低品質なナビゲーション・サイドバー・リンク集の抑制
- CommonMark / GFM 系の Markdown への変換サブセット
- MathML から TeX への変換
- `chromiumoxide` を使った動的取得（`dynamic` feature）
- 既存 CDP endpoint への接続によるレンダリング HTML 取得
- Lightpanda endpoint 検出時の `lightpanda fetch` フォールバックによる安定取得

## Node.js / npm

```bash
npm install @web-readable/core
```

```js
const { extract, extractToMarkdown, htmlFragmentToMarkdown } = require('@web-readable/core')

const article = extract('<main><article><h1>タイトル</h1><p>本文です。</p></article></main>')
console.log(article.text_content)
console.log(extractToMarkdown('<article><p>Hello <strong>world</strong></p></article>'))
console.log(htmlFragmentToMarkdown('<p>Hello <strong>world</strong></p>'))
```

## 現在の公開API

- `extract(html)`  
  静的 HTML を抽出
- `extract_with_options(html, &ExtractOptions)`  
  オプション付きで静的 HTML を抽出
- `html_fragment_to_markdown(html)`  
  HTML フラグメントを Markdown に変換
- `html_fragment_to_markdown_with_options(html, &MarkdownOptions)`  
  埋め込みカードを URL に展開しつつ Markdown に変換
- `extract_to_markdown(html)` / `extract_to_markdown_with_options(html, &ExtractOptions)`  
  抽出結果を Markdown で取得
- `extract_to_markdown_with_markdown_options(html, &ExtractOptions, &MarkdownOptions)`  
  抽出と Markdown 変換の両方にオプションを指定
- `extract_from_url(url, &DynamicOptions, &ExtractOptions)`  
  CDP 経由で取得したページを抽出（`dynamic` feature 必須、`merge_paginated_content` で次ページ統合）
- `extract_to_markdown_from_url(url, &DynamicOptions, &ExtractOptions)`  
  CDP 経由で取得したページを Markdown で取得（`dynamic` feature 必須）
- `extract_to_markdown_from_url_with_markdown_options(url, &DynamicOptions, &ExtractOptions, &MarkdownOptions)`  
  CDP 取得 + Markdown 埋め込み展開

返却値は `ExtractedContent` で、主に以下を含みます。

- `title`
- `content_html`
- `text_content`
- `length`
- `score`
- `metadata`（`byline`, `excerpt`, `site_name`, `lang`, `published_time`）

## インストール

```toml
[dependencies]
web_readable = "0.1.0"
```

動的取得を使う場合:

```toml
[dependencies]
web_readable = { version = "0.1.0", features = ["dynamic"] }
```

## 静的HTMLの抽出

```rust
use web_readable::extract;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let html = r#"
        <html>
          <body>
            <main>
              <article>
                <h1>記事タイトル</h1>
                <p>本文です。</p>
              </article>
            </main>
          </body>
        </html>
    "#;

    let article = extract(html)?;
    println!("{}", article.text_content);
    Ok(())
}
```

## オプション付きの抽出

```rust
use web_readable::{extract_with_options, ExtractOptions};

let options = ExtractOptions {
    include_images: true,
    merge_paginated_content: true,
    ..ExtractOptions::default()
};

let article = extract_with_options(html, &options)?;
```

```rust
use web_readable::{html_fragment_to_markdown_with_options, MarkdownOptions};

let markdown = html_fragment_to_markdown_with_options(
    html,
    &MarkdownOptions {
        decode_embeds_as_urls: true,
    },
);
```

## 動的ページの抽出

このモードは、すでに起動済みの Chromium / Chrome の CDP endpoint に接続します。  
Lightpanda endpoint を使う場合は自動で `lightpanda fetch` に切り替わります。

```rust
use web_readable::{extract_from_url, DynamicOptions, ExtractOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dynamic = DynamicOptions::new("ws://127.0.0.1:9222/devtools/browser/<id>");
    let article = extract_from_url(
        "https://example.com/article",
        &dynamic,
        &ExtractOptions::default(),
    )
    .await?;

    println!("{}", article.text_content);
    Ok(())
}
```

Lightpanda を使うときは、別プロセスで CDP サーバーを立てます。

```bash
lightpanda serve --host 127.0.0.1 --port 9222
cargo run --features dynamic --bin web_readable -- https://example.com 127.0.0.1:9222
```

## Markdown 変換

```rust
use web_readable::html_fragment_to_markdown;

let markdown = html_fragment_to_markdown(r#"<p>Hello <strong>world</strong></p>"#);
```

## CLI

簡易 CLI は `web_readable` バイナリとして用意しています。第一引数に URL、第二引数に CDP endpoint、第三引数に期待 Markdown ファイルを渡せます。

```bash
cargo run --features dynamic --bin web_readable -- https://example.com 127.0.0.1:9222
cargo run --features dynamic --bin web_readable -- https://example.com 127.0.0.1:9222 expected.md
```

## ルール詳細

- [contentRule.md](contentRule.md)
- [markdownRule.md](markdownRule.md)

## 実装メモ

- 旧 Readability 実装は参考にしているが、古い class/id 依存のルールはそのまま使っていない
- スコアリングは HTML5 時代の構造を優先する
- `dynamic` feature は optional で、通常利用時の依存を軽く保つ
