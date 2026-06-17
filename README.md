# cap-catalog

SQLite-everywhere な横断機能カタログ。CCoW への指示作成時に「どの repo の
どの機能を見せるか」を**機能単位**でブラウズ・検索し、組み合わせて指示 md を
生成する。

詳細は [Epic #1](https://github.com/ippoan/cap-catalog/issues/1) を参照。

## 配布

| 成果物 | 配布先 |
|---|---|
| `catalog.sqlite` (FTS5 + trigram) | R2 |
| CLI バイナリ (musl static, rusqlite+FTS5) | GitHub Releases |
| ブラウザ UI (SQLite-WASM + OPFS) | (TBD) |

## Schema

[`schema/catalog.sql`](./schema/catalog.sql) が `catalog.sqlite` の唯一の真実。
DB は起動時に `schema_version` を比較し古ければ warn する。

### 抽出元

| Language | Source |
|---|---|
| Rust | rustdoc JSON |
| JS / TS | typedoc + TS API |
| Go | `go/doc` + `go/packages` |

feature 注釈は doc-comment 由来 (例: `//! @feature: tenko-rollcall`)。

## サブ issue

- #2 `catalog.sqlite` schema + ビルド ← **着手中**
- #3 reusable CI: 抽出ワークフロー (言語ディスパッチ)
- #4 Rust 抽出
- #5 JS / TS 抽出
- #6 Go 抽出
- #7 catalog ビルド → R2 配布 + バージョニング
- #8 CLI クライアント (musl static, rusqlite+FTS5)
- #9 ブラウザ UI (SQLite-WASM + OPFS)
- #10 ci-dashboard 表示統合 (display-only)

クリティカルパス: `#2 → #3(+#4/#5/#6) → #7 → consumer(#8/#9/#10)`
