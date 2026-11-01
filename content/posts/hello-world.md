---
title: Hello, World
date: 2026-10-01
tags: [bare, intro]
description: bare の紹介と、対応している Markdown 記法の一覧です。
---

## bare へようこそ

bare は Rust の標準ライブラリだけで書かれた、最小構成の静的サイトジェネレータです。
Markdown を書いて `cargo run` するだけで、`dist/` に HTML が出力されます。

![content/ を bare が読み込み、dist/ に HTML を出力する流れ](/images/build-flow.svg)

## できること

- Frontmatter（`title`, `date`, `tags`）の解析
- 技術ブログに必要な Markdown 記法の変換（下の表を参照）
- `content/static/` に置いた画像などのファイルを `dist/` へそのままコピー
- `{{title}}` 形式のプレースホルダーによるテンプレート置換

## 対応している Markdown

| 記法 | 書き方 | 表示 |
| --- | --- | --- |
| 強調 | `**太字**` / `*斜体*` / `~~取り消し~~` | **太字** / *斜体* / ~~取り消し~~ |
| コード | `` `code` `` | `code` |
| リンク | `[text](url)` | [bare](https://github.com/Nagatani/bare) |
| 画像 | `![alt](/images/a.png)` | 上の図 |
| リスト | `- item` / `1. item`（字下げで入れ子） | 下を参照 |
| タスク | `- [x] 済` / `- [ ] 未` | 下を参照 |
| 脚注 | `本文[^1]` と `[^1]: 注釈` | 下を参照[^footnote] |
| 引用 | `> text` | 下を参照 |
| 表 | `\| a \| b \|` | この表 |
| HTML | `<kbd>Ctrl</kbd>` / `<ruby>漢字<rt>かんじ</rt></ruby>` | <kbd>Ctrl</kbd> / <ruby>漢字<rt>かんじ</rt></ruby> |

リストは 2 文字以上字下げすると入れ子になり、項目の中にコードブロックも書けます。

- 記事を書く
  - [x] `content/posts/` に Markdown を置く
  - [ ] `draft: true` を消して公開する
- 確認する
  1. `cargo run -- serve` でプレビュー
  2. `cargo run -- check` でリンク切れを検査

     ```sh
     cargo run -- check
     ```

[^footnote]: 脚注は番号が自動で振られ、記事の末尾にまとめて表示されます。

Markdown で書けないものは HTML で書けます。行頭から `<details>` や `<div>` などで始めたブロックは、
空行までそのまま出力されます（中の Markdown は変換されません）。

<details>
<summary>クリックで開く</summary>
<p>HTML のブロックは Markdown を通さず、そのまま出力されます。</p>
</details>

> 画像のパスは `/images/...` のようにサイトのルートから書けます。
> GitHub Pages のサブパス（`/bare/` など）で公開しても、自動で正しい相対パスに変換されます。

---

## はじめかた

[Nagatani/bare](https://github.com/Nagatani/bare) をフォークすれば、GitHub Pages ですぐに自分のブログを公開できます。
手順は [bare をフォークして自分のブログを始める](start-your-blog.html) にまとめました。

手元で試すだけなら、次の 2 行で十分です。

```sh
cargo run
open dist/index.html
```
