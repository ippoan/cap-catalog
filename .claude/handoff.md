# Session handoff — 2026-06-17 (cap-catalog#29 Phase 1 post-rollout)

## 未コミットの変更

- `/home/user/browser-render-rust` に **local commit `5f6f22c`** あり (3 file 追加: ci.yml + ci-shape-report.yml + cap-catalog-extract.yml)。**push 不能** — repo が `yhonda-ohishi-pub-dev/browser-render-rust` → `ohishi-exp/browser-render-rust` に移動済み、本 session の access scope 外。`mcp__claude-code-remote__add_repo` 未同梱で session スコープ追加も不可
- それ以外の repo は clean

## 次にやること

優先順 (impact 高い順):

### 1. cap-catalog#32 #1 — builder REPOS list 展開 (HIGH、Phase 1 全体に value)

`cap-catalog/.github/workflows/catalog-build-upload.yml` line 85 が `REPOS=("ippoan/cap-catalog")` 1 件のみ pull。**Phase 1 で投入した 31 repo の caller artifact が builder に取り込まれていない** = `v1/latest.jsonl` に他 repo の symbol が乗らない。

推奨実装 (案 c): `/ci-matrix?format=json` を curl して `cap_catalog_covered=true` の repo を runtime 列挙。Phase 1 と同じ SoT を使えば「caller 投入 → 自動取り込み」が一気通貫。
fallback: /ci-matrix unreachable 時は inline 32 repo list で fail-soft。

詳細案: ippoan/cap-catalog#32

### 2. ohishi-exp/browser-render-rust に CI 一式投入 (cap-catalog#31)

local commit `5f6f22c` の patch を ohishi-exp/browser-render-rust に持っていく。
- ci.yml: rust-ci.yml caller、binary_name = browser-render
- ci-shape-report.yml: cross-org (ohishi-exp → ippoan) なので `RELEASE_WAVE_WEBHOOK_SECRET` 明示渡し
- cap-catalog-extract.yml: language: rust

**前提**: 次 session でユーザーが ohishi-exp/browser-render-rust を session scope に追加すること。

注意: `rust-ci.yml` は default で `clippy --all-targets -- -D warnings` が走るので初回 fmt/clippy 違反で fail する可能性大。必要に応じて `cargo_clippy_args: '... -W warnings'` 等で緩めるか、不適合箇所を別 PR で修正。

### 3. cap-catalog#32 #3 — TS extract lockfile 不在 conditional cache (MEDIUM)

`ippoan/ci-workflows/.github/workflows/catalog-extract.yml` の TS path で `setup-node@v4 cache: 'npm'` が `package-lock.json` 不在で fail (= mcp-cf-workers で実害)。修正案: `cache:` を `hashFiles(...) != ''` で conditional 化、もしくは lockfile 不在時は cache を skip。

### 4. ci-dashboard#402 — `/ci-matrix` KV cache staleness 修正

webhook 駆動の shape data が caller workflow 追加後に古いまま残るので、scheduled refresh (6h cron) を CIDashboardHub DO の alarm で実装するのが推奨。詳細は issue body の修正案 #1。

### 5. cap-catalog#31 — moneyforward の追跡 issue 状態を更新

ci-shape-report caller は yhonda-ohishi/moneyforward#4 で merged 済 (報告対象に追記)。RELEASE_WAVE_WEBHOOK_SECRET が repo secret として未投入なので report job は webhook POST で fail するが merge 自体は通過 (= 想定通り)。secret 投入は user 判断。

## 注意点

- **branch policy**: 全 repo で `claude/tender-faraday-86oh1l` を使う (本 session ブランチ)。ただし複数 PR を作る場合は他 branch 名を考慮要 (= 上書きの risk)
- **cross-org `secrets: inherit` 不可**: ohishi-exp / yhonda-ohishi caller → ippoan/ci-workflows 呼び出しは明示渡し必須
  - `RELEASE_WAVE_WEBHOOK_SECRET` (ci-shape-report)
  - `CI_APP_ID` / `CI_APP_PRIVATE_KEY` (auto-merge)
- **`Closes/Fixes/Resolves #N` 禁止** — `Refs #N` を使う
- **`mcp__github__enable_pr_auto_merge` を reflex で呼ばない** — user 明示指示時のみ
- **secret 値を context / PR / issue に出さない** — `secret-inject` skill 経由

## 本 session で merge / filed したもの

- **ci-dashboard#399** ✅ — `/ci-matrix?format=json` 実装 + cap-catalog 取りこぼし検出
- **ci-dashboard#401** ✅ — yhonda-ohishi/freee を `no-code` exclusion
- **ci-dashboard#402** 📋 — `/ci-matrix` KV cache staleness (4 修正案)
- **mcp-relay-rs#38 / alc-app#45 / daiun-salary#15 / moneyforward#3** ✅ — per-job permission tightening (security HIGH#3)
- **moneyforward#2** ✅ — cap-catalog-extract.yml caller 投入
- **moneyforward#4** ✅ — ci-shape-report caller 投入 (webhook secret 不在で report job fail だが merge 済)
- **cap-catalog#31** 📋 — listing 漏れ 2 repo (moneyforward + browser-render-rust)
- **cap-catalog#32** 📋 — Phase 1 builder 取り込み確認 (3 件発見)
- **browser-render-rust 5f6f22c** 🔧 — local commit、push 不能 (上記参照)

Refs ippoan/cap-catalog#29
Refs ippoan/cap-catalog#31
Refs ippoan/cap-catalog#32
Refs ippoan/ci-dashboard#398
Refs ippoan/ci-dashboard#402
