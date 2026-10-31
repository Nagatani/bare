# Changelog

フォークしたブログに本家の更新を取り込むときの参考にしてください。
エンジン（`src/`）とワークフローはそのまま取り込めます。`content/templates/` を
カスタマイズしている場合は、「テンプレートの変更」に書かれた差分を自分のテンプレートに反映してください。
反映しなくてもビルドは失敗しません（新しいトークンを書かなければ、その機能が出力されないだけです）。

## Unreleased

### 追加
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
  - `<title>{{title}} · {{site_title}}</title>` → `<title>{{page_title}}</title>`
  - `<meta name="description" content="{{site_description}}">` を削除し、代わりに `{{meta}}` を追加
  - CSS に `.post-desc` と `.lead` を追加
- `post.html`: `<h1>` の直後に `<p class="lead">{{description}}</p>` を追加
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
