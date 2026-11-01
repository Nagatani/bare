# bare

A zero-dependency, minimalist static site generator written in pure Rust.

**フォークするだけで始められる、日本語の技術ブログのための静的サイトジェネレータ。**
`Cargo.toml` の `[dependencies]` は空で、Rust 標準ライブラリ（`std`）だけで動作します。

デモ: <https://nagatani.github.io/bare/>

## bare の特徴

### 壊れたブログを公開しない
`bare check` は、生成した HTML のすべての内部リンク・`#見出し` へのリンク・画像を検査します。
GitHub Actions はこのチェックに通ったときだけデプロイするので、リンク切れが公開されることはありません。
普通の Markdown の相対リンク（`[前回](other-post.html#まとめ)`）がそのまま検査対象です。専用の記法は要りません。
画像の `alt` 抜けと、テンプレートのプレースホルダーの書き間違いも警告します。

### 日本語がそのままきれいに出る
- 段落内の改行で、日本語の文と文の間に余計な空白が入らない（英語圏向けの SSG では、設定しないと空白が入ることがよくあります）
- 日本語の見出しもページ内リンクにできる（`#はじめに`）
- 見出しを文節単位で改行し、システムフォントで和文と欧文を混ぜて読みやすく組む

### 何もしなくても速いページ
| 項目 | 値（このデモサイト） |
| --- | --- |
| Lighthouse | 全 7 ページ（記事・固定ページ・一覧・タグ・アーカイブ）で 4 項目すべて 100（モバイル / デスクトップとも） |
| JavaScript | 0 バイト |
| トップページの転送量（HTML + CSS、gzip） | 約 6KB |
| 画像の `width` / `height` | 自動で付与（表示時にレイアウトがずれない） |
| コードのシンタックスハイライト | ビルド時に生成（JavaScript 不要） |

### フォークしたブログが本家の更新で壊れない
- 自分のデザインは `custom.css` に書く。本家はこのファイルを変更しないので、Sync fork で衝突しない
- 新しいテンプレートや設定はすべて任意。古いテンプレートのままでもビルドが通る
- テンプレートに反映すべき差分は [CHANGELOG.md](CHANGELOG.md) にまとまっている

### 全部読める大きさ
| 項目 | 値 |
| --- | --- |
| 外部クレート | 0（サプライチェーン攻撃の入り口がない） |
| Rust のコード | 約 4,900 行（テストとハイライト用のキーワード表を含む） |
| クリーンな状態からのリリースビルド | 約 1.7 秒 |
| バイナリ | 約 920KB |
| このサイトの生成（リンク検査込み） | 約 25ms |

Lighthouse の値は、Lighthouse 12.6（`npx lighthouse`）でローカル配信したこのリポジトリのサイトを計測したものです。

### 原稿がどこにも縛られない
独自のショートコードはありません。GitHub 上でそのまま表示できる Markdown だけで書くので、
いつでも他のツールやサービスに原稿を持っていけます。

## 自分のブログを始める（フォーク）

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
# 手元でプレビュー（保存すると自動で再ビルドし、ブラウザも自動でリロード）
cargo run -- serve

# 新しい記事（下書き）を今日の日付で作る
cargo run -- new my-post --title "記事のタイトル"

# ビルドして content/ から dist/ を生成
cargo run

# ビルドしてリンク切れがあれば失敗する（CI と同じ検査）
cargo run -- check

# テスト
cargo test
```

| コマンド | 内容 |
| --- | --- |
| `bare` / `bare build` | `dist/` にサイトを生成。リンク切れなどは警告として表示 |
| `bare check` | ビルドしたうえで、リンク切れ・存在しないアンカー・画像があれば終了コード 1 |
| `bare serve` | `http://127.0.0.1:3000/<リポジトリ名>/` でプレビュー。下書きも表示 |
| `bare new <名前>` | `content/posts/<名前>.md` を今日の日付・`draft: true` 付きで作成。`--title` でタイトル、`--page` で固定ページ（`content/pages/`） |

