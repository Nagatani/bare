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

```sh
cargo run
open dist/index.html
```

ソースは [GitHub](https://github.com/) に置いて、`dist/` をそのまま静的ホスティングに載せれば公開完了です。
