# sv600 verification — `synvelis-native` opt-in `rvf.uitest.v1` 制御シーム（Issue #600 stage 1 slice 1）

- 対象 Issue: `synvelis#600`（OPEN 維持 — 本 slice は `Refs #600`。段階 1 slice 1 で Issue を閉じない）
- Branch: `feature/600-uitest-seam-native`（初回 push は `scarecrow173/sv600-seam-slice1` / PR #603。Gate の branch 命名指針で `feature/` 名へ移行）
- 実装 commit: `9460e58`（evidence はこの commit の延長として同一 worktree で採取）
- 実行環境: Windows 11 (Git Bash) / mise 管理 toolchain（`mise run` / `mise exec` 経由）
- canonical 検証は `mise run` タスク。統合検証（merge 後対象 SHA の CI 証拠）は Coordinator が確認する — 本書の Pass はローカル実測のみを示す

## 変更検証（canonical = mise）

| コマンド | OS | exit | 結果 |
| --- | --- | --- | --- |
| `mise run fmt-check` | Windows | 0 | Passed — `cargo fmt --all -- --check` 差分なし |
| `mise run check-app-native` | Windows | 0 | Passed — `cargo check --features native --all-targets` Finished |
| `mise run lint-app-native` | Windows | 0 | Passed — clippy `-D warnings` Finished、warning なし |
| `mise run build-app-native` | Windows | 0 | Passed — `--locked` dev build Finished（通常経路 bin 不変の確認） |
| `cargo check -p synvelis-app --features uitest --all-targets`（`check-app-uitest` 同等） | Windows | 0 | Passed — uitest lane 全 target Finished |
| `mise run lint-app-uitest` | Windows | 0 | Passed — clippy `-D warnings` Finished |
| `mise run test synvelis-app`（headless 既定 lane） | Windows | 0 | Passed — 全 test binary `test result: ok` |
| `mise run test-app-uitest` | Windows | 0 | Passed — `uitest::tests` 6 件含む全 suite 緑（gate truth table / unauthenticated / disabled serve / malformed emit / Ready-attach-push / end-to-end `ipc_emit`→`syn.app.info`→receipts→`test.shutdown` を実 `NativeShell` 構成で検証） |
| `mise run build-app-uitest` | Windows | 0 | Passed — `--locked` dev build Finished |
| `mise run test-app-gpui` | Windows | 0 | Passed — gpui-test feature 全 suite `test result: ok`（非回帰 lane） |
| `mise run check-headless-deps` | Windows | 0 | Passed — `no GUI framework deps in 12 headless crates` |
| `mise run check-ruviform-rev` | Windows | 0（再実行） | Passed — `resolved 84a68f25d5…（30 lockfile sources, lockfile-primary, REV match）`。初回は worktree の vendored tree が stale（`20a24a66…`、dispatch 時コピー由来・本変更非起因の `synvelis#230` 条件）で fail-closed。`mise run sync-ui-deps` で `ui/vendor/ruviform` を lock rev へ再同期し Pass。vendor tree は gitignore 生成物のためコミット差分なし |
| `mise exec -- prek run --all-files` | Windows | 0 | Passed — 全 hook Passed/Skipped（commit 時 prek hook も全 Pass） |

### 非回帰 lane の注記

`native` feature の処方 lane（`check/lint/build-app-native`）は env-gate 挙動を検証しない — `uitest` seam の runtime opt-in は追加した `test-app-uitest` lane（`cargo test -p synvelis-app --features uitest`）が代替カバーする（`opted_in` truth table・disabled serve・inert env の 3 テスト）。

## 実窓 live 証拠（`RVF_TEST_CONTROL=stdio`、実 GPUI window）

`target/debug/synvelis-native.exe`（`--features uitest` ビルド）を Node フレームクライアント（u32-LE length prefix + JSON、`rvf.uitest.v1`）で駆動。actions→resulting state:

