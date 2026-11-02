# Changelog

フォークしたブログに本家の更新を取り込むときの参考にしてください。
エンジン（`src/`）とワークフローはそのまま取り込めます。`content/templates/` を
カスタマイズしている場合は、「テンプレートの変更」に書かれた差分を自分のテンプレートに反映してください。
反映しなくてもビルドは失敗しません（新しいトークンを書かなければ、その機能が出力されないだけです）。

## 2026-11-02

### テーマへの移行（フォークした人向け）
テンプレートと CSS を `content/themes/default/` に移し、`site.conf` の `theme:` でテーマを切り替えられるようにしました。
自分のテーマには変えたいファイルだけを置けばよく、残りは `default` が使われるので、今後は本家の更新と衝突しにくくなります。

- **何もしなくてもビルドは通ります。** `content/themes/` がなければ従来どおり `content/templates/` と `content/static/css/` を使います。
  `content/themes/` と `content/templates/` が両方あるときは `content/templates/` を最優先で使い、警告を出します。
- 本家を取り込むと、git は `content/templates/` と `content/static/css/` を `content/themes/default/` へ移します。
  自分で編集していたファイルは、移動先で衝突するか、変更ごと `default` に移ることがあります。その場合は次の手順で整理してください。
  1. 自分のテーマのディレクトリを作る（例: `content/themes/my-theme/templates/` と `content/themes/my-theme/static/css/`）
  2. 自分で変更したテンプレート・CSS の部品だけを、そこへ移す（`git diff upstream/main -- content/themes/default` で確認できます）
  3. `content/themes/default/` を本家の版に戻す（`git checkout upstream/main -- content/themes/default`）
  4. `content/templates/` が残っていれば削除し、`site.conf` に `theme: my-theme` を書く
- `custom.css`・画像・favicon・`CNAME` は、これまでどおり `content/static/` に置きます。

### 追加
- `site.conf` の `archive_title`・`tags_title`・`not_found_title`: 年別アーカイブ・タグ一覧・404 ページの `<title>` に使う名前
  （既定は `Archive`・`Tags`・`404 Not Found`。日本語のブログなら「アーカイブ」などに変えられる）
- テーマ: `content/themes/<名前>/templates/` と `content/themes/<名前>/static/` を、`site.conf` の `theme:` で選択。
  ファイルごとに 選んだテーマ → `default` の順で探し、CSS の `@import` も同じ順で解決（`_tokens.css` だけを差し替えるなど）。
  静的ファイルは `content/static/` が最優先。テーマが見つからない・名前が不正な場合はビルドエラー
- `bare new <名前> --theme`: `content/themes/default/` を `content/themes/<名前>/` にコピーして、自分のテーマを作成
- サンプル記事に「記事の書き方」「デザインを変える」「ビルドと公開のしくみ」を追加し、README の詳しい説明をそちらへ移動。
  サンプル記事が不要なフォークは、これらも削除してください
- テンプレートの引数: `{{pagination class="my-pager" prev="Newer"}}` のように、`{{nav}}`・`{{tags}}`・`{{toc}}`・
  `{{post_list}}`・`{{pagination}}`・`{{archive}}`・`{{tag_list}}` の class 名や文言をテンプレートから変更
  （引数なしの出力は従来どおり。知らない引数や不正な値はビルド時に警告。詳細は README の「テンプレートの引数」）
- 記事一覧のページ分割: `site.conf` の `per_page` 件ごとに `index.html`, `page/2.html`, ... に分け、`{{pagination}}` でページ送りを表示
  （`per_page` が空なら従来どおり 1 ページ。`{{pagination}}` のない `index.html` で `per_page` を設定するとビルド時に警告）
- CSS の `@import` をビルド時に展開: `style.css` から `@import "_x.css";` で読み込んだファイルを 1 つにまとめて出力。
  `_` で始まる CSS は部品として扱い、単体では出力しない。出力する CSS からはコメントと空行を取り除く
- HTML の入力: 行頭の `<div>`・`<details>`・`<iframe>` などのブロックと、段落内の `<kbd>`・`<ruby>` などのタグをそのまま出力。
  `<https://…>` の自動リンク。実在する HTML 要素名だけを対象にするので、`Vec<u8>` などは従来どおり文字として表示
