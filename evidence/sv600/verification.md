# sv600 verification — opt-in `rvf.uitest.v1` 制御シーム + `syn-ctl` / `verify-synvelis`（Issue #600 stage 1 slice 1 + stage 2 slice 2）

- 対象 Issue: `synvelis#600`（OPEN 維持 — 両 slice とも `Refs #600`。Issue を閉じない）
- stage 1 slice 1: Branch `feature/600-uitest-seam-native`（初回 push は `scarecrow173/sv600-seam-slice1` / PR #603 → PR #604 で merge、実装 commit `9460e58`）
- stage 2 slice 2: Branch `feature/600-verify-synvelis-doctor` — `syn-ctl` doctor/drive、`verify-synvelis` skill + feature map seed、`.gitignore` carve-out。evidence の `repo.sha` は当該 branch 作業時の base（`d9e11dc`、slice 1 merge commit）を記録する
- 実行環境: Windows 11 (Git Bash) / mise 管理 toolchain（`mise run` / `mise exec` 経由）
- canonical 検証は `mise run` タスク。統合検証（merge 後対象 SHA の CI 証拠）は Coordinator が確認する — 本書の Pass はローカル実測のみを示す

# stage 1 slice 1 — `synvelis-native` opt-in `rvf.uitest.v1` 制御シーム（PR #604 merge 済み、証拠保持）

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

# stage 2 slice 2 — `syn-ctl` doctor/drive + `verify-synvelis` skill + feature map seed

本 slice は slice 1 の `rvf.uitest.v1` seam を変更しない。`syn-ctl` は `ruviform-ui-test::TestDriver` を再利用する薄い制御クライアントとして、framing・transport・secret・protocol 解釈を Synvelis 側に複製しない。

## 変更検証（canonical = mise）

| コマンド | OS | exit | 結果 |
| --- | --- | --- | --- |
| `mise run fmt-check` | Windows | 0 | Passed — 差分なし（refactor 後再実行） |
| `mise run check-app-uitest` | Windows | 0 | Passed — uitest lane 全 target Finished（`syn-ctl` bin 含む） |
| `mise run lint-app-uitest` | Windows | 0 | Passed — clippy `-D warnings` Finished。初回は `too_many_lines` 3 件（`doctor`/`run_session`/`drive_session`）を検出 → `spawn_failed`/`check_identity`/`drive_child_exit` へ helper 抽出して解消（`#[allow]` 不使用） |
| `mise run test-app-uitest` | Windows | 0 | Passed — `uitest::tests` 6 件含む全 suite 緑 |
| `mise run build-app-uitest` | Windows | 0 | Passed — `--locked` dev build Finished |
| `mise run test-app-uitest-ctl`（新設 lane） | Windows | 0 | Passed — `cargo build --features uitest --bin synvelis-native --bin syn-ctl --locked` → `doctor --dry-run`（plan JSON、spawn せず）→ live `doctor --out evidence/sv600/syn-ctl-doctor.json`（verdict `pass`） |
| `cargo check --locked --all-targets -p synvelis-app`（headless 既定 lane） | Windows | 0 | Passed — 非 uitest build 非回帰（`syn-ctl` は `required-features` で uitest lane に隔離） |
| `mise run check-headless-deps` | Windows | 0 | Passed — `no GUI framework deps in 12 headless crates` |
| `mise run check-ruviform-rev` | Windows | 0（再実行） | Passed — `resolved 84a68f25d5…（30 lockfile sources, lockfile-primary, REV match）`。slice 1 と同じ stale vendor 条件（`synvelis#230`）を `mise run sync-ui-deps` で復旧。vendor は gitignore 生成物、差分なし |
| `mise exec -- prek run --all-files` | Windows | 0 | Passed — 全 hook Passed/Skipped |

## `syn-ctl doctor` live 証拠（実 GPUI window、stdio）

`syn-ctl doctor --out evidence/sv600/syn-ctl-doctor.json` → **exit 0 / verdict `pass`**。read-only 6 check 全緑:

| check | verdict | 実測値 |
| --- | --- | --- |
| `spawn` | pass | `TestDriver::spawn_stdio` が `RVF_TEST_CONTROL=stdio` で AUT を spawn |
| `handshake` | pass | `test.hello` → `Welcome`（`v:1`, `session_id:s-dd62b3f2f08eac5a`, `run_id:run-a88cdb6965a08999`, `backend:gpui-windowed/gpui/windows`） |
| `capabilities` | pass | `fixture:true`, `clock_domains:[host_tick]`, `faults:[window_crash,ipc_disconnect]`, `observation:[windows,receipts,notices]`, `screenshot:true`, `input_injection:false`, `dom_access:false` |
| `identity` | pass | `synvelis 0.1.0 @ 84a68f25d5afd1b1f201018d76894d25b122f150` — name/version がこの build、`welcome.app.revision` == `HOST_REVISION`（Cargo.lock の ruviform pin） |
| `shutdown` | pass | `test.shutdown` graceful → `CleanupReport{windows_open:1, windows_closed:1, exited:true, app:{mounts:1, mounts_attached:1, windows_open:1}}` |
| `child-exit` | pass | `try_exit_status` → **exit code 0**（orphan なし） |

