---
title: デザインを変える
date: 2026-11-02
tags: [bare, guide]
description: custom.css での調整、テーマの作り方、CSS の構成、テンプレートのトークンと引数。
---

見た目を変える方法は 2 つあります。本家の更新を取り込んでも衝突しないよう、どちらも本家が触らない場所に書きます。

| やりたいこと | 方法 |
| --- | --- |
| 色・フォント・幅などを少し変える | `content/static/css/custom.css` で CSS 変数を上書きする |
| HTML の構造や CSS を大きく変える | 自分のテーマを作る |

## custom.css で調整する

`custom.css` は既定の CSS の後に読み込まれ、どのルールよりも優先されます。

```css
:root {
  --color-link: light-dark(#2563eb, #60a5fa);
  --color-link-visited: light-dark(#7c3aed, #a78bfa);
  --content-width: 48rem;
}
```

使える変数は `content/themes/default/static/css/_tokens.css` にすべて並んでいます。
ライト / ダークは `light-dark()` で 1 つの変数に両方の値を持たせています。

## テーマを作る

```sh
cargo run -- new my-theme --theme
```

`content/themes/default/` が `content/themes/my-theme/` にコピーされます。
`content/site.conf` に `theme: my-theme` と書くと、そのテーマでビルドされます。

```text
content/themes/
  default/            本家が管理する既定のテーマ（編集しない）
    templates/
    static/css/
  my-theme/           自分のテーマ
    templates/post.html
    static/css/_layout.css
```

- テンプレートと CSS は、選んだテーマ → `default` の順に探します。
- **変えなかったファイルは削除してください。** 削除したファイルは `default` のものが使われるので、
  本家がそのファイルを改善したときに、更新がそのまま届きます。
- CSS の `@import` も同じ順に探します。`_layout.css` だけを残せば、`default` の `style.css` がそれを読み込みます。
- CSS の部品はファイルごと差し替わります（`_tokens.css` を置くと、`default` の `_tokens.css` は使われません）。
- 画像・favicon・`CNAME`・`custom.css` はテーマに関係ないので、`content/static/` に置きます。こちらが最優先です。

## CSS の構成

ビルド時に `style.css` の `@import` を展開し、`dist/css/style.css` の 1 ファイルにまとめます。
`_` で始まるファイルは部品で、単体では出力されません。出力からはコメントと空行を取り除きます。

| ファイル | 内容 | レイヤー |
| --- | --- | --- |
| `style.css` | 入口。レイヤーの順番と `@import` の一覧 | — |
| `_tokens.css` | 色・フォント・余白・幅などの CSS 変数 | `tokens` |
| `_reset.css` | ブラウザ既定のスタイルの打ち消し | `reset` |
| `_base.css` | 見出し・段落・リスト・引用・表などの HTML 要素 | `base` |
| `_code.css` | コード・シンタックスハイライト・`<kbd>` | `base` |
| `_layout.css` | ヘッダー・本文エリア・フッター・印刷用 | `layout` |
| `_prose.css` | 見出しの `#`・目次・脚注・タスクリスト | `prose` |
| `_components.css` | ナビゲーション・記事一覧・タグ・ページ送り・アーカイブ | `components` |
| `_utilities.css` | `.visually-hidden` などの補助クラス | `utilities` |

- 優先度は `reset, tokens, base, layout, prose, components, utilities` の順に上がります。
  レイヤーの外にある `custom.css` はそのすべてに勝つので、`!important` は不要です。
- 区切りは線ではなく余白で表します。線を使うのは表・目次・コードブロック・タグ・ページ送りなどの枠だけです。
- 文字色はすべて、背景とコードの背景に対してコントラスト比 4.5:1（WCAG AA）以上にしています。
- `light-dark()`・CSS ネスト・コンテナクエリ・`:has()` など、2024 年以降の主要ブラウザの機能を前提にしています。

## テンプレート

テンプレートは `{{名前}}` の部分を置き換えるだけの HTML です。
すべてのテンプレートで、`site.conf` の値（`{{site_title}}`・`{{site_description}}`・`{{site_author}}`・`{{site_lang}}`・`{{site_url}}`）と、
ページからサイトのルートへの相対パス `{{root}}`（`./` や `../`）が使えます。
トップページへのリンクは `href="{{root}}"` と書くと、`index.html` を付けずにどのページからでも正しく戻れます。

| ファイル | 使えるトークン |
| --- | --- |
| `layout.html` | `{{page_title}}`, `{{title}}`, `{{meta}}`（description・OGP など）, `{{nav}}`, `{{content}}` |
| `post.html` | `{{title}}`, `{{date}}`, `{{description}}`, `{{tags}}`, `{{toc}}`, `{{reading_minutes}}`, `{{content}}` |
| `index.html` | `{{post_list}}`, `{{post_count}}`, `{{pagination}}`, `{{page_number}}`, `{{page_count}}` |
| `page.html`（任意） | `{{title}}`, `{{description}}`, `{{toc}}`, `{{content}}` |
| `archive.html`（任意） | `{{archive}}`, `{{post_count}}` |
| `tag.html`（任意） | `{{tag}}`, `{{post_list}}`, `{{post_count}}` |
| `tags.html`（任意） | `{{tag_list}}`, `{{tag_count}}` |
| `404.html`（任意） | `{{root}}` は `/bare/` のようなルートからの絶対パスになる |

任意のテンプレートがなければ、そのページは出力されません（`page.html` は簡易な既定のものを使います）。
知らないトークンはビルド時に警告し、出力にはそのまま残します。

### トークンの引数

HTML をまとめて出力するトークンは、`{{名前 キー="値"}}` の形で class 名や文言を変えられます。
省略した引数は既定値になります。

```html
{{pagination class="pagination my-pager" prev="Newer" next="Older" prev_label="Newer posts" next_label="Older posts"}}
```

| トークン | 引数（既定値） |
| --- | --- |
| `{{nav}}` | `class`（`site-nav`）, `label`（`サイト`） |
| `{{tags}}` | `class`（`tags`）, `item_class`（`tag`）, `label`（`タグ`）, `prefix`（空。例: `#`） |
| `{{toc}}` | `class`（`toc`） |
| `{{post_list}}` | `item_class`（`post-item`）, `desc`（`true`）, `tags`（`true`） |
| `{{pagination}}` | `class`（`pagination`）, `current_class`（空）, `prev`（`←`）, `next`（`→`）, `prev_label`（`前のページ`）, `next_label`（`次のページ`）, `gap`（`…`） |
| `{{archive}}` | `class`（`archive-year`）, `heading`（`h2`）, `count_prefix`（`、`）, `count_suffix`（`件`） |
| `{{tag_list}}` | `link_class`（`tag`）, `count_prefix`（`、`）, `count_suffix`（`件`） |

- `label`・`prev_label`・`next_label` は読み上げ用の `aria-label` です。
- `count_prefix` と `count_suffix` は件数の前後に付く読み上げ専用の文字で、画面には出ません
  （既定では「2026、3件」と読まれます。英語なら `count_prefix=", " count_suffix=" posts"`）。
- class を `""` にすると class 属性を出力しません。`desc`・`tags` は `"true"` か `"false"` です。
- 値はそのまま出力されるので、`prev="<svg>…</svg>"` のように HTML も書けます。値の中に `"` は書けません。
- 知らない引数や不正な値はビルド時に警告し、既定の出力にします。
