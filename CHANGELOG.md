# Changelog

フォークしたブログに本家の更新を取り込むときの参考にしてください。
エンジン（`src/`）とワークフローはそのまま取り込めます。`content/templates/` を
カスタマイズしている場合は、「テンプレートの変更」に書かれた差分を自分のテンプレートに反映してください。
反映しなくてもビルドは失敗しません（新しいトークンを書かなければ、その機能が出力されないだけです）。

## Unreleased

### 追加
- CSS を `content/static/css/style.css` に分離し、`@layer`・`light-dark()`・CSS ネスト・コンテナクエリで再構成
- 自分用の上書きスタイル `content/static/css/custom.css`（本家では変更しません）
- 見出しに `id` と `#` リンクを付与（ページ内リンク用）
- 「本文へ移動」スキップリンク、印刷用スタイル
- PR とブランチ用の CI（fmt・clippy・テスト。stable と Rust 1.78）と Dependabot
- Atom フィード `feed.xml` と `sitemap.xml` を生成（公開 URL が分かるときのみ。GitHub Actions では自動）
- OGP / Twitter Card / canonical の `<meta>` タグを `{{meta}}` で出力
- 記事の frontmatter に `description`・`image`・`draft` を追加
- `--drafts`（下書きも出力）と `--base-url <URL>` オプション
- `site.conf` に `base_url`・`image` を追加
- `404.html` を生成（`content/templates/404.html` がある場合のみ）
- 記事の `date` が `YYYY-MM-DD` でないときに警告を表示
- 日本語やスペースを含むファイル名のリンクを URL エンコード

### テンプレートの変更
- `layout.html`
  - `<style>...</style>` を削除し、`<link rel="stylesheet" href="{{root}}css/style.css">` と
    `<link rel="stylesheet" href="{{root}}css/custom.css">` に置き換え（自分の CSS は custom.css へ移す）
  - `<body>` 直後に `<a class="skip-link" href="#main">本文へ移動</a>` を追加
  - `<main class="wrap">` → `<main id="main" class="site-main wrap">`
  - `<title>{{title}} · {{site_title}}</title>` → `<title>{{page_title}}</title>`
  - `<meta name="description" content="{{site_description}}">` を削除し、代わりに `{{meta}}` を追加
- `post.html`
  - `<h1>` の直後に `<p class="lead">{{description}}</p>` を追加
  - `<div class="post-body">` → `<div class="post-body prose">`
- `404.html`: 新規（任意）

### ワークフロー
- `configure-pages` をビルドの前に移動し、その出力 `base_url` を `--base-url` で渡すように変更

## 2026-11-01

### 追加
- `content/static/` を `dist/` にコピー（画像・favicon・`CNAME` など）
- Markdown: 画像、強調、番号付きリスト、引用、表、水平線、エスケープ
- GitHub Actions の版を Node 24 対応版に更新、Pages 未設定時のエラーメッセージ

### テンプレートの変更
- `layout.html`: favicon の `<link>` と、表・引用・画像用の CSS を追加
