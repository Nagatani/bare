# bare

A zero-dependency, minimalist static site generator written in pure Rust.

`Cargo.toml` の `[dependencies]` は空で、Rust 標準ライブラリ（`std`）だけで動作します。

## 使い方

```sh
# ビルドして content/ から dist/ を生成
cargo run

# リリースビルドのバイナリを使う
cargo build --release
./target/release/bare

# 入出力ディレクトリを指定
cargo run -- build --content content --out dist

# dist/ を丸ごと削除してから作り直す
cargo run -- --clean

# テスト
cargo test
```

実行例:

```text
bare: building content -> dist
  wrote dist/posts/rust-without-crates.html
  wrote dist/posts/hello-world.html
  wrote dist/index.html
bare: generated 3 files (2 posts + index) in 1 ms
```

生成後は `dist/index.html` をブラウザで開いてください（相対リンクなので `file://` でも閲覧できます）。

通常のビルドでは `dist/posts/` だけを作り直し、`dist/` 直下に置いた他のファイルは残します。
`--clean` を付けると `dist/` 全体を削除してから生成します。ただし、作業ディレクトリや
`content/` を含むディレクトリ（`--out .` など）は安全のため削除を拒否します。

## GitHub Pages で公開する

`.github/workflows/pages.yml` が `main` への push ごとに `cargo test` → `bare --clean` を実行し、
`dist/` を GitHub Pages にデプロイします（Actions タブから手動実行も可能）。

初回のみ、リポジトリの **Settings → Pages → Build and deployment → Source** を
**GitHub Actions** に設定してください。公開 URL は `https://<user>.github.io/<repo>/` です。
リンクはすべて相対パスなので、サブパス配下でもそのまま動作します。

## ディレクトリ構成

```text
src/
  main.rs       CLI エントリポイント（引数パース・実行フロー・ログ出力）
  parser.rs     Frontmatter と簡易 Markdown パーサー
  template.rs   {{placeholder}} 置換エンジン
  site.rs       記事探索・HTML 生成・dist/ 出力
content/
  posts/        記事（*.md）
  templates/    layout.html / post.html / index.html
dist/           生成物（git 管理外）
.github/workflows/pages.yml   GitHub Pages へのデプロイ
```

## 記事の書き方

```markdown
---
title: 記事タイトル
date: 2026-10-31
tags: [rust, ssg]
---

## 見出し

本文。`inline code` や [リンク](https://example.com) が使えます。

- リスト項目
```

- `date` は `YYYY-MM-DD` 形式で書くと、一覧が新着順に正しく並びます。
- `tags` は `[a, b]` と `a, b` のどちらの書き方でも構いません。

### 対応する Markdown

| 記法 | 出力 |
| --- | --- |
| `#` 〜 `######` | `<h1>` 〜 `<h6>` |
| ```` ```lang ```` 〜 ```` ``` ```` | `<pre><code class="language-lang">`（HTML エスケープ済み） |
| `` `code` `` | `<code>` |
| `- item` の連続行 | `<ul><li>` |
| 空行区切りのテキスト | `<p>` |
| `[text](url)` | `<a href="url">` |

## テンプレート

| ファイル | 使えるトークン |
| --- | --- |
| `layout.html` | `{{title}}`, `{{content}}`, `{{root}}`（`index.html` へ戻る相対パス） |
| `post.html` | `{{title}}`, `{{date}}`, `{{tags}}`, `{{content}}` |
| `index.html` | `{{post_list}}`, `{{post_count}}` |

未定義のトークンはそのまま出力されるので、タイプミスに気づけます。本文中に書かれた `{{...}}` は置換されません。
