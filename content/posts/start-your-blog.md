---
title: bare をフォークして自分のブログを始める
date: 2026-11-01
tags: [bare, guide, github-pages]
---

bare は「フォークして、そのまま自分のブログにする」ことを想定したリポジトリです。
GitHub のアカウントさえあれば、Rust をインストールしなくてもブラウザだけで公開まで進められます。
ビルドと公開は GitHub Actions が自動で行います。

## 1. リポジトリをフォークする

[Nagatani/Bare](https://github.com/Nagatani/Bare) を開き、右上の Fork から Create fork を押します。

公開 URL はリポジトリ名で決まります。

- 名前を変えずに `Bare` のまま → `https://<ユーザー名>.github.io/Bare/`
- 名前を `<ユーザー名>.github.io` にする → `https://<ユーザー名>.github.io/`

リンクはすべて相対パスで出力されるので、どちらの URL でもそのまま動きます。

## 2. GitHub Actions を有効にする

フォークしたリポジトリでは、安全のためワークフローが無効になっています。
Actions タブを開き、`I understand my workflows, go ahead and enable them` を押してください。

## 3. GitHub Pages の公開元を設定する

Settings → Pages → Build and deployment の Source を `GitHub Actions` にします。
これを忘れると、ワークフローの deploy ジョブが失敗します。

## 4. サイト名を自分のものにする

`content/site.conf` を開き、鉛筆アイコンから編集します。

```text
title: 自分のブログ
description: 日々の技術メモ
author: あなたの名前
lang: ja
```

`main` ブランチにコミットすると、Actions がビルドとデプロイを始めます。
数十秒後、Actions タブの deploy ジョブに公開 URL が表示されます。

## 5. 記事を書く

`content/posts/` に Markdown ファイルを追加します。ファイル名がそのまま URL になります。
たとえば `my-first-post.md` は `posts/my-first-post.html` として公開されます。

```markdown
---
title: はじめての記事
date: 2026-11-01
tags: [diary]
---

## 見出し

本文です。`inline code` や [リンク](https://example.com) も書けます。

- リスト
- 項目
```

- `date` は `YYYY-MM-DD` で書くと、一覧が新着順に並びます。
- サンプル記事（この記事を含む 3 本）は削除して構いません。記事が 0 本になってもビルドは通ります。
- 使える記法は見出し、段落、リスト、リンク、インラインコード、コードブロックです。

デザインを変えたいときは `content/templates/` の HTML と CSS を編集します。

## 6. 手元でプレビューする（任意）

[rustup](https://rustup.rs/) で Rust を入れておけば、公開前に手元で確認できます。

```sh
git clone https://github.com/<ユーザー名>/Bare.git
cd Bare
cargo run -- --clean
open dist/index.html
```

`dist/` は生成物なので Git には含めません。公開時は Actions が改めてビルドします。

## 7. 本家の更新を取り込む

自分の原稿やデザインは `content/` に、エンジンは `src/` にまとまっています。
そのため、本家の更新はたいてい衝突せずに取り込めます。

GitHub 上でリポジトリのトップにある Sync fork を押すのがいちばん簡単です。
コマンドラインで取り込む場合は、次のようにします。

```sh
git remote add upstream https://github.com/Nagatani/Bare.git
git fetch upstream
git merge upstream/main
```

`content/templates/` を大きく書き換えている場合は、テンプレートで衝突することがあります。
そのときは自分の版を残しつつ、必要な変更だけを取り込んでください。
