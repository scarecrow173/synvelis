# Issue #602 verification — frozen 2-arm benchmark harness (slice 1)

- synvelis SHA: `6a26788e34f905dba7969ab990080bbef3505ea2` (worktree
  `sv602-frozen-bench`, branch `feature/602-frozen-bench-slice1`)
- ruviform resolved SHA: `84a68f25d5afd1b1f201018d76894d25b122f150`
  (lockfile pin; `check-ruviform-rev` REV match after `sync-ui-deps`)
- toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)` / cargo 1.99.1
- environment: windows / x86_64 / native-cli (16 cores)
- spec: `docs/test-specs/issue-602-frozen-render-bench.md`

## Lane matrix

| lane | command | expected | observed | verdict |
| --- | --- | --- | --- | --- |
| fmt | `mise run fmt-check` | clean | clean | pass |
| check | `mise run check synvelis-render` / `synvelis-player` | exit 0 | exit 0 (`--all-targets` incl. bench) | pass |
| lint | `mise run lint synvelis-render` / `synvelis-player` | clippy `-D warnings` clean | clean | pass |
| test | `mise run test synvelis-render` | pass | 99 tests pass | pass |
| test | `mise run test synvelis-player` | pass | all suites pass | pass |
| bench | `mise run bench-frozen` | gate pass + exit 0 | gate `pass` (5/5 cells), `errors=0`, `sha_mismatch=0`; `run-gate-pass.jsonl` | pass |
| selftest | `… --selftest` | gate fail + exit 1 | gate `fail`: K cells `ratio_below_threshold`, P cells `fingerprint_mismatch`; `selftest-gate-fail.jsonl` | pass (fail_expected) |
| fail-closed | `… --baseline <missing>` | fail + exit 1 | `baseline_unreadable` → gate fail | pass (fail_expected) |
| fail-closed | `… --baseline <candidate approval:null>` | fail + exit 1 | `unreadable-or-unpromoted` → gate fail | pass (fail_expected) |
| fail-closed | `… --baseline <malformed>` | fail + exit 1 | gate fail | pass (fail_expected) |
| filter | `… zzz-nomatch` | `gate not_run` + exit 0 | as expected | pass |
| smoke | `… --test` | `gate not_run` + exit 0 | as expected, `errors=0` all cells | pass |
| headless-deps | `mise run check-headless-deps` | pass | 12 headless crates clean | pass |
| ruviform-rev | `mise run check-ruviform-rev` | REV match | `84a68f25` match (after `sync-ui-deps`; vendor was stale `20a24a66`, pre-existing worktree copy — synvelis#230) | pass |
| hooks | `mise exec -- prek run --all-files` | pass | all hooks pass | pass |

NotRun/Unsupported: none for this slice (lane is headless/native-cli by
design; no interactive-session requirement).

## Measured cells (gate-pass run)

| cell | metric | value | bound |
| --- | --- | --- | --- |
| `mix/4x256` | ratio naive/prod | 0.0906 | `min_ratio` 0.0465 |
| `mix/8x512` | ratio | 0.1263 | 0.0596 |
| `mix/16x256` | ratio | 0.1009 | 0.0505 |
| `export/4x48000` | normalized | 2.63 | `max_normalized` 6.15 |
| `export/8x96000` | normalized | 5.73 | 11.69 |

Arm-K correctness: A/B PCM bit-exact (`errors=0`) on all cells.
Arm-P fingerprints: `1177683d…` / `bdda6380…` stable across reps
(`sha_mismatch=0`); `export_frames` 48064/96064 = range + `Known(64)`
tail.

## ruviform#823 (found during this slice)

`Prefetch::manual` initializes its free-block pool inside
`debug_assert!`; in release builds (`cargo bench`) the `try_push` side
effect is elided → producers `Backpressured` forever → consumers
underrun → all-silence (`written=0, mismatch=24576`). Debug builds
(`cargo test`, l1_e2e) never hit it. Workaround in root `Cargo.toml`:
`[profile.bench.package.ruviform-media-reader] debug-assertions = true`
with a removal condition comment. Upstream issue filed:
`scarecrow173/ruviform#823`.

## Files

- `decision.tsv` — appended run ledger (hillclimb schema)
- `run-gate-pass.jsonl` — full JSONL of the gate-pass run
- `selftest-gate-fail.jsonl` — full JSONL of the injected-regression run
- `baseline-candidate.sv602.json` — emitted candidate
  (`"approval": null`, kept unpromoted as evidence)
- promoted pin: `crates/synvelis-render/benches/frozen/baseline.sv602.json`

---

# Issue #602 verification — slice 2 (ruviform rev bump + rf#823 workaround removal + baseline re-pin)

- synvelis SHA: `ea763ab4949c00a0883c1db244c0cb23ffb41a2d` (worktree
  `sv602-rev-bump`, branch `chore/602-ruviform-rev-bump`)
- ruviform resolved SHA: `84a68f25d5afd1b1f201018d76894d25b122f150`
  → `cd4213a1d665600f6adfefa0c3a90f2be9b4e4ce`（lockfile pin — ruviform
  main HEAD; rf#823 `b202f9ab`/`32477e3f`・rf#824 `7853ccd2`・
  rf#631 `a56a0be9` を含有）
- workaround 撤去: ルート `Cargo.toml` の
  `[profile.bench.package.ruviform-media-reader] debug-assertions = true`
  と直上 WORKAROUND コメントブロックを除去（コメント記載の撤去条件
  「pinned rev を bump したら除去」に到達）。当該ファイルの diff は
  このブロック除去のみ。
- toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)` / cargo 1.99.1
- environment: windows / x86_64 / native-cli (16 cores)
- rev-watch 差分精査: `ruviform-cd4213a1-diff-review.md`
  （73 commits — consumer 向け `pub` 破壊 0 件、dep-tree 内は全て
  additive/内部修正。gpui crates.io → git fork tag `v0.2.2-rf1` は
  rf#532 Option-A 由来の pin 起因正当差分）

## Lane matrix (slice 2)

| lane | command | expected | observed | verdict |
| --- | --- | --- | --- | --- |
| lockfile | `mise exec -- cargo update -p <30 ruviform pkgs>` | 全 ruviform crate が単一 SHA | 30 lockfile sources 全て `#cd4213a1`; `git diff Cargo.lock` は ruviform source fragment 更新 + gpui source 置換（pin 由来正当差分）のみ・他 drift なし | pass |
| vendor | `mise run sync-ui-deps` | `ui/vendor/ruviform` を lock SHA で再生成 | `synced ruviform ui/* @ cd4213a1d6`（stale `20a24a66` → `cd4213a1`） | pass |
| build-ui | `mise run build-ui` | dist + manifest 再生成 | `synvelis-assets.json` host `cd4213a1` | pass |
| ruviform-rev | `mise run check-ruviform-rev`（+ `node tools/check-ruviform-rev.mjs --require-vendor` 補助証拠） | lock・REV・manifest 単一 SHA 一致 | `resolved cd4213a1 (30 lockfile sources, lockfile-primary, REV match)` | pass |
| fmt | `mise run fmt-check` | clean | clean | pass |
| check | `mise run check synvelis-render` | exit 0 | exit 0（`--all-targets` incl. bench, 新 rev で compile） | pass |
| lint | `mise run lint synvelis-render` | clippy `-D warnings` clean | clean | pass |
| test | `mise run test synvelis-render` | pass | 99 tests pass | pass |
| test | `mise run test synvelis-player` | pass | all suites pass（HeadlessDriver 本番経路の非回帰） | pass |
| rt_ contract | `mise run test-rt` | pass | 311 test binaries all ok・0 failed | pass |
| headless-deps | `mise run check-headless-deps` | pass | 12 headless crates clean | pass |
| app-native | `mise run check-app-native` | pass | exit 0（gpui fork `f77e44e` tag `v0.2.2-rf1` 含む native feature check — GPUI lane spot check） | pass |
| write-baseline | `mise run bench-frozen -- --write-baseline evidence/sv602/baseline-candidate.sv602.json` | `approval:null` candidate 出力・gate 非参照 | candidate 出力（r12）。**注**: `--write-baseline` の path は harness が verbatim で解釈し bench process cwd（`crates/synvelis-render`）相対で書かれたため `crates/synvelis-render/evidence/` に生成 — evidence path へ移動・stray dir 除去済み（内容は不変） | pass |
| promote | candidate → `benches/frozen/baseline.sv602.json` | `approval` object 化して配置 | cells/env は candidate と byte 一致・`approval` のみ置換（by/at/note 記入）して配置・commit 対象 | done (2 相) |
| bench | `mise run bench-frozen` | gate pass + exit 0 | gate `pass`（5/5 cells）、`errors=0`、`sha_mismatch=0`、`baseline_state: promoted`（r13）。**workaround 撤去後の bench profile で Prefetch 正常動作を実証** | pass |
| selftest | `mise run bench-frozen -- --selftest` | gate fail + exit 1 | gate `fail`・exit 1: K cells `ratio_below_threshold` ×3・P cells `fingerprint_mismatch` ×2（r14） | pass (fail_expected) |
| smoke | `mise run bench-frozen -- --test` | `gate not_run` + exit 0・`errors=0` | as expected | pass |
| hooks | `mise exec -- prek run --all-files` | pass | all hooks pass | pass |

NotRun/Unsupported: `*-full` 系 lane・Feature Matrix・`*-msrv`・3OS
横断・実機 opt-in lane は本 slice の必須検証外（`POLICIES.md` §5・
Daily Full CI・手動 Full CI に委譲）。

## candidate → promote 差分（measured_env）

| key | slice 1 pin | slice 2 candidate/promote |
| --- | --- | --- |
| `synvelis_sha` | `6a26788e…` | `ea763ab4…` |
| `ruviform_sha` | `84a68f25…` | `cd4213a1…` |
| `mix/4x256` `min_ratio` | 0.04654 | 0.04716 |
| `mix/8x512` `min_ratio` | 0.05964 | 0.06039 |
| `mix/16x256` `min_ratio` | 0.05049 | 0.05255 |
| `export/4x48000` `max_normalized` | 6.153 | 5.513 |
| `export/8x96000` `max_normalized` | 11.692 | 12.055 |
| `export/4x48000` `pcm_sha256` | `1177683d…` | `1177683d…`（**同一**） |
| `export/8x96000` `pcm_sha256` | `bdda6380…` | `bdda6380…`（**同一**） |

Arm-P fingerprint は rev bump 前後で bit 一致（export pipeline の
決定性が維持）。Arm-K ratio/Arm-P normalized の bound は同一 session
計測値からの再導出（min_ratio=median/2・max_normalized=median×2 の
既定 margin — 機種差相殺設計どおり絶対値は動くが ratio 系は安定域内）。

## Files (slice 2)

- `decision.tsv` — r12（write-baseline）・r13（gate pass run）・r14
  （selftest gate fail）を自動追記（`--test` は gate `not_run` のため
  row 非追記の契約どおり）
- `baseline-candidate.sv602.json` — 新 candidate（`approval:null` のまま
  保存・旧 slice 1 candidate は git 履歴に残る）
- `ruviform-cd4213a1-diff-review.md` — rev-watch 差分精査メモ
- promoted pin: `crates/synvelis-render/benches/frozen/baseline.sv602.json`
  （`approval` object 化済み — worker が PR 内で promote 実施、slice 1 同型）
