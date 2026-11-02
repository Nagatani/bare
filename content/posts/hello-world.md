---
title: Hello, World
date: 2026-10-01
tags: [bare, intro]
description: bare の紹介と、記事がどう表示されるかの見本です。
---

## bare へようこそ

bare は Rust の標準ライブラリだけで書かれた、最小構成の静的サイトジェネレータです。
Markdown を書いて `cargo run` するだけで、`dist/` に HTML が出力されます。

![content/ を bare が読み込み、dist/ に HTML を出力する流れ](/images/build-flow.svg)

## なぜ依存ゼロなのか

`Cargo.toml` の `[dependencies]` が空であることには、地味ですが確かな利点があります。

- ビルドが速く、`cargo build` が数秒で終わる
- サプライチェーンのリスクを考えなくてよい
- 何がどう動いているかを、すべて自分で説明できる

## できること

- 技術ブログに必要な Markdown を HTML に変換し、コードはビルド時に色付けする（[記事の書き方](writing-guide.html)）
- `bare check` で、内部リンク・`#見出し` へのリンク・画像の切れを検査する
- タグ別ページ・年別アーカイブ・ページ送り・目次・Atom フィード・サイトマップを自動で作る
- テーマとテンプレートで見た目を変える（[デザインを変える](customize-design.html)）

## 表示の見本

記事は次のように表示されます。書き方の一覧は [記事の書き方](writing-guide.html) にまとめています。

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

コードブロックは、ビルド時に色が付きます。たとえば bare は正規表現を使わず、
`split_once` のような文字列スライスの操作だけで Frontmatter を読んでいます。

```rust
use std::collections::HashMap;

fn parse(block: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in block.lines() {
        if let Some((key, value)) = line.split_once(':') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}
```

`Vec<String>` のような型名は、HTML のタグと間違えずにそのまま表示されます。
本文に書いたテンプレートのプレースホルダー（`{{title}}`）も置き換えられません。

```html
<h1>{{title}}</h1>
```

---

## はじめかた

[Nagatani/bare](https://github.com/Nagatani/bare) をフォークすれば、GitHub Pages ですぐに自分のブログを公開できます。
手順は [bare をフォークして自分のブログを始める](start-your-blog.html) にまとめました。

手元で試すだけなら、次のコマンドでプレビューできます。表示された URL をブラウザで開いてください。
原稿を保存すると自動で再ビルドされ、ブラウザも自動でリロードされます。

```sh
cargo run -- serve
```
