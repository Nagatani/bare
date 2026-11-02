---
title: ビルドと公開のしくみ
date: 2026-11-02
tags: [bare, guide, github-pages]
description: コマンドとオプション、プレビューサーバー、生成されるファイル、GitHub Actions での公開。
---

## コマンド

| コマンド | 内容 |
| --- | --- |
| `bare` / `bare build` | `dist/` にサイトを生成。リンク切れなどは警告として表示 |
| `bare check` | ビルドしたうえで、リンク切れ・存在しないアンカー・画像があれば終了コード 1 |
| `bare serve` | プレビュー。保存すると再ビルドし、ブラウザも自動でリロード。下書きも表示 |
| `bare new <名前>` | 記事の下書きを作成。`--page` で固定ページ、`--theme` で既定のテーマのコピー |

手元では `cargo run -- serve` のように、`cargo run --` の後にコマンドを書きます。

| オプション | 内容 |
| --- | --- |
| `-c, --content <DIR>` | 原稿のディレクトリ（既定: `content`） |
| `-o, --out <DIR>` | 出力先（既定: `dist`、serve は `target/bare-serve`） |
| `--clean` | 出力先を丸ごと削除してから生成 |
| `--drafts` | `draft: true` の記事も出力 |
| `--base-url <URL>` | 公開 URL。`feed.xml`・`sitemap.xml`・OGP の絶対 URL に使う |
| `-p, --port <PORT>` | serve のポート（既定: 3000） |
| `--subpath <PATH>` | serve で配信するパス（既定: GitHub Pages と同じになるよう自動判定） |
| `--title <TITLE>` | `new` で作るファイルのタイトル |

## site.conf

| キー | 内容 | 既定値 |
| --- | --- | --- |
| `title` | サイト名 | `bare` |
| `description` | キャッチコピーと `<meta name="description">` | （空） |
| `author` | `<meta name="author">` | （空） |
| `lang` | `<html lang>` | `ja` |
| `base_url` | 公開 URL | （空。GitHub Actions では自動） |
| `image` | SNS で共有したときの既定のプレビュー画像 | （空） |
| `per_page` | 記事一覧の 1 ページあたりの件数 | （空。すべてを 1 ページに） |
| `theme` | 使うテーマ（`content/themes/` の下のディレクトリ名） | `default` |

## プレビュー（bare serve）

- 公開時と同じパスで配信します。git の `origin` が `…/bare.git` なら `/bare/`、
  `<user>.github.io` リポジトリや `content/static/CNAME` があれば `/` です。
- GitHub Pages と同じく、`/posts/a` は `posts/a.html` を返し、存在しない URL には `404.html`（ステータス 404）を返します。
- 自動リロード用の小さなスクリプトは配信時にだけ差し込み、`dist/` には含めません。
- 出力は `target/bare-serve/` に書くので、`dist/` には影響しません。

## 生成されるファイル

| ファイル | 内容 |
| --- | --- |
| `index.html` / `page/<番号>.html` | 記事一覧（新着順）。`per_page` 件ごとにページを分ける |
| `posts/<ファイル名>.html` | 各記事 |
| `<ファイル名>.html` | 固定ページ |
| `tags/<タグ>.html` / `tags/index.html` | タグ別の記事一覧 / タグ一覧 |
| `archive.html` | 年別の記事一覧 |
| `404.html` | 存在しない URL 用のページ |
| `feed.xml` / `sitemap.xml` | Atom フィード（最新 20 件）とサイトマップ。公開 URL が分かる場合だけ |
| その他 | `content/static/` とテーマの `static/` からのコピー |

- 通常のビルドでは `dist/posts/`・`dist/tags/`・`dist/page/` だけを作り直し、ほかのファイルは残します。
- `--clean` は、作業ディレクトリや `content/` を含むディレクトリ（`--out .` など）の削除を拒否します。
- 静的ファイルは、同じパスがあれば `content/static/` → 選んだテーマ → `default` テーマの順に優先します。
- CSS / JS の URL には `?v=<内容のハッシュ>` を自動で付けます。変更したときだけブラウザが取り直します。

## GitHub Pages で公開する

`.github/workflows/pages.yml` が、`main` への push ごとに `cargo test` → `bare check --clean` を実行し、
通れば `dist/` を GitHub Pages にデプロイします（Actions タブから手動でも実行できます）。

- 初回だけ、**Settings → Pages → Build and deployment → Source** を **GitHub Actions** にします。
  未設定のまま実行すると、最初のステップで設定方法を示して止まります。
- 公開 URL（独自ドメインを含む）は自動で `--base-url` に渡されるので、フィード・サイトマップ・OGP は設定なしで生成されます。
  手元のビルドで公開 URL が分からない場合、これらは生成されません（ログに note が出ます）。
- PR とブランチでは `.github/workflows/ci.yml` が、フォーマット・clippy・テスト・`bare check` を実行します。
