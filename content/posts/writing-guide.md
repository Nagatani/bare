---
title: 記事の書き方
date: 2026-11-02
tags: [bare, guide]
description: frontmatter の項目、固定ページ、使える Markdown と HTML、シンタックスハイライトのまとめ。
---

記事は `content/posts/` に置く Markdown ファイルです。ファイル名がそのまま URL になります
（`hello-world.md` → `posts/hello-world.html`）。英数字とハイフンにしておくのがおすすめです。

`bare new my-post --title "記事のタイトル"` で、今日の日付と `draft: true` 入りのファイルを作れます。

## frontmatter

```markdown
---
title: 記事タイトル
date: 2026-10-31
tags: [rust, ssg]
description: 一覧・検索結果・SNS・フィードに表示される説明（任意）
image: /images/cover.png
draft: true
---

本文
```

| 項目 | 内容 |
| --- | --- |
| `title` | タイトル。省略するとファイル名 |
| `date` | `YYYY-MM-DD`。一覧はこの新着順（形式が違うと警告） |
| `tags` | `[a, b]` と `a, b` のどちらでも可。タグ別ページへのリンクになる |
| `description` | 省略すると、本文の最初の段落の先頭（120 文字まで）を使う |
| `image` | SNS で共有したときのプレビュー画像 |
| `draft` | `true` なら出力しない（`bare serve` と `--drafts` では出力） |

見出しが 2 つ以上ある記事には目次が付きます。読了時間の目安は、日本語 500 字 / 分・英語 200 語 / 分で、
コードブロックを除いて計算します。

## 固定ページ

`content/pages/about.md` は `about.html` として出力されます（日付・タグ・フィードなし）。
使える項目は `title`・`description`・`image`・`nav`・`draft` です。
`nav: 1` のように番号を書くと、ヘッダーのナビゲーションに番号順で並びます。

`bare new about --page` で雛形を作れます。

## Markdown

| 記法 | 出力 |
| --- | --- |
| `#` 〜 `######` | 見出し（`id` と `#` リンク付き） |
| 空行区切りのテキスト | 段落（日本語の行末は空白を入れずに連結） |
| ```` ```lang ```` 〜 ```` ``` ```` | コードブロック（4 つ以上のバッククォートで囲めば中に ```` ``` ```` を書ける） |
| `` `code` `` | インラインコード |
| `- item` / `1. item` | リスト（2 文字以上の字下げで入れ子。項目の中にコードブロックや段落も書ける） |
| `- [ ] todo` / `- [x] done` | チェックボックス付きのリスト |
| `本文[^1]` + `[^1]: 注釈` | 脚注（参照順に番号を振り、記事の末尾にまとめる） |
| `> text` | 引用（中にリストなども書ける） |
| `\| a \| b \|` + `\| --- \| :---: \|` | 表（`:` で左・中央・右寄せ） |
| `---` | 区切り |
| `[text](url "title")` / `![alt](url)` | リンク / 画像（画像には `width` と `height` を自動で付与） |
| `**strong**` / `*em*` / `~~del~~` | 強調 / 斜体 / 取り消し線 |
| `\*` など | 記号をそのまま表示 |

- `/images/a.png` のように `/` で始まる URL は、ページからの相対パスに変換されます。
  GitHub Pages のサブパス（`https://<user>.github.io/<repo>/`）で公開しても、画像やリンクが切れません。
- 見出しの `id` は見出しの文字から作られます（例: `#できること`）。重複すると `-1`, `-2` が付きます。
- 記事どうしは普通の相対リンク（`[前回](other-post.html#まとめ)`）でつなぎます。
  リンク先のページや見出しがなければ、`bare check` がエラーにします。

## HTML

Markdown で書けないものは HTML で書けます。

- 行頭から `<div>`・`<details>`・`<figure>`・`<iframe>`・`<video>` などで始めたブロックは、空行までそのまま出力されます
  （中の Markdown は変換されません）。`<pre>`・`<script>`・`<style>`・コメントは、閉じタグまで空行を含めて出力します。
- 段落の中では `<kbd>`・`<ruby>`・`<sub>`・`<sup>`・`<mark>`・`<span>`・`<br>` などのタグが使えます。
- HTML として扱うのは実在する要素名のタグだけです。`Vec<u8>` のような文字列はそのまま表示されます。
  タグを文字として見せたいときは、コードにするか `\<b>` とエスケープします。
- `/` で始まる `src`・`href`・`poster` は、Markdown のリンクと同じく相対パスに変換されます。
- `<https://example.com>` は自動でリンクになります。

## シンタックスハイライト

ビルド時に色を付けるので、ページに JavaScript は入りません。対応言語は次のとおりです。

`rust` `toml` `yaml` `json` `sh`/`bash`/`console` `js`/`ts` `python` `go` `c`/`cpp`/`java`/`kotlin`/`swift`/`csharp` `css` `sql` `html`/`xml` `diff`

それ以外の言語（`text` など）は、色を付けずにそのまま表示します。
色は CSS の `--hl-*` 変数で変えられます（[デザインを変える](customize-design.html)）。