| action | resulting state（実測） |
| --- | --- |
| `test.hello{run_id:"sv600-live-stdio"}` | `result ok` — `app={name:"synvelis",version:"0.1.0",revision:"84a68f25…"}`（`HOST_REVISION`）、`backend="gpui-windowed"/"gpui/windows"`、capabilities `fixture:true, faults:[window_crash,ipc_disconnect], observation:[windows,receipts,notices], screenshot:true, input_injection:false, dom_access:false` |
| `test.observe.windows` | window `id:1`（`ready:true`）— 実窓が mount 済み |
| `test.fixture.prepare{name:"ipc_emit",params:{window:1,body:<syn.app.info intent>}}` | `result ok` — `applied:{window:1}` |
| `test.observe.receipts` | `in "message"`(65B, seq 228) → `out "post_event"`(319B, seq 229) — 注入 `syn.app.info` が inbound boundary で `WindowLifecycle::Action` となり、app の typed Result が通常 `post_event` で窓へ戻った。実窓の `created`/`ready`/`focused`/`resized`/`moved`/`tick` receipt も journaled |
| `test.observe.screenshot{window:1}` | bounded `result err` — `service_error: "screenshot of 1280x800 window exceeds the 1048576-byte reply cap"`。adapter 側の異常ではなく protocol reply cap 内の正直な失敗（cap 超過は error event で返る） |
| `test.shutdown` | `result ok` — `bye:true`、`cleanup:{windows_open:1, windows_closed:1, exited:true, app:{mounts:1, mounts_attached:1, windows_open:1}}`（app 側 `shutdown` の JSON を含む） |
| process exit | **exit 0** — `serve_from_env` が `Shutdown` outcome で正常終了 |

Notices: `window.created`(seq0) → `app.closing`(seq1) → `window.destroyed`(seq2)。

- 全 reply transcript: `evidence/sv600/uitest-live-transcript-run1.jsonl`（context + 9 replies + exit）
- receipt journal 全件: `evidence/sv600/uitest-live-receipts.jsonl`（256 entries + context）

## 2回目観測（再現性確認）

同一手順の第 2 実行（`uitest-live-transcript-run2.jsonl`）— `test.hello` ok → window 1 → `ipc_emit` applied → receipts → screenshot bounded err → `shutdown` ok → **exit 0**。run 1 と同じ reply 形状・同じ cleanup report（receipt seq 番号のみ実行時差）。

## opt-out 証拠（inert 確認）

`uitest-inert-probes.jsonl` — framed `test.hello` を stdin へ書込み 4 秒観測:

| mode | env | framed replies | 判定 |
| --- | --- | --- | --- |
| `disabled` | `RVF_TEST_CONTROL=stdio` + `RVF_TEST_DISABLE=1` | **0** | disable が transport 選択を上回り control channel は開かない（通常 product path が起動。probe 後 terminate） |
| `noenv` | `RVF_TEST_*` 全 unset | **0** | 未設定時も inert — 既定経路そのまま |

## cleanup 記録

`test.shutdown` graceful → notices `app.closing`/`window.destroyed`（journal）→ `CleanupReport{windows_closed:1, exited:true}` → control thread `serve_from_env` が `Shutdown` を返し app thread `join` → **exit 0**。inert probe では通常経路の実窓を terminate（probe 用一時起動、残留物なし）。worktree の stale vendor は `sync-ui-deps` で復旧済み（gitignore 生成物、差分なし）。

## 未実施（NotRun / Unsupported）と理由

| 項目 | 区分 | 理由・後続 |
| --- | --- | --- |
| `RVF_TEST_CONTROL=tcp` lane | NotRun | transport 選択自体は `serve_from_env` 側の upstream 実装。本 slice の seam 証拠は stdio で充足（TCP は loopback+secret の agent 層、将来 fixture 利用時に実測可） |
| Full CI lanes（`check-full`/`lint-full`/`test-full`/`build-full`/`*-msrv`/3OS） | NotRun | `POLICIES.md` §5 — scoped equivalent で充足。統合検証は merge 後対象 SHA の CI（Coordinator） |
| Issue #600 後続 slice | Unsupported（本 slice 範囲外） | 本 slice は seam 開通のみ |

## artifact 一覧

| file | 内容 |
| --- | --- |
| `verification.md` | 本書 — actions+state 表・lane 記録・cleanup・2回目観測・NotRun |
| `uitest-live-transcript-run1.jsonl` | 実窓 drive run 1 の全 control reply + exit code |
| `uitest-live-transcript-run2.jsonl` | 同一手順 run 2（再現性） |
| `uitest-live-receipts.jsonl` | run 1 の `test.observe.receipts` journal 全 256 entries |
| `uitest-inert-probes.jsonl` | disabled / noenv probe 記録（framed reply 0 の証拠） |
