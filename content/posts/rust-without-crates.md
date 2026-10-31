---
title: "クレートなしで Rust を書く"
date: 2026-10-15
tags: rust, std, minimalism
description: 外部クレートを使わず、標準ライブラリだけで Rust のツールを書く利点とコツ。
---

## なぜ依存ゼロなのか

`Cargo.toml` の `[dependencies]` が空であることには、地味ですが確かな利点があります。

- ビルドが速く、`cargo build` が数秒で終わる
- サプライチェーンのリスクを考えなくてよい
- 何がどう動いているかを、すべて自分で説明できる

## 文字列スライスだけでパースする

正規表現がなくても、`strip_prefix` と `split_once` があれば Frontmatter は十分に読めます。

```rust
use std::collections::HashMap;

fn parse(block: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in block.lines() {
        if let Some((key, value)) = line.split_once(':') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}
```

### エスケープも忘れずに

`Vec<String>` のような型をそのまま HTML に書くと `<String>` がタグとして解釈されてしまうため、`<` と `>` は必ず実体参照に置き換えます。
テンプレートの `{{title}}` も、本文中では置換されずにそのまま表示されます。

```html
<h1>{{title}}</h1>
```

## 参考

- [The Rust Standard Library](https://doc.rust-lang.org/std/)
- [std::fs モジュール](https://doc.rust-lang.org/std/fs/index.html)
