# bare

A zero-dependency, minimalist static site generator written in pure Rust.

`Cargo.toml` の `[dependencies]` は空で、Rust 標準ライブラリ（`std`）だけで動作します。

デモ: <https://nagatani.github.io/bare/>

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

# 下書き（draft: true）も出力して確認する
cargo run -- --drafts

# 公開 URL を指定して feed.xml / sitemap.xml / OGP の絶対 URL も生成する
cargo run -- --base-url https://<user>.github.io/bare/

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
bare: generated 4 files (3 posts + index), copied 2 static files in 9 ms
```

生成後は `dist/index.html` をブラウザで開いてください（相対リンクなので `file://` でも閲覧できます）。

通常のビルドでは `dist/posts/` だけを作り直し、`dist/` 直下に置いた他のファイルは残します。
`--clean` を付けると `dist/` 全体を削除してから生成します。ただし、作業ディレクトリや
`content/` を含むディレクトリ（`--out .` など）は安全のため削除を拒否します。

`content/static/` の中身（画像・favicon・`CNAME` など）は、ディレクトリ構造を保ったまま `dist/` にコピーされます。

## GitHub Pages で公開する

`.github/workflows/pages.yml` が `main` への push ごとに `cargo test` → `bare --clean` を実行し、
`dist/` を GitHub Pages にデプロイします（Actions タブから手動実行も可能）。
Pages が未設定のまま実行された場合は、最初のステップで設定方法を示すエラーを出して止まります。

初回のみ、リポジトリの **Settings → Pages → Build and deployment → Source** を
**GitHub Actions** に設定してください。公開 URL は `https://<user>.github.io/<repo>/` です。
リンクはすべて相対パスなので、サブパス配下でもそのまま動作します。

ワークフローは `configure-pages` が返す公開 URL（独自ドメインを含む）を `--base-url` で渡します。
そのため Actions でのビルドでは、`feed.xml`・`sitemap.xml`・OGP の絶対 URL が設定なしで生成されます。
手元のビルドで公開 URL が分からない場合、これらは生成されません（ログに note が表示されます）。

### 生成されるファイル

| ファイル | 内容 |
| --- | --- |
| `index.html` | 記事一覧（新着順） |
| `posts/<ファイル名>.html` | 各記事 |
| `404.html` | 存在しない URL 用のページ（`templates/404.html` がある場合） |
| `feed.xml` | Atom フィード（最新 20 件。公開 URL が分かる場合） |
| `sitemap.xml` | サイトマップ（公開 URL が分かる場合） |
| その他 | `content/static/` からのコピー |

## 本家の更新を取り込む（フォークした人向け）

GitHub の **Sync fork** か、`git fetch upstream && git merge upstream/main` で取り込みます。
変更内容とテンプレートに必要な差分は [CHANGELOG.md](CHANGELOG.md) にまとめています。

衝突しやすいのは、自分で編集・削除したサンプル記事、`content/site.conf`、`content/templates/` です。

- 削除したサンプル記事との衝突（modify/delete）→ `git rm content/posts/<ファイル>` で削除したままにする
- `site.conf` やテンプレートとの衝突 → 自分の版を残し、CHANGELOG の「テンプレートの変更」を手で反映する

### 本家をメンテナンスするときの方針

フォークを壊さないため、次のことを守ります。

- 古いテンプレートのままでもビルドが通るようにする（新しいテンプレートは任意にし、新しいトークンは書かれていなければ出力しないだけにする）
- `site.conf` の新しいキーには必ず既定値を持たせる
- サンプル記事と `site.conf` の変更は最小限にする（フォーク側で消されたり書き換えられたりしている前提）
- テンプレートを変えたら CHANGELOG の「テンプレートの変更」に差分を書く

## ディレクトリ構成