| オプション | 内容 |
| --- | --- |
| `-c, --content <DIR>` | 原稿のディレクトリ（既定: `content`） |
| `-o, --out <DIR>` | 出力先（既定: `dist`、serve は `target/bare-serve`） |
| `--clean` | 出力先を丸ごと削除してから生成 |
| `--drafts` | `draft: true` の記事も出力 |
| `--base-url <URL>` | 公開 URL。`feed.xml`・`sitemap.xml`・OGP の絶対 URL に使う |
| `-p, --port <PORT>` | serve のポート（既定: 3000） |
| `--subpath <PATH>` | serve で配信するパス（既定: GitHub Pages と同じになるよう自動判定） |

### `bare serve`
- 公開時と同じパスで配信します。git の `origin` が `…/bare.git` なら `/bare/`、
  `<user>.github.io` リポジトリや `content/static/CNAME` があれば `/` です。
- GitHub Pages と同じく、`/posts/a` → `posts/a.html`、存在しない URL は `404.html`（ステータス 404）を返します。
- 原稿・テンプレート・CSS を保存すると再ビルドし、開いているページを自動でリロードします。
  リロード用の小さなスクリプトは配信時にだけ差し込み、`dist/` には含めません。
- プレビュー用の出力は `target/bare-serve/` に書くので、`dist/` には影響しません。

### ビルドの細かい挙動
- 通常のビルドでは `dist/posts/`・`dist/tags/`・`dist/page/` だけを作り直し、`dist/` 直下の他のファイルは残します。
- `--clean` では作業ディレクトリや `content/` を含むディレクトリ（`--out .` など）の削除を拒否します。
- `content/static/` の中身（画像・CSS・favicon・`CNAME` など）は、構造を保ったまま `dist/` にコピーされます。
- ページから読み込む CSS / JS には `?v=<内容のハッシュ>` を自動で付けます。CSS を変えたときだけブラウザが取り直すので、古いデザインが残りません。

## GitHub Pages で公開する

`.github/workflows/pages.yml` が `main` への push ごとに `cargo test` → `bare check --clean` を実行し、
チェックに通れば `dist/` を GitHub Pages にデプロイします（Actions タブから手動実行も可能）。
Pages が未設定のまま実行された場合は、最初のステップで設定方法を示すエラーを出して止まります。

初回のみ、リポジトリの **Settings → Pages → Build and deployment → Source** を
**GitHub Actions** に設定してください。公開 URL は `https://<user>.github.io/<repo>/` です。

ワークフローは `configure-pages` が返す公開 URL（独自ドメインを含む）を `--base-url` で渡すので、
`feed.xml`・`sitemap.xml`・OGP の絶対 URL は設定なしで生成されます。
手元のビルドで公開 URL が分からない場合、これらは生成されません（ログに note が表示されます）。

PR とブランチでは `.github/workflows/ci.yml` がフォーマット・clippy・テスト・`bare check` を実行します。

### 生成されるファイル

