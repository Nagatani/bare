# Changelog

フォークしたブログに本家の更新を取り込むときの参考にしてください。
エンジン（`src/`）とワークフローはそのまま取り込めます。`content/templates/` を
カスタマイズしている場合は、「テンプレートの変更」に書かれた差分を自分のテンプレートに反映してください。
反映しなくてもビルドは失敗しません（新しいトークンを書かなければ、その機能が出力されないだけです）。

## Unreleased

### 追加
- `bare serve`: 保存すると再ビルドして自動リロードするプレビューサーバー（GitHub Pages と同じサブパス・404 の挙動）
- `bare check`: 生成した HTML の内部リンク・`#アンカー`・画像のリンク切れを検査（CI はこれに通ったときだけデプロイ）
- ビルド時のシンタックスハイライト（Rust・TOML・YAML・JSON・シェル・JS/TS・Python・Go・C 系・CSS・SQL・HTML・diff）
- タグ別ページ `tags/<タグ>.html` とタグ一覧 `tags/index.html`。記事のタグがリンクになる
- 固定ページ `content/pages/*.md` と、ヘッダーのナビゲーション（frontmatter の `nav`）
- Markdown の画像に `width` / `height` を自動付与（PNG・GIF・JPEG・WebP・SVG）
- テンプレートに未定義のプレースホルダーがあると警告
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
- `tag.html` / `tags.html` / `page.html`: 新規（任意。置くとタグ別ページ・タグ一覧・固定ページ専用の表示になる）
- `index.html`: タグ一覧へのリンク `<a href="{{root}}tags/">タグ一覧</a>` を追加
- `layout.html`
  - ヘッダーを `<p class="brand-group">`（サイト名 + キャッチコピー）と `{{nav}}` に変更
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
- `pages.yml` / `ci.yml` のビルドを `bare check` に変更（リンク切れがあると失敗する）

## 2026-11-01

### 追加
- `content/static/` を `dist/` にコピー（画像・favicon・`CNAME` など）
- Markdown: 画像、強調、番号付きリスト、引用、表、水平線、エスケープ
- GitHub Actions の版を Node 24 対応版に更新、Pages 未設定時のエラーメッセージ

### テンプレートの変更
- `layout.html`: favicon の `<link>` と、表・引用・画像用の CSS を追加
