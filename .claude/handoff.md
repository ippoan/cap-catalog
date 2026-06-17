# Session handoff — 2026-06-17 (cap-catalog#33 から継続)

base branch: `claude/eager-dijkstra-vrz1jx` (全 repo 共通の current session branch)

## 未コミットの変更

なし。本セッションで触った全 repo (ohishi-exp/browser-render-rust /
ippoan/ci-workflows) は push 済み。

## 本セッションで完了したもの (cap-catalog#31 の片方を着地)

- ✅ **ippoan/ci-workflows#138 merged** — `rust-ci.yml` と
  `catalog-extract.yml` に optional `submodules` input を追加 (default `""`
  で backward-compat)
- 🟡 **ohishi-exp/browser-render-rust#2 (open, CI 部分 green 待ち)** —
  bootstrap CI 一式 (`ci.yml` / `ci-shape-report.yml` /
  `cap-catalog-extract.yml`) を投入。`submodules: 'recursive'` opt-in、
  `cargo_build_args: ''` (Cargo.lock が .gitignore のため `--locked` 外し)、
  `--all-features` 外し (`grpc` feature が `build.rs` → tonic_build →
  runner に protoc 無く fail) まで適用済み。submodule URL も
  `yhonda-ohishi-pub-dev/rust-scraper` → `ohishi-exp/rust-scraper` に
  更新済 (commit `0a0ab09`)

## 次にやること (優先順)

### 1. ⛔ ohishi-exp/rust-scraper を session scope に追加してから fmt PR

cap-catalog#31 の 1/2 (browser-render-rust) を完全 green に持っていくため
**残るブロッカーは 1 つだけ** = submodule (`ohishi-exp/rust-scraper`) 内の
rustfmt 違反 3 箇所:

- `rust-scraper/src/etc/scraper.rs` L781 (long `.evaluate(...)` chain)
- `rust-scraper/src/etc/scraper.rs` L835 (long `ScraperError::NoUsageData(...)`)
- `rust-scraper/src/service.rs` L50 (long `std::env::var(...).ok().map(...)`)

ohishi-exp/rust-scraper を session scope に add → branch
`claude/eager-dijkstra-vrz1jx` を切る → `cargo fmt` → PR →
merge → browser-render-rust 側の submodule pointer を bump (commit して
ohishi-exp/browser-render-rust#2 に push) で全 job green になるはず。

`mcp__claude-code-remote__*` (add_repo / list_repos) ツールが本 session
では schema search で見つからなかった。次 session の session start 時に
ohishi-exp/rust-scraper を attached repo として含めて起動するのが確実。

### 2. cap-catalog#31 の残り (moneyforward) は既に完了済み

handoff cap-catalog#33 の項目 #1 "moneyforward" は前 session で
`moneyforward#2` (cap-catalog-extract.yml) + `moneyforward#4`
(ci-shape-report.yml) が merge 済み。本セッションでは追加作業なし。

### 3. cap-catalog#32 #1 (builder REPOS list 展開) — 元の最優先タスク

cap-catalog#33 の handoff 元 issue 上の **HIGH 優先タスク** (= 本セッション
では着手せず item #2 の browser-render-rust に分岐した)。`/ci-matrix?format=json`
の `cap_catalog_covered=true` 列挙を runtime curl して
`catalog-build-upload.yml` の `REPOS` list を 31 caller に展開する。

### 4. cap-catalog#32 #3 (TS extract lockfile 不在 conditional cache) — MEDIUM

### 5. ci-dashboard#402 (`/ci-matrix` KV cache staleness) — MEDIUM

## 注意点

- **cross-org `secrets: inherit` 不可** — ohishi-exp/yhonda-ohishi caller
  → ippoan/ci-workflows は `CI_APP_ID` / `CI_APP_PRIVATE_KEY` /
  `RELEASE_WAVE_WEBHOOK_SECRET` 等を明示渡し
- **`Closes/Fixes/Resolves #N` 禁止** — `Refs #N` のみ
- **`mcp__github__enable_pr_auto_merge` を reflex で呼ばない** — user
  明示指示時のみ。auto-merge.yml workflow を持つ repo は workflow 側が
  enable する
- **secret 値を context / PR / issue に出さない** — `secret-inject` skill 経由
- **ohishi-exp/browser-render-rust の deploy は触らない** — `git push` の
  pre-push hook が GHCR build + Kagoya VPS deploy を担当している (user 手元のみ)。
  本 PR は CI 投入のみで deploy 系は意図的に scope 外
