# cap-catalog

SQLite-everywhere な横断機能カタログ。CCoW への指示作成時に「どの repo の
どの機能を見せるか」を**機能単位**でブラウズ・検索し、組み合わせて指示 md を
生成する。

詳細は [Epic #1](https://github.com/ippoan/cap-catalog/issues/1) を参照。

## 配布

| 成果物 | 配布先 |
|---|---|
| `catalog.sqlite` (FTS5 + trigram + porter) | R2 (予定) |
| `cap` CLI バイナリ (rusqlite+FTS5) | GitHub Releases |
| ブラウザ UI (SQLite-WASM + OPFS) | (TBD) |

## cap CLI

手元に download した `catalog.sqlite` を read-only に query するシェル CLI。

### インストール

```bash
# Linux x86_64
curl -L https://github.com/ippoan/cap-catalog/releases/latest/download/cap-v0.1.0-x86_64-unknown-linux-gnu.tar.gz | tar xz

# Linux aarch64
curl -L https://github.com/ippoan/cap-catalog/releases/latest/download/cap-v0.1.0-aarch64-unknown-linux-gnu.tar.gz | tar xz

# macOS arm64
curl -L https://github.com/ippoan/cap-catalog/releases/latest/download/cap-v0.1.0-aarch64-apple-darwin.tar.gz | tar xz
```

dev (prerelease) チャネルは `dev-*` tag で配信。stable consumer は `releases/latest` API で `dev-*` を踏まない (`rust-binary-release.yml` が tag に `-` を含むと自動 prerelease 扱い)。

### 使い方

```bash
# 検索 (trigram name + porter doc を UNION)
cap search auth
cap --format json search "JWT"

# 1 行 detail
cap show auth_client::createAuthFetch

# 集計
cap features
cap repos

# embedded schema metadata (DB 不要)
cap schema
```

`--db <path>` または `$CAP_CATALOG_DB` で `catalog.sqlite` の位置を指定 (default `./catalog.sqlite`)。

## Schema

[`schema/catalog.sql`](./schema/catalog.sql) が `catalog.sqlite` の唯一の真実。
DB は起動時に `schema_version` を比較し古ければ warn する。

### 抽出元

| Language | Source | 実装 |
|---|---|---|
| Rust | rustdoc JSON | `scripts/extract-rust.py` (#4) |
| JS / TS | typedoc + TS API | 未実装 (#5) |
| Go | `go/doc` + `go/packages` | 未実装 (#6) |

feature 注釈は doc-comment 由来 (例: `//! @feature: tenko-rollcall`)。

## Workflow

```
[ caller repos (auth-worker / rust-flickr / ...) ]
       │
       │  ci-workflows `catalog-extract.yml` reusable
       │  → rustdoc JSON → JSONL (catalog-extract artifact)
       ▼
[ cap-catalog repo ]
       │  cap-catalog-build (この repo の binary)
       │  → catalog.sqlite
       ▼
[ R2 (予定) ] ──→ [ cap CLI / ブラウザ UI / ci-dashboard ]
```

## サブ issue

- [x] #2 `catalog.sqlite` schema + ビルド
- [x] #3 reusable CI: 抽出ワークフロー (ippoan/ci-workflows#133)
- [x] #4 Rust 抽出
- [ ] #5 JS / TS 抽出
- [ ] #6 Go 抽出
- [x] #7 catalog builder (R2 upload は別 PR)
- [x] #8 CLI クライアント
- [ ] #9 ブラウザ UI (SQLite-WASM + OPFS)
- [ ] #10 ci-dashboard 表示統合 (display-only)

クリティカルパス: `#2 → #3(+#4/#5/#6) → #7 → consumer(#8/#9/#10)`
