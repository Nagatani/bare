---
title: Hello, World
date: 2026-10-01
tags: [bare, intro]
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
| リスト | `- item` / `1. item` | 箇条書き・番号付き |
| 引用 | `> text` | 下を参照 |
| 表 | `\| a \| b \|` | この表 |

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
