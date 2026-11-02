---
title: About
description: このブログと bare について。
nav: 1
---

## このサイトについて

これは [bare](https://github.com/Nagatani/bare) のデモサイトです。
フォークすると、このページを自分のプロフィールに書き換えて使えます。

## 固定ページの作り方

`content/pages/` に Markdown を置くと、サイトのルートに HTML として出力されます。
このページは `content/pages/about.md` から `about.html` として生成されています。

- `nav: 1` のように番号を書くと、ヘッダーのナビゲーションに番号順で表示されます
- `nav` を書かなければ、ナビゲーションには出ません（リンクだけで辿れるページになります）
- 記事と違って日付やタグはなく、フィードにも含まれません

使える項目は [記事の書き方](posts/writing-guide.html#固定ページ) にまとめています。
