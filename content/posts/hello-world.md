---
title: Hello, World
date: 2026-10-01
tags: [bare, intro]
---

## bare へようこそ

bare は Rust の標準ライブラリだけで書かれた、最小構成の静的サイトジェネレータです。
Markdown を書いて `cargo run` するだけで、`dist/` に HTML が出力されます。

## できること

- Frontmatter（`title`, `date`, `tags`）の解析
- 見出し・段落・リスト・リンク・インラインコードの変換
- フェンス付きコードブロックと HTML エスケープ
- `{{title}}` 形式のプレースホルダーによるテンプレート置換

## はじめかた

[Nagatani/bare](https://github.com/Nagatani/bare) をフォークすれば、GitHub Pages ですぐに自分のブログを公開できます。
手順は [bare をフォークして自分のブログを始める](start-your-blog.html) にまとめました。

手元で試すだけなら、次の 2 行で十分です。

```sh
cargo run
open dist/index.html
```