| ファイル | 内容 |
| --- | --- |
| `index.html` / `page/<番号>.html` | 記事一覧（新着順）。`site.conf` の `per_page` 件ごとにページを分ける |
| `posts/<ファイル名>.html` | 各記事 |
| `<ファイル名>.html` | 固定ページ（`content/pages/`） |
| `tags/<タグ>.html` / `tags/index.html` | タグ別の記事一覧 / タグ一覧（`templates/tag.html` / `tags.html` がある場合） |
| `archive.html` | 年別の記事一覧（`templates/archive.html` がある場合） |
| `404.html` | 存在しない URL 用のページ（`templates/404.html` がある場合） |
| `feed.xml` | Atom フィード（最新 20 件。公開 URL が分かる場合） |
| `sitemap.xml` | サイトマップ（公開 URL が分かる場合） |
| その他 | `content/static/` からのコピー |

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
```

- `date` は `YYYY-MM-DD` で書くと、一覧が新着順に並びます（形式が違うと警告）。
- `tags` は `[a, b]` と `a, b` のどちらの書き方でも構いません。タグはタグ別ページへのリンクになります。
- `description` を省略すると、本文の最初の段落の先頭（120 文字まで）が一覧・検索結果・SNS・RSS に使われます。
- `image` は SNS で共有したときのプレビュー画像です（任意）。
- 見出しが 2 つ以上ある記事には目次（`{{toc}}`）が付きます。読了時間の目安（`{{reading_minutes}}`）は、
  日本語 500 字 / 分・英語 200 語 / 分で、コードブロックを除いて計算します。
- `draft: true` の記事は出力されません（`--drafts` と `serve` では出力）。
- ファイル名がそのまま URL になるので、英数字とハイフンにしておくのがおすすめです。

### 固定ページ

`content/pages/about.md` は `about.html` として出力されます（日付・タグ・フィードなし）。
frontmatter に `nav: 1` のように番号を書くと、ヘッダーのナビゲーションに番号順で並びます。
使える項目は `title`・`description`・`image`・`nav`・`draft` です。

### 対応する Markdown

| 記法 | 出力 |
| --- | --- |
| `#` 〜 `######` | `<h1>` 〜 `<h6>`（`id` と `#` リンク付き） |
| 空行区切りのテキスト | `<p>`（日本語の行末は空白を入れずに連結） |
| ```` ```lang ```` 〜 ```` ``` ```` | `<pre><code class="language-lang">`（対応言語はハイライト。4 つ以上のバッククォートで囲めば中に ```` ``` ```` を書ける） |
| `` `code` `` / ``` `` a`b `` ``` | `<code>` |
| `- item` / `* item` / `+ item` の連続行 | `<ul><li>`（2 文字以上の字下げで入れ子。項目の中にコードブロックや段落も書ける） |
| `1. item` / `1) item` の連続行 | `<ol><li>`（開始番号を引き継ぐ） |
| `- [ ] todo` / `- [x] done` | チェックボックス付きのリスト |
| `本文[^1]` + `[^1]: 注釈` | 脚注（参照順に番号を振り、記事の末尾にまとめて表示） |
| `> text` | `<blockquote>`（中にリストなども書ける） |
| `\| a \| b \|` + `\| --- \| :---: \|` | `<table>`（`:` で左・中央・右寄せ） |
| `---` / `***` / `___` | `<hr>` |
| `[text](url "title")` | `<a href="url">` |
| `![alt](url "title")` | `<img src="url" alt="alt" width height>` |
| `**strong**` / `*em*` / `~~del~~` | `<strong>` / `<em>` / `<del>` |
| `\*` など | 記号をそのまま表示 |

- `/images/a.png` のように `/` で始まる URL は、ページからの相対パスに変換されます。
  GitHub Pages のサブパス（`https://<user>.github.io/<repo>/`）で公開しても画像やリンクが切れません。
- 見出しの `id` は見出しの文字から作られます（例: `posts/hello-world.html#できること`）。重複すると `-1`, `-2` が付きます。
- Markdown で書けないものは HTML で書けます（次の節）。

### HTML

- 行頭から `<div>`・`<details>`・`<figure>`・`<table>`・`<iframe>`・`<video>`・`<!-- -->` などで始めたブロックは、
  空行までそのまま出力されます（中の Markdown は変換されません）。`<pre>`・`<script>`・`<style>`・コメントは、
  閉じタグまで空行を含めて出力します。
- 段落の中では `<kbd>`・`<ruby>`／`<rt>`・`<sub>`・`<sup>`・`<mark>`・`<span>`・`<br>`・`<img>` などのタグがそのまま使えます。
- HTML として扱うのは実在する HTML 要素名のタグだけです。`Vec<u8>` や `Option<T>` のような文字列は、これまでどおりそのまま表示されます。
  タグを文字として見せたいときは `` `<div>` `` のようにコードにするか、`\<b>` とエスケープします。
- `src="/images/a.png"` のように `/` で始まる `src`・`href`・`poster` は、Markdown のリンクと同じく相対パスに変換されます。
- `<https://example.com>` は自動でリンクになります。

### シンタックスハイライト

ビルド時に色付けするので、ページに JavaScript は入りません。対応言語:

`rust` `toml` `yaml` `json` `sh`/`bash`/`console` `js`/`ts` `python` `go` `c`/`cpp`/`java`/`kotlin`/`swift`/`csharp` `css` `sql` `html`/`xml` `diff`

それ以外の言語（`text` など）は、色付けせずにそのまま表示します。色は `style.css` の `--hl-*` 変数で変えられます。

## スタイル（CSS）

CSS は `content/static/css/` にあり、`dist/css/` にコピーされます。

