use web_readable::{ExtractOptions, extract_with_options};

#[test]
fn extracts_main_article_content() {
    let html = include_str!("fixtures/article_semantic.html");
    let result =
        extract_with_options(html, &ExtractOptions::default()).expect("extract should work");

    assert_eq!(result.title, "Rust Reader テスト記事");
    assert!(
        result
            .text_content
            .contains("本文候補として評価されるべき内容")
    );
    assert!(!result.content_html.contains("<nav"));
    assert!(!result.content_html.contains("<aside"));
    assert_eq!(result.metadata.byline.as_deref(), Some("Example Author"));
    assert_eq!(result.metadata.site_name.as_deref(), Some("Example News"));
}

#[test]
fn downranks_link_heavy_sidebar() {
    let html = include_str!("fixtures/noise_links.html");
    let options = ExtractOptions {
        min_output_text: 80,
        ..ExtractOptions::default()
    };
    let result = extract_with_options(html, &options).expect("extract should work");

    assert!(result.text_content.contains("実験用の本文です"));
    assert!(!result.text_content.contains("A B C D E F"));
}

#[test]
fn can_exclude_images() {
    let html = include_str!("fixtures/article_semantic.html");
    let options = ExtractOptions {
        include_images: false,
        ..ExtractOptions::default()
    };

    let result = extract_with_options(html, &options).expect("extract should work");
    assert!(!result.content_html.contains("<img"));
}

#[test]
fn extracts_impress_news_article_without_promotional_noise() {
    let html = include_str!("fixtures/impress_news_2110950.html");
    let result =
        extract_with_options(html, &ExtractOptions::default()).expect("extract should work");

    assert!(
        result
            .text_content
            .contains("「Microsoft Edge」v148.0.3967.83")
    );
    assert!(result.text_content.contains("v148.0.3967.83"));
    assert!(result.text_content.contains("2026年5月22日 16:49"));
    assert!(result.text_content.contains("CVE-2026-9110"));
    assert!(result.text_content.contains("CVE-2026-9126"));
    assert!(!result.text_content.contains("窓の杜から"));
    assert!(!result.text_content.contains("Amazonで購入"));
    assert!(!result.text_content.contains("「Microsoft Edge」関連商品"));
    assert!(!result.text_content.contains("ニュース"));
    assert!(!result.text_content.contains("樽井 秀人"));
}