- `bare new <名前>`: 今日の日付と `draft: true` 入りの記事ファイルを作成（`--page` で固定ページ）
- 目次 `{{toc}}`（見出しが 2 つ以上ある記事・固定ページ）と読了時間 `{{reading_minutes}}`
- `description` を省略したとき、本文の最初の段落から抜粋を作って一覧・OGP・フィードに使用
- 年別アーカイブ `archive.html`（`content/templates/archive.html` がある場合）
- Markdown: 入れ子のリスト、リスト項目内のコードブロックや段落、タスクリスト `- [x]`、脚注 `[^1]`
- CSS / JS の URL に内容のハッシュ `?v=…` を自動付与（キャッシュ対策。テンプレートの変更は不要）
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

### 変更
- 既定のテーマ `style.css` を役割ごとのファイル（`_tokens.css`・`_base.css`・`_code.css`・`_layout.css`・
  `_prose.css`・`_components.css` など）に分割。`style.css` は入口として `@import` の一覧だけを持つ。
  `style.css` を直接編集していたフォークは、本家の更新を取り込むときに衝突します。変更は `custom.css` に移してください
- 配色を技術文書の読みやすさ重視に変更: リンクは昔ながらの青（訪問済みは紫）、アクセントもリンクと同じ青に統一。
  コードブロックに枠線、表の見出しと引用に背景色、`<mark>` の色を追加。
  文字色はすべて背景・コード背景に対してコントラスト比 4.5:1 以上。ファビコンと図も青に変更
- 区切り線を減らし、余白で区切るデザインに変更: ヘッダー・フッター・記事ヘッダー・記事一覧・脚注・「記事一覧へ戻る」の
  `border` を削除。ヘッダーは下端をグラデーションでぼかし、フッターは背景色へゆるやかに移る。
  Markdown の `---`（`<hr>`）は線ではなく 3 つの点で表示。表・目次・コード・タグ・ページ送りの枠はそのまま
- 本文が短いページでもフッターが画面の下端に来るように変更: `body` を高さ `100svh` 以上のグリッドにし、
  `<main>` が残りの高さを埋める（CSS のみ。テンプレートの変更は不要）。
  グリッドの列は `minmax(0, 1fr)` にして、長いコードや表があってもスマホ幅でページが横にはみ出さないようにしている
  （この指定がない途中の版では、長いコードや表のある記事がスマホ幅で横にスクロールしていました）
- サイト直下のページ（トップ・年別アーカイブ・固定ページ）で `{{root}}` を空文字ではなく `./` に変更。
  `href="{{root}}"` が自分自身へのリンクにならず、どのページからもトップページ（`/bare/` など）へ戻れる。
  ページ送りの 1 ページ目へのリンクも `index.html` ではなくディレクトリ（`../`）に
- テンプレートの警告に、実際に使われたファイルのパス（例: `content/themes/my-theme/templates/index.html`）を表示
- 読み上げ向けのラベルを追加: `{{nav}}` に `aria-label="サイト"`、記事のタグ一覧に `aria-label="タグ"`、
  ページ送りの ←/→ に「前のページ」「次のページ」。年別アーカイブとタグ一覧の件数は「2026、3件」のように読み上げる
- 変数を追加: `--color-link`・`--color-link-visited`・`--color-link-hover`・`--color-mark`・`--code-bg`・`--code-border`・`--text-code`。
  `--color-accent` は残しています（既定値は `--color-link` と同じ）。レイヤー `overrides` は使われていなかったため削除し、`prose` を追加
- 段落内の `<b>` や `<kbd>` などの HTML タグは、エスケープせずに HTML として出力するようになりました。
  タグを文字として見せたいときは `` `<b>` `` のようにコードにするか `\<b>` と書きます

### テンプレートの変更
- `index.html`: 記事一覧の後にページ送り `<nav class="pager" aria-label="ページ送り">{{pagination}}</nav>` を追加
  （`site.conf` に `per_page` を書く場合は必須）
- `post.html`: メタ情報に `<span>約 {{reading_minutes}} 分で読めます</span>`、ヘッダーの後に
  `<details class="toc-box" open><summary>目次</summary>{{toc}}</details>` を追加
- `archive.html`: 新規（任意）
- `index.html`: 年別アーカイブへのリンク `<a href="{{root}}archive.html">年別アーカイブ</a>` を追加
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
- `post.html` / `archive.html` / `tag.html` / `tags.html` / `404.html`: 戻るリンクの矢印を
  `<span aria-hidden="true">←</span>` に変更（読み上げで「矢印」と読まれないように）
- `tag.html`: `<span class="tag-hash">#</span>` に `aria-hidden="true"` を追加
- `post.html` / `archive.html` / `tag.html` / `tags.html` / `404.html`: 戻るリンクを `{{root}}index.html`（`post.html` は `../index.html`）から
  `{{root}}` に変更（URL に `index.html` が付かない）。`layout.html` のサイト名のリンクは `href="{{root}}"`

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
