# contentRule

本文抽出は「HTML の構造」と「テキスト量」の両方を見る。

## 候補選定

- `script` / `style` / `noscript` / `template` / `nav` / `aside` / `footer` / `header` / `button` / `input` / `select` / `textarea` / `form` は候補から外す
- まず `article` / `main` / `section` / `div` / `p` / `body` を優先する
- それ以外は `role="main"` / `role="article"` を持つ要素だけを候補にする
- `min_candidate_text` 未満のノードは候補にしない

## スコア

スコアはおおむね以下の合算で決める。

- タグの種類
  - `main` +38
  - `article` +34
  - `section` +16
  - `div` +8
  - `p` +10
  - `body` +6
  - `figure` -6
  - `header` / `footer` / `aside` / `nav` -42
  - `ul` / `ol` -18
  - `form` -36
- `role`
  - `main` / `article` +20
  - `navigation` / `complementary` / `banner` / `contentinfo` / `menu` -28
- `class` / `id`
  - `article`, `content`, `main`, `post`, `entry`, `story`, `prose`, `markdown`, `read`, `body` を含むと加点
  - `comment`, `footer`, `header`, `nav`, `menu`, `sidebar`, `share`, `related`, `advert`, `ads`, `promo`, `breadcrumb`, `social`, `pagination`, `widget` を含むと減点
- テキスト量
  - 文字数の平方根に比例して加点
- 段落数
  - `p` / `pre` / `blockquote` が多いほど加点
- 記号量
  - 句読点が多いほど少し加点
- メディア
  - 画像などが多く、短文なら減点
- リンク密度
  - リンクが多いほど減点
  - 記事見出し周辺の `corner` / `author` / `download` / `affiliate` / `amazon` / `social` / `bookmark` 系の誘導ブロックは Markdown 化の前に落とす

## 閾値

- `min_candidate_text`: 120
- `min_output_text`: 200
- `sibling_score_ratio`: 0.22
- リンクが強いサイドバーは強めに減点する
- 段落がなく見出しばかりの小さいブロックは候補から落とす

## 兄弟要素の扱い

- 最上位候補の親要素にぶら下がる兄弟を、一定比率以上なら一緒に採用する
- `p` で十分長く、リンク密度が低いものは補助的に採用する
- 長文かつリンク密度が低い要素も補助的に採用する

## 仕上げ

- hidden 要素や `display:none` / `visibility:hidden` / `opacity:0` は除外する
- 最終出力が `min_output_text` 未満ならエラーにする
- 候補が無ければ `body` → `main` → `article` の順でフォールバックする

## 動的取得との関係

動的取得で得た HTML でも、このルールは同じように適用される。
Lightpanda endpoint の場合は、取得段階で `lightpanda fetch` に切り替えて安定化する。
`merge_paginated_content` を有効にすると、`rel="next"` や pagination 系リンクをたどって本文を順番に統合する。