fixture・clock・fault・window 変更・screenshot op は doctor から送らない（`--dry-run` の plan JSON にも ops は `test.hello`/`test.capabilities`/`test.shutdown` のみ）。

## `syn-ctl drive` live 証拠（実窓 IPC 注入、2 回観測）

`syn-ctl drive --out evidence/sv600/syn-ctl-drive-run{1,2}.jsonl` → **両 run とも exit 0 / verdict `pass`**、8 records/run（context + 5 step + child_exit + verdict）:

| seq | op | 実測（run 1） |
| --- | --- | --- |
| 1 | `test.hello` | ok — `session_id:s-e675b68ebf8df4df`、`app.revision:84a68f25…` |
| 2 | `test.observe.windows` | ok — window `id:1`, `ready:true` |
| 3 | `test.fixture.prepare` | ok — `ipc_emit` `applied:{window:1}`（body = `rvf.ipc.v1` `syn.app.info` intent、78B） |
| 4 | `test.observe.receipts` | ok — 79 entries, `truncated:false`。注入対: **in `message` seq 76 (78B) → out `post_event` seq 77 (332B)** — `syn.app.info` が inbound boundary を通り typed Result が通常 `post_event` で窓へ戻った |
| 5 | `test.shutdown` | ok — `windows_open:1 → windows_closed:1, exited:true` |
| — | `child_exit` | **exit 0** |
| — | verdict | `pass` |

run 2（`syn-ctl-drive-run2.jsonl`）は同一形状・同一 verdict で再現性を確認（seq/run_id は実行時差）。drive は実 `NativeShell`/`WindowLifecycle`/`AppService` 経路を通るため、注入→receipt の往復が「実アプリの通常経路」を証明する。

## opt-out / blocked 証拠

| probe | env | exit | 実測 |
| --- | --- | --- | --- |
| `syn-ctl doctor`（`evidence/sv600/syn-ctl-disabled.json`） | `RVF_TEST_DISABLE=1` | **1** | verdict `not_run`、`error.environment_blocked:true`、spawn check `not_run`（detail「kill switch」）、`next_step` 明示。fake-pass せず |
| `syn-ctl doctor --dry-run` | — | 0 | plan JSON のみ、spawn なし |
| `syn-ctl doctor --app <nonexistent>`（live 確認済み） | — | **1** | verdict `not_run` + `environment_blocked`（missing AUT は fail でなく環境 blocked） |

## cleanup 記録

doctor/drive とも `test.shutdown` graceful → `CleanupReport{windows_closed:1, exited:true}` → `try_exit_status` で child exit 0 を確認。`TestDriver` Drop が生存 child を kill+reap する二重防衛。probe・evidence 生成物は `evidence/sv600/` に残し、spawn した AUT プロセスの残留なし。

## 未実施（NotRun / Unsupported）と理由

| 項目 | 区分 | 理由・後続 |
| --- | --- | --- |
| `syn-ctl --transport tcp` drive/doctor | NotRun | `TestDriver::spawn_tcp`（loopback+run-bound secret）は配線済み・cli 受理済みだが live 実測は本 slice 範囲外（将来 lane で実測可） |
| feature map 全 7 domain 正規化 | Unsupported（後続 slice） | seed は `features/cli-headless-ops.md` + `features/native-app-via-seam.md` のみ。`synvelis#255` 判例通り段階的に拡張 |
| webview/TCP product lane | Unsupported（後続 slice） | stage 2 後続 slice のスコープ |
| Full CI lanes / 3OS / merge 後統合検証 | NotRun | `POLICIES.md` — scoped equivalent で充足。統合検証は merge 後対象 SHA の CI（Coordinator） |

## artifact 一覧

| file | 内容 |
| --- | --- |
| `verification.md` | 本書 — actions+state 表・lane 記録・cleanup・2回目観測・NotRun |
| `uitest-live-transcript-run1.jsonl` | slice 1: 実窓 drive run 1 の全 control reply + exit code |
| `uitest-live-transcript-run2.jsonl` | slice 1: 同一手順 run 2（再現性） |
| `uitest-live-receipts.jsonl` | slice 1: run 1 の `test.observe.receipts` journal 全 256 entries |
| `uitest-inert-probes.jsonl` | slice 1: disabled / noenv probe 記録（framed reply 0 の証拠） |
| `syn-ctl-doctor.json` | slice 2: live doctor report — 6 checks 全 pass、identity `synvelis 0.1.0 @ 84a68f25…`、child exit 0 |
| `syn-ctl-drive-run1.jsonl` | slice 2: drive run 1 — context/5 steps/child_exit/verdict=pass、注入 receipt 対込み |
| `syn-ctl-drive-run2.jsonl` | slice 2: drive run 2（2 回目観測、同一形状 pass） |
| `syn-ctl-disabled.json` | slice 2: `RVF_TEST_DISABLE=1` blocked report — `not_run` + `environment_blocked` |
