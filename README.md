# bare

A zero-dependency, minimalist static site generator written in pure Rust.

**フォークするだけで始められる、日本語の技術ブログのための静的サイトジェネレータ。**
`Cargo.toml` の `[dependencies]` は空で、Rust 標準ライブラリ（`std`）だけで動作します。
OGP 画像の生成だけは Google Chrome を使います。GitHub Actions ではランナーに入っている Chrome を使うので、準備は要りません
（Chrome がない環境では生成を省略し、ビルドは通ります）。

デモ: <https://nagatani.github.io/bare/>

## 特徴

- **壊れたブログを公開しない** — `bare check` が内部リンク・`#見出し` へのリンク・画像をすべて検査し、
  GitHub Actions は通ったときだけデプロイします。普通の Markdown の相対リンクがそのまま検査対象です。
- **日本語がそのままきれいに出る** — 段落内の改行で余計な空白が入らず、日本語の見出しもページ内リンクにできます。
- **何もしなくても速い** — JavaScript は 0 バイト。シンタックスハイライトはビルド時に生成し、画像には縦横サイズを自動で付けます。
  SNS 用のプレビュー画像（OGP 画像）も、記事のタイトルから自動で作ります。
  Lighthouse はデモの全ページで 4 項目すべて 100 です。
- **本家の更新で壊れない** — 自分のデザインは `custom.css` か自分のテーマに書くので、Sync fork で衝突しません。
  古いテンプレートのままでもビルドが通ります。
- **全部読める大きさ** — 外部クレートは 0 で、サプライチェーン攻撃の入り口がありません。
- **原稿がどこにも縛られない** — 独自のショートコードはなく、GitHub 上でそのまま表示できる Markdown だけで書きます。

## 始める

Rust のインストールは不要で、ビルドと公開は GitHub Actions が行います。

1. 右上の **Fork** からフォークする（リポジトリ名を `<ユーザー名>.github.io` にすると URL がルートになります）
2. フォーク先の **Actions** タブで、ワークフローを有効にする
3. **Settings → Pages → Build and deployment → Source** を **GitHub Actions** にする
4. `content/site.conf` のサイト名などを書き換えて `main` にコミットする → 自動で公開されます
5. `content/posts/` に記事を書き、不要なサンプル記事を削除する

詳しくは [bare をフォークして自分のブログを始める](content/posts/start-your-blog.md) を見てください。

## 使い方

```sh
cargo run -- serve                        # プレビュー（保存すると自動で再ビルド・リロード）
cargo run -- new my-post --title "題名"   # 記事の下書きを作る（--page で固定ページ）
cargo run -- new my-theme --theme         # 既定のテーマをコピーして自分のテーマを作る
cargo run -- check                        # ビルドし、リンク切れがあれば失敗する（CI と同じ）
cargo run                                 # content/ から dist/ を生成
cargo test
```

オプションは `cargo run -- --help` で確認できます。

```text
content/
  site.conf       サイト名・言語・テーマなどの設定
  posts/          記事（*.md）
  pages/          固定ページ（*.md）
  static/         画像・favicon・custom.css など。そのまま dist/ へコピー
  themes/
    default/      既定のテーマ（本家が管理。編集しない）
    <自分のテーマ>/  変えたいファイルだけを置く。site.conf の theme: で選ぶ
```

## ドキュメント

| 記事 | 内容 |
| --- | --- |
| [記事の書き方](content/posts/writing-guide.md) | frontmatter、固定ページ、使える Markdown と HTML、シンタックスハイライト |
| [デザインを変える](content/posts/customize-design.md) | `custom.css`、テーマの作り方、CSS の構成、テンプレートのトークンと引数 |
| [ビルドと公開のしくみ](content/posts/build-and-publish.md) | コマンドとオプション、`site.conf`、プレビュー、生成されるファイル、GitHub Pages |
| [CHANGELOG.md](CHANGELOG.md) | 変更履歴と、フォーク先のテンプレートに反映する差分 |

## 本家の更新を取り込む

GitHub の **Sync fork** か、`git fetch upstream && git merge upstream/main` で取り込みます。

- 削除したサンプル記事が衝突したら、`git rm content/posts/<ファイル>` で削除したままにする
- `site.conf` が衝突したら、自分の版を残す
- 自分のテーマで差し替えたテンプレートには、CHANGELOG の「テンプレートの変更」を見て差分を反映する

## 開発

エンジンは `src/` にあります（`site.rs` が全体の流れ、`parser.rs` が Markdown、`template.rs` がテンプレート）。
フォークを壊さないため、本家では次のことを守ります。

- 古いテンプレートのままでもビルドが通るようにする（新しいトークンは、書かれていなければ出力しないだけにする）
- `site.conf` の新しいキーには必ず既定値を持たせる
- サンプル記事と `site.conf` の変更は最小限にする
- `content/static/css/custom.css` は空のまま変更しない。テーマは `default` だけを管理する
- テンプレートを変えたら、CHANGELOG の「テンプレートの変更」に差分を書く
