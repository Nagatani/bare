# bare

A zero-dependency, minimalist static site generator written in pure Rust.

**フォークするだけで始められる、日本語の技術ブログのための静的サイトジェネレータ。**

デモ: <https://nagatani.github.io/bare/>

## 特徴

- **依存ゼロ** — Rust の標準ライブラリだけで動きます（OGP 画像の生成だけは Chrome を使います）。
- **壊れたブログを公開しない** — リンク切れがあればデプロイしません。
- **速い** — JavaScript は 0 バイト。Lighthouse は全ページで 100 点です。
- **フォークしても更新を取り込める** — 自分のデザインは本家と衝突しない場所に書きます。

## 始める

Rust のインストールは不要です。ビルドと公開は GitHub Actions が行います。

1. このリポジトリをフォークする
2. フォーク先の **Actions** タブで、ワークフローを有効にする
3. **Settings → Pages** の **Source** を **GitHub Actions** にする
4. `content/site.conf` のサイト名を書き換えて、`main` にコミットする

詳しくは [bare をフォークして自分のブログを始める](content/posts/start-your-blog.md) を見てください。

## 使い方

```sh
cargo run -- serve              # プレビュー
cargo run -- new my-post        # 記事の下書きを作る
cargo run -- check              # リンク切れを検査する
```

## ドキュメント

| 記事 | 内容 |
| --- | --- |
| [フォークして自分のブログを始める](content/posts/start-your-blog.md) | 公開までの手順、ブログのアップデート |
| [記事の書き方](content/posts/writing-guide.md) | frontmatter、固定ページ、Markdown と HTML |
| [デザインを変える](content/posts/customize-design.md) | `custom.css`、テーマ、テンプレート |
| [ビルドと公開のしくみ](content/posts/build-and-publish.md) | コマンド、`site.conf`、ディレクトリ構成、OGP 画像、GitHub Pages |
| [CHANGELOG.md](CHANGELOG.md) | 変更履歴、テンプレートに反映する差分、本家の開発方針 |

## ブログのアップデート

GitHub の **Sync fork** で、本家の改善を取り込めます。
衝突したときの対処は [ブログのアップデート](content/posts/start-your-blog.md#7-ブログのアップデート) を見てください。
