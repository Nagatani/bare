---
title: bare をフォークして自分のブログを始める
date: 2026-11-01
tags: [bare, guide, github-pages]
description: リポジトリをフォークして、GitHub Pages で自分のブログを公開するまでの手順。
---

bare は「フォークして、そのまま自分のブログにする」ことを想定したリポジトリです。
GitHub のアカウントさえあれば、Rust をインストールしなくてもブラウザだけで公開まで進められます。
ビルドと公開は GitHub Actions が自動で行います。

## 1. リポジトリをフォークする

[Nagatani/bare](https://github.com/Nagatani/bare) を開き、右上の Fork から Create fork を押します。

公開 URL はリポジトリ名で決まります。

- 名前を変えずに `bare` のまま → `https://<ユーザー名>.github.io/bare/`
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

Rust を入れてあれば、`cargo run -- new my-first-post --title "はじめての記事"` で
今日の日付入りのファイルを作れます。作った記事は下書き（`draft: true`）なので、消すまで公開されません[^draft]。

```markdown
---
title: はじめての記事
date: 2026-11-01
tags: [diary]
description: 一覧や SNS に表示される短い説明
---

## 見出し

本文です。`inline code` や [リンク](https://example.com) も書けます。

- リスト
- 項目
```

- `date` は `YYYY-MM-DD` で書くと、一覧が新着順に並びます（形式が違うとビルド時に警告が出ます）。
- `description` は一覧ページ・検索結果・SNS のプレビュー・フィードの要約に使われます。
  省略すると、本文の最初の段落の先頭が代わりに使われます。
- `image: /images/cover.png` を書くと、SNS で共有したときのプレビュー画像になります（省略可）。
- 最初から入っているサンプル記事（この記事を含む）は削除して構いません。記事が 0 本になってもビルドは通ります。
- 使える記法は [記事の書き方](writing-guide.html) にまとめています。
- 見出し（`##`）が 2 つ以上ある記事には、自動で目次が付きます。読了時間の目安も表示されます。

[^draft]: 下書きは `cargo run -- serve` のプレビューでは表示されるので、書きながら確認できます。

画像やファイルは `content/static/` に置きます。中身はそのまま `dist/` にコピーされます。

1. 画像を `content/static/images/diagram.png` に置く
2. 記事から `![図の説明](/images/diagram.png)` のように参照する

`content/static/` には favicon（`favicon.svg`）も入っているので、差し替えれば自分のアイコンになります。
独自ドメインを使う場合は、`content/static/CNAME` にドメイン名を書いておきます。

デザインを変えたいときは `content/static/css/custom.css` に CSS を書きます。
たとえば次のように書くと、リンクの色と本文の幅が変わります。
色・フォント・余白などの変数は `content/themes/default/static/css/_tokens.css` に一覧があります。

```css
:root {
  --color-link: light-dark(#2563eb, #60a5fa);
  --content-width: 48rem;
}
```

`custom.css` は本家では変更されないので、本家の更新を取り込んでも衝突しません。
HTML の構造を変えたいときは、`cargo run -- new my-theme --theme` で自分のテーマを作ります
（詳しくは [デザインを変える](customize-design.html)）。

プロフィールなどの固定ページは `content/pages/` に置きます（[About](../about.html) がその例です）。
frontmatter に `nav: 1` と書くと、ヘッダーのナビゲーションに表示されます。

タグ別の記事一覧・年別アーカイブ・Atom フィード・サイトマップ・404 ページは自動で生成されます
（一覧は [ビルドと公開のしくみ](build-and-publish.html#生成されるファイル)）。

## 6. 手元でプレビューする（任意）

[rustup](https://rustup.rs/) で Rust を入れておけば、公開前に手元で確認できます。

```sh
git clone https://github.com/<ユーザー名>/bare.git
cd bare
cargo run -- serve
```

表示された URL（`http://127.0.0.1:3000/bare/` など）をブラウザで開きます。
記事を保存すると自動で再ビルドされ、ブラウザも自動でリロードされます。下書きもここでは表示されます。

公開前にリンク切れがないかは `cargo run -- check` で確認できます。
GitHub Actions も同じ検査をしていて、記事どうしのリンクや `#見出し` へのリンク、画像が切れていると公開を止めます。
どのファイルのどのリンクが切れているかは、Actions のログに表示されます。

## 7. 本家の更新を取り込む

自分の原稿やデザインは `content/` に、エンジンは `src/` にまとまっています。
そのため、本家の更新はたいてい衝突せずに取り込めます。

GitHub 上でリポジトリのトップにある Sync fork を押すのがいちばん簡単です。
コマンドラインで取り込む場合は、次のようにします。

```sh
git remote add upstream https://github.com/Nagatani/bare.git
git fetch upstream
git merge upstream/main
```

衝突しやすいのは、自分で書き換えた `content/site.conf` と、削除したサンプル記事です。
デザインを `custom.css` や自分のテーマで変えていれば、本家の `default` テーマとは衝突しません。

- 削除したサンプル記事が衝突したら、`git rm content/posts/<ファイル名>` で削除したままにします
- `site.conf` が衝突したら、自分の版を残します
- 本家が新しいサンプル記事を追加した場合も、不要なら削除してください

自分のテーマで差し替えたテンプレートには、本家の変更が届きません。
本家の [CHANGELOG.md](https://github.com/Nagatani/bare/blob/main/CHANGELOG.md) の「テンプレートの変更」を見て、必要な差分だけを手で反映してください。
反映しなくてもビルドは通ります（新しい機能が出力されないだけです）。