```text
src/
  main.rs       CLI エントリポイント（引数パース・実行フロー・ログ出力）
  parser.rs     Frontmatter と簡易 Markdown パーサー
  template.rs   {{placeholder}} 置換エンジン
  site.rs       記事探索・HTML 生成・dist/ 出力
  feed.rs       Atom フィードと sitemap の生成
content/
  site.conf     サイト名・説明・著者・言語・公開 URL などの設定
  posts/        記事（*.md。0 本でもビルド可）
  templates/    layout.html / post.html / index.html / 404.html（任意）
  static/       dist/ にそのままコピーされるファイル（画像・favicon など）
dist/           生成物（git 管理外）
.github/workflows/pages.yml   GitHub Pages へのデプロイ
```

## 記事の書き方

```markdown
---
title: 記事タイトル
date: 2026-10-31
tags: [rust, ssg]
description: 一覧・検索結果・SNS・RSS に表示される説明（任意）
image: /images/cover.png
draft: true
---

## 見出し

本文。`inline code` や [リンク](https://example.com) が使えます。

- リスト項目
```

- `date` は `YYYY-MM-DD` 形式で書くと、一覧が新着順に正しく並びます。
- `tags` は `[a, b]` と `a, b` のどちらの書き方でも構いません。
- `image` は SNS で共有したときのプレビュー画像です（任意。`content/static/` に置いた画像を指定）。
- `draft: true` の記事は出力されません（`--drafts` で出力）。
- ファイル名がそのまま URL になるので、英数字とハイフンにしておくのがおすすめです。

### 対応する Markdown

| 記法 | 出力 |
| --- | --- |
| `#` 〜 `######` | `<h1>` 〜 `<h6>` |
| 空行区切りのテキスト | `<p>`（日本語の行末は空白を入れずに連結） |
| ```` ```lang ```` 〜 ```` ``` ```` | `<pre><code class="language-lang">`（HTML エスケープ済み。4 つ以上のバッククォートで囲めば中に ```` ``` ```` を書ける） |
| `` `code` `` / ``` `` a`b `` ``` | `<code>` |
| `- item` / `* item` / `+ item` の連続行 | `<ul><li>` |
| `1. item` / `1) item` の連続行 | `<ol><li>`（開始番号を引き継ぐ） |
| `> text` | `<blockquote>`（中にリストなども書ける） |
| `\| a \| b \|` + `\| --- \| :---: \|` | `<table>`（`:` で左・中央・右寄せ） |
| `---` / `***` / `___` | `<hr>` |
| `[text](url "title")` | `<a href="url">` |
| `![alt](url "title")` | `<img src="url" alt="alt">` |
| `**strong**` / `*em*` / `~~del~~` | `<strong>` / `<em>` / `<del>` |
| `\*` など | 記号をそのまま表示 |

`/images/a.png` のように `/` で始まる URL は、ページからの相対パスに変換されます。
GitHub Pages のサブパス（`https://<user>.github.io/<repo>/`）で公開しても画像やリンクが切れません。

未対応: 入れ子のリスト、リスト項目の複数行、生の HTML（エスケープされて表示されます）。

## テンプレート

`content/site.conf` の各キーは、すべてのテンプレートで `site_` を付けた名前で使えます。

| キー | トークン | 既定値 |
| --- | --- | --- |
| `title` | `{{site_title}}` | `bare` |
| `description` | `{{site_description}}` | （空） |
| `author` | `{{site_author}}` | （空） |
| `lang` | `{{site_lang}}` | `ja` |
| `base_url` | `{{site_url}}` | （空。CI では自動） |
| `image` | — | （空）。OGP の既定画像 |

テンプレートごとに使えるトークンは次のとおりです。

| ファイル | 使えるトークン |
| --- | --- |
| `layout.html` | `{{page_title}}`（`<title>` 用）, `{{title}}`, `{{meta}}`（description・OGP・canonical・フィードの `<meta>`/`<link>`）, `{{content}}`, `{{root}}`（サイトのルートへの相対パス） |
| `post.html` | `{{title}}`, `{{date}}`, `{{description}}`, `{{tags}}`, `{{content}}` |
| `index.html` | `{{post_list}}`, `{{post_count}}` |
| `404.html` | `{{root}}`（`/bare/` のようなルートからの絶対パス） |

未定義のトークンはそのまま出力されるので、タイプミスに気づけます。本文中に書かれた `{{...}}` は置換されません。
