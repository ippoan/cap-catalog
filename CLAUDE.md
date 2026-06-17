# CLAUDE.md

Claude Code 向けの本リポジトリ作業ルール。

設計の親 issue: [#1](https://github.com/ippoan/cap-catalog/issues/1) (epic)

## このリポジトリの方針

- `catalog.sqlite` を **唯一のシリアライズ表現**とする。CLI / ブラウザ UI /
  ci-dashboard / D1 昇格先まで全 consumer が同一 schema + FTS5 クエリを共有する
- MCP ツールはカタログに含めない (= コードシンボル: Rust / JS·TS / Go に集中)
- feature 注釈は **source の doc-comment 由来**に限定 (`//! @feature: <name>`)。
  外部 sidecar ファイルや YAML manifests には書かない
- `schema_version` を DB に保持し、CLI 起動時に比較。古ければ **warn** (fail-close
  はしない — 古い CLI でも read 自体は通す)
- 配布対称: バイナリは GitHub Releases / `catalog.sqlite` は R2
- バイナリ依存は凍結する (= 抽出側 = Rust/TS/Go の依存が増えても、CLI / UI 側の
  実行時依存は広がらない)

## Worktree / branch 命名規則

形式: `<issue-number>-<type>-<short-description>`

- `issue-number`: 必須。先に issue を立ててから branch を作る
- `type`: `feat` | `fix` | `refactor` | `infra`
- `short-description`: 半角小文字英数字とハイフン

Claude Code が自動採番する `claude/...` で実装に入る場合は、対応する issue を
紐付けた上で PR description に `Refs #N` を明記する。

## PR description / commit message のキーワード

- 使用禁止: `Closes #N` / `Fixes #N` / `Resolves #N`
  - PR auto-merge が走った瞬間に issue が自動 close されるため、release 時の
    close 確認 UI と整合しない
- 使用推奨: `Refs #N` / `Related to #N` / `Part of #N`

PR テンプレートは `.github/pull_request_template.md` で `Refs` を強制する
(bootstrap 後に追加)。

## GitHub 自動化 (重要)

- **`main` に直 push しない。** PR を作る (bootstrap 後)
- `mcp__github__enable_pr_auto_merge` を reflex で呼ばない (user 明示指示時のみ)
- PR 作成後は同じ turn で `mcp__github__subscribe_pr_activity` を呼び CI を watch する

## ビルド / テスト

bootstrap 段階。スキーマだけは sqlite3 CLI で検証可能:

```sh
sqlite3 /tmp/catalog.sqlite < schema/catalog.sql
sqlite3 /tmp/catalog.sqlite "SELECT version FROM schema_version;"
```

CI (builder / extract workflows / CLI) は後続 issue で追加する。

---

_共通項を直すときは [`ippoan/claude-md`](https://github.com/ippoan/claude-md) の
`CLAUDE.md.template` を更新すること。_
