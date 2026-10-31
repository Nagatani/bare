# bare

A zero-dependency, minimalist static site generator written in pure Rust.

`Cargo.toml` の `[dependencies]` は空で、Rust 標準ライブラリ（`std`）だけで動作します。

## 自分のブログを始める（フォーク）

このリポジトリはフォークしてそのまま自分のブログにすることを想定しています。
Rust のインストールは不要で、ビルドと公開は GitHub Actions が行います。

1. 右上の **Fork** からフォークする（リポジトリ名を `<ユーザー名>.github.io` にすると URL がルートになります）
2. フォーク先の **Actions** タブで、ワークフローを有効にする（フォーク直後は無効になっています）
3. **Settings → Pages → Build and deployment → Source** を **GitHub Actions** にする
4. `content/site.conf` のサイト名などを書き換えて `main` にコミットする → 自動で公開されます
5. `content/posts/` に記事を追加し、不要なサンプル記事を削除する

詳しい手順と本家の更新の取り込み方は、サンプル記事
[`content/posts/start-your-blog.md`](content/posts/start-your-blog.md) にまとめています。

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
  wrote dist/posts/start-your-blog.html
  wrote dist/posts/rust-without-crates.html
  wrote dist/posts/hello-world.html
  wrote dist/index.html
bare: generated 4 files (3 posts + index) in 2 ms
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
  site.conf     サイト名・説明・著者・言語の設定
  posts/        記事（*.md。0 本でもビルド可）
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

`content/site.conf` の各キーは、すべてのテンプレートで `site_` を付けた名前で使えます。

| キー | トークン | 既定値 |
| --- | --- | --- |
| `title` | `{{site_title}}` | `bare` |
| `description` | `{{site_description}}` | （空） |
| `author` | `{{site_author}}` | （空） |
| `lang` | `{{site_lang}}` | `ja` |

テンプレートごとに使えるトークンは次のとおりです。

| ファイル | 使えるトークン |
| --- | --- |
| `layout.html` | `{{title}}`, `{{content}}`, `{{root}}`（`index.html` へ戻る相対パス） |
| `post.html` | `{{title}}`, `{{date}}`, `{{tags}}`, `{{content}}` |
| `index.html` | `{{post_list}}`, `{{post_count}}` |

未定義のトークンはそのまま出力されるので、タイプミスに気づけます。本文中に書かれた `{{...}}` は置換されません。
