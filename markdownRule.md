# markdownRule

HTML フラグメントは CommonMark / GFM に近い形へ変換する。

## ブロック要素

| 要素 | 出力 |
| --- | --- |
| `h1`-`h6` | `#` 見出し |
| `p` | 段落 |
| `blockquote` | `>` 引用 |
| `ul` / `ol` / `li` | 箇条書き / 番号付きリスト |
| `pre` | fenced code block |
| `table` | テーブル |
| `figure` / `section` / `article` / `main` / `div` / `body` | 子要素を再帰的に結合 |
| `math` | TeX 風の数式 |

## インライン要素

- `a` は `[text](href)`
- `strong` / `b` は `**text**`
- `em` / `i` / `cite` は `*text*`
- `code` はバッククォートで囲む
- `img` は `![alt](src "title")`
- `decode_embeds_as_urls` を有効にすると、`twitter-tweet` や link card 系の埋め込みを URL 文字列に展開する
- `span` / `small` / `sup` / `sub` / `mark` / `del` / `ins` / `u` / `q` / `time` / `abbr` は中身をそのまま通す
- それ以外は、子要素の再帰変換かテキスト収集にフォールバックする

## 数式

- MathML を TeX 風表記に変換する
- `math display="block"` は `$$ ... $$`
- `mi` / `mn` / `mtext` / `mo` などを組み合わせて出力する
- `mfrac` / `msup` / `msub` / `msubsup` / `msqrt` / `mroot` / `mfenced` / `munderover` / `munder` / `mover` / `mtable` も扱う

## 安全性と正規化

- `script` / `style` / `noscript` / `template` は落とす
- `href` / `src` / `srcset` は `javascript:` と `data:text/html` を拒否する
- 属性はタグごとに安全なものだけ残す
- 余分な空白と空行は最後に正規化する
- 連続するコードフェンス長は内容に応じて伸ばす

## 補足

- 抽出済みの `content_html` を Markdown に変換する前提
- 画像を落としたい場合は `ExtractOptions::include_images = false` を使う