- `style.css` … 既定のテーマ。`@layer reset, tokens, base, layout, components, utilities, overrides` の順に優先度が上がります。
  色・余白・幅などは `tokens` レイヤーの CSS 変数（`--color-accent`, `--content-width`, `--hl-keyword` など）にまとめています。
  ライト / ダークは `light-dark()` で1つの変数に両方の値を持たせています。
- `custom.css` … 自分用の上書き。レイヤーの外に書いたルールは `style.css` のどのルールよりも優先されるので、
  `!important` や長いセレクタは不要です。本家はこのファイルを変更しないので、Sync fork でも衝突しません。

```css
/* custom.css の例: アクセントカラーと本文幅を変える */
:root {
  --color-accent: light-dark(#2563eb, #60a5fa);
  --content-width: 48rem;
}
```

`light-dark()`・CSS ネスト・コンテナクエリなど、2024 年以降の主要ブラウザで使える機能を前提にしています。

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
| `per_page` | — | （空）。記事一覧の 1 ページあたりの件数。空ならすべてを 1 ページに表示 |

すべてのテンプレートで `{{root}}`（ページからサイトのルートへの相対パス）が使えます。
テンプレートごとに使えるトークンは次のとおりです。

| ファイル | 使えるトークン |
| --- | --- |
| `layout.html` | `{{page_title}}`（`<title>` 用）, `{{title}}`, `{{meta}}`（description・OGP・canonical・フィード）, `{{nav}}`（固定ページのナビゲーション）, `{{content}}` |
| `post.html` | `{{title}}`, `{{date}}`, `{{description}}`, `{{tags}}`, `{{toc}}`（目次。短い記事では空）, `{{reading_minutes}}`, `{{content}}` |
| `page.html`（任意） | `{{title}}`, `{{description}}`, `{{toc}}`, `{{content}}` |
| `archive.html`（任意） | `{{archive}}`（年ごとの記事一覧）, `{{post_count}}` |
| `index.html` | `{{post_list}}`, `{{post_count}}`（全記事数）, `{{pagination}}`（ページ送り。1 ページなら空）, `{{page_number}}`, `{{page_count}}` |
| `tag.html`（任意） | `{{tag}}`, `{{post_list}}`, `{{post_count}}` |
| `tags.html`（任意） | `{{tag_list}}`, `{{tag_count}}` |
| `404.html`（任意） | `{{root}}` は `/bare/` のようなルートからの絶対パスになります |

テンプレートに未定義のトークンがあるとビルド時に警告します（出力にはそのまま残ります）。
本文中に書かれた `{{...}}` は置換されません。

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
- `content/static/css/custom.css` は空のまま変更しない（フォーク側のカスタマイズ用）
- テンプレートを変えたら CHANGELOG の「テンプレートの変更」に差分を書く

## ディレクトリ構成

```text
src/
  main.rs       CLI（build / check / serve / new）
  site.rs       記事・固定ページ・タグの読み込みと HTML 生成
  parser.rs     Frontmatter と Markdown パーサー
  highlight.rs  ビルド時のシンタックスハイライト
  template.rs   {{placeholder}} 置換エンジン
  feed.rs       Atom フィードと sitemap
  images.rs     画像ファイルから縦横サイズを読む
  assets.rs     CSS / JS の URL にハッシュを付ける（キャッシュ対策）
  text.rs       抜粋と読了時間
  scaffold.rs   bare new（記事の雛形）
  check.rs      リンク切れ・アンカー・alt の検査
  serve.rs      プレビュー用サーバーと自動リロード
content/
  site.conf     サイト名・説明・著者・言語・公開 URL などの設定
  posts/        記事（*.md。0 本でもビルド可）
  pages/        固定ページ（*.md）
  templates/    layout / post / index（必須）、page / tag / tags / archive / 404（任意）
  static/       dist/ にそのままコピーされるファイル
    css/style.css   既定のテーマ（@layer で整理）
    css/custom.css  自分用の上書きスタイル（本家は変更しない）
.github/
  workflows/pages.yml  テスト → check → GitHub Pages へデプロイ
  workflows/ci.yml     PR とブランチの lint・テスト（stable と最小対応版）
  dependabot.yml       Actions の版を毎週更新
dist/           生成物（git 管理外）
```
