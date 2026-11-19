# CLI・配線・本番統合スライス — 実行証拠

Synvelis の CLI・配線・本番統合スライス群で実施された検証の記録をまとめる。headless/fake driver 上の Pass と実機・実 OS での受入は区別して記し、NotRun・Deferred・計画段階の検証は実施済みと誤読されないよう明示する。文書 status には drift があり、synvelis#515 の task 文書は `status: "todo"`・checkbox 未チェックのまま、synvelis#518 の IR は `status: "in-progress"` のまま残っている。Issue 自体が OPEN 継続のもの（synvelis#262・#274）は slice 単位の検証完了を意味し、外部 gate は別 Issue/別 lane に帰属する。

## synvelis#262 headless CLI 実行配線

Windows headless 実バイナリ E2E 24/24 green。`workflow run --file` `.yaml` emit-only→completed/0、`.luau` 実 VM→plan→coordinator→completed/0、query step→live lane→Gateway→empty policy `denied`/exit 3、`action run --file` `.luau`→`denied`/exit 3 + dry-run `executed:false`+recipe metadata/exit 0、`--inputs`/`--inputs-file`/相互排他/宣言 input 不足の fail-closed、分類行列（`.rhai` PoC・`.lua`・未知拡張子・kind mismatch）維持。`cargo test` synvelis-cli 24/24・synvelis-automation・synvelis-gateway 全 pass、review-protocol blocker 0。

残件（実 job step・named macro 実解決・approval→apply 継続・durable run journal・製品 E2E/3OS）は NotRun/Deferred で Issue #262 は OPEN 継続（synvelis#262）。

## synvelis#274 script analyze

`synvelis-cli script analyze --file` の実バイナリ e2e 9 tests（happy path・diagnostics=completed data・completion・fail-closed 行列・実行面非回帰・human 分離）+ 既存 `cli_e2e` 26 全 Pass。mise `fmt-check`/`check-cli`/`lint-cli`（`-D warnings` warning 0）/`test-cli`/`build-cli`/`check synvelis-automation`/`check synvelis-script`/`check-headless-deps`（12 headless crates に GUI deps なし）/`prek run --all-files`/`git diff --check` 全て exit 0。PR #509、base `b5a8919`、`git merge origin/main` は Already up to date（Cargo.lock 差分なし）。

実 Luau.Analysis FFI（ruviform#560・rf#692）・GPUI editor・mount・DTCG token・Rhai analyzer・`*-full`/`*-msrv`/`gw10`/`test-rt`/`check-app-*` lane は NotRun。Issue #274 OPEN 継続（synvelis#274）。

## synvelis#283 strip-fields gateway exercise

`cb06_strip_gateway.rs` 新規 2 test Pass（strip fixture `AddTrackStrip`+`InsertProcessor` revision 2、fork・ordinary op typed reject `clip.mute`/`track.create`/`clip.move`・stale scope digest/branch revision reject・`ApproveToApply` agent approval adopt・discard・list/describe/compare・domain boundary/capability 補助）。mise `check synvelis-gateway`/`fmt-check`/`lint synvelis-gateway`/`test synvelis-gateway`/`check-headless-deps`/`check synvelis-project`/`prek run --all-files` 全て pass。wire 表面に strip op capability が無い実測に基づき positive mask-gated edit 非表現を確定し、spec の Deferred Design Concerns に記録。test-spec 文書は新設せず cb-06 test-spec の「Gateway commit-gate 経路カバレッジ」節に検証意図を追記（test-spec-skip を front matter に明記）（synvelis#283）。

## synvelis#286 Difference Explorer app wiring

workspace `cb09_diff_explorer` 40/40 Pass（compare・provider provenance・CAS adopt・typed reject・authority/context・adapter 解決を含む）、app `cb09_variant_diff` 11/11 Pass（op dispatch・view lifecycle・mount smoke・project 切替 cleanup・stale/implicit target reject・preview 非 commit）。`mise run check-app-native`/`lint-app-native`/`test-app-gpui`/`check-app-gpui` は final head で exit 0。`mise run fmt-check` は本 worktree で Windows `os error 206`（環境起因・diff 無関係）により exit 1 — 全 15 member の個別 `cargo fmt -p -- --check` は pass、main repo では aggregate も exit 0 を確認済み。実 GPUI window/plugin dock/RT/PCM/perceptual acceptance は synvelis#288・CB-11 gate の範囲（synvelis#286）。

## synvelis#324 plugin_instances VCS diff

`cargo test -p synvelis-project --test vcs02_diff` 36 passed/0 failed（新規 plugin-instance 分類 8 test: entry Added/Removed・`id` swap `Replaced`・`plugin` rebind `Replaced`・watermark-only drift `Equal`/`Complete`・pre-v13 `id` `Suppressed`→`PartialProjection`・policy v3 で instance-only 差分 `Different`・processor raw の cross-kind 衝突 `AmbiguousIdentity(CrossKindId)`・`target_fingerprint` が `PluginInstance` で `Unsupported`）。`plugin_instance_persist` 13 + `vcs06_apply_matrix` 8 test 全 pass。mise `fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`/`build` 全 pass（Windows 11・rust 1.99.0 pinned。初回は `assert_is_empty` lint と fmt 整形差分で fail → 修正後 pass）。spec-doc は route 判断により不作成（VCS-02 spec の既存分類規約内の拡張）。`PluginInstance` change の apply/restore は Deferred Design Concern として別 Issue 候補、VCS-06 ledger 追記と docs index は別 worker 占有、Issue #324 の close 判定は coordinator/人間に委譲（synvelis#324）。

## synvelis#365 impl index 再生成

`index plan --dir docs/impl/ir` が変更前 rejected（147 件中 44 件 front matter schema 違反）・`docs/impl/exp` が generated-index marker 欠落で rejected・引数なし `index apply` が exit 1 → 是正後 `index plan`→`sync`(148 changes)→`index apply --expect <planHash>` exit 0（written=1）→再 plan で `in-sync`。引数なし `index apply` exit 0・written=5（designs/plans/specs/tasks/test-specs README）。`audit_impl_record.js` error 319（baseline 331 から減少、missing-section 319 は schema 是正で可視化された既存不備）、warning 54、index 系 finding 0、本 IR の finding 0。`audit_experiment_log.js` error 52（baseline と同一、`issue-147-sccache.jsonl` の invalid-event-shape 既存不備）・warning 0（index-not-managed 解消）。変更 44 文書は front matter 以降 body が HEAD と byte 一致・id/title/created/owners 不変・rename/delete なし（M のみ）。OS: Windows、Node.js v26.12.1（synvelis#365）。

## synvelis#368 IR missing-section 補完

`audit_impl_record.js` の `missing-section` finding 319→0 件（60 文書・必須 7 セクション全件に非空本文、正規セクション名の重複なし）。`missing-experiment-link` 18・`broken-relation-link` 36 は front matter 由来 warning で baseline 同一・増加なし（範囲外）。60 文書全件で front matter が HEAD と byte 一致・ファイル名不変をスクリプト検証（問題 0 件）。`index plan --dir docs/impl/ir` が `in-sync`（本文のみの変更のため drift なし）。review-protocol 協調・敵対 2 系統で敵対側 Issue Candidate なし、協調側指摘は作業用一時スクリプト（repo root の `*.js`/`*.txt`/`*.json`）が commit 対象になり得る点のみ（commit 前に削除して解消、P2）。docs-only 変更のため製品コード・テストの再実行は対象外（synvelis#368）。

## synvelis#514 session engine selection 判定

判定分岐 B（実装不要）を採用。変更は `crates/synvelis-app/src/engine.rs` の module doc comment のみ（selection 非消費が意図的である旨と将来の配線 trigger を明記）+ doc chain 9 文書 + `evidence/sv514/`。`synvelis-app` の dep 追加なし（synvelis-render 非依存維持）、`synvelis-player`/`synvelis-project`/`synvelis-workspace`（`rcp_selection.rs` 以外）変更なし、`Cargo.toml`/`Cargo.lock` 変更なし。EXP-1〜4 全て棄却（rebuild 置換・観測併走・engine 側消費・workspace 側 stateless 表現）。stateless `PluginStateDecl` は player 契約変更が必要と確定し別 Issue に帰属（→ synvelis#530 で解決）（synvelis#514）。

## synvelis#515 OS credential session

`cargo test -p synvelis-gateway --test cli_adapter` 30/30 pass（新規 FileCliSessionStore 6 tests: restart verify・expiry/revocation persist・digest-only file・forged kind/malformed/agent-spec fail-closed）。`cargo test -p synvelis-cli --test cli_e2e session_` 2/2 pass（実 Windows Credential Manager での issue→list→revoke roundtrip + `--session` verified-Human attestation + revoke 後 fail-closed + secret 非流出）。非 Windows は `CliError::Unsupported`(exit 5)。正規検証一式（mise fmt/check/lint/test/build/headless-deps/prek）は evidence/sv515 ledger に実測記録。対話存在証明は synvelis#114 系 deferred、macOS/Linux vault は Unsupported（NotRun）、実 2-process/GUI 受入は synvelis#267/#114 lane（synvelis#515）。

## synvelis#516 mlua→ruviform adapter

`cargo test -p synvelis-script-luau` 30 test pass・0 fail（real-VM: descriptor・言語/digest/budget fail-closed・artifact・sandbox globals・session isolation・evaluate-once・cancel）。`mise run test synvelis-automation` 全 test pass（`.luau` real-VM resolver/eval・mixed-source run 含む）、`check-cli`/`lint-cli`/`test-cli` pass（lease 外 consumer 無変更）、`check-headless-deps` pass（12 headless crates・GUI/GPU/WebView なし）、prek pass。`cargo tree` で mlua は `ruviform-script-luau` 経由のみ、synvelis crate は mlua を直接宣言しない（推移的に隔離）。E1〜E5 全て解決: 薄い binding 採用・pin 更新必須確認・artifact Record 必須の分類差・`BudgetExceeded.kind` Option 化・clippy 修正。実 Luau.Analysis native FFI bridge 切替は synvelis#274 gate、macOS/Linux 実機受入は synvelis#267 gate（NotRun）（synvelis#516）。

## synvelis#517 render/branch pipeline 配線

補助証拠 `cargo test` — synvelis-project `--test sv517_render_wiring` 6/6 pass（store persistence/corruption・journaled publish pending/failed・restore fork-copy/unavailable）、synvelis-gateway `--test sv517_render_actions` 6/6 pass（capability 登録・dispatch parity・typed failure・fail-closed・capability floor・idempotent replay）。E1〜E5 の実測: `CreateNew` race→readback digest 検証、`Published` 非返却の typed `Pending`、per-op permission は宣言 metadata（実効 floor は wire capability + scoped effects）、同一 key+payload→`DuplicateRequest`/異 payload→`IdempotencyConflict` の replay 分類、receipt `copied` の publish 後再計算。mise 正規検証・prek・review-protocol は PR 前実施と記録（synvelis#517）。

## synvelis#518 production job system

補助証拠 `cargo test` — synvelis-render 全 suite pass（`rcp03_render_policy` 30 件、うち新規 async pin 9 件: pol01 mixed-trigger coalescing・duplicate intent idempotency、pol06 concurrency quota・grant-update wake・retryable retry・explicit crash resume・queued cancel typed receipt、bounded queue `QueueFull`、headless fail-closed。l2_e2e 17・recipe 11・artifact 20・gate 7・sf03 2・plugin-state 5・e2e 5・doc test も pass）。synvelis-project 全 suite pass（v15 pin 3 件更新後・inline render_policy round-trip 含む）、synvelis-workspace 全 suite pass、`cargo check` workspace clean（render/workspace/cli/mcp-poc）。IR status は `in-progress` のままで、mise 正規検証（fmt-check/check/lint/test × 対象 package・prek）と review-protocol は PR 前実施と記録。実 `RenderExecutor`/settings/demand resolver の production 実装は未存在で headless host は fail-closed typed rejection のみ返す（render 成功を主張しない）（synvelis#518）。

## synvelis#521 Git LFS TLS transport

`cargo test -p synvelis-library --test lfs` 19/19 Pass（in-process `MockServer`/`MockTlsServer`、実 network 不使用）— trusted CA happy path（batch POST + signed GET + verify）、untrusted CA→`TlsVerification`、native roots が self-signed reject、batch 401→`AuthExpired`、download 403→`PermissionDenied`、TLS redirect follow、cross-origin credential 非付与、handshake 中 deadline→`BudgetExceeded`。`cargo check --all-targets` clean（`--locked` は lock 再生成後に Pass）。plaintext HTTP・file://・SSH/git unsupported は非回帰。実 HTTPS endpoint・実 remote（GitHub LFS 等）・SSH/`git://`・実 OS credential store・GUI・`*-full`/`test-rt`/`check-app-*`/`gw10` lane は NotRun。WebDAV-over-HTTPS は synvelis#90（synvelis#521）。

## synvelis#530 stateless PluginStateDecl

`PluginStateDecl::stateless()` predicate + `verify_plugin` `Unsupported` arm のみの変更で variant/field 追加なし。oracle 追加: `rcp_04.rs` に profile 単体・verdict matrix・selection-level Live/Blocked の 4 case、`rcp_selection_integration.rs` に stateless evidence 経路 + 未 capture fail-closed の 1 case。lease 外ファイル（`rcp07_gate.rs`/`render_ui.rs`/`rcp06_render_ui.rs`）無変更、`Cargo.toml`/`Cargo.lock` 変更なし。EXP-1〜3 棄却（field 追加・variant 追加×2 — 全て lease 外波及または既存語彙と論理同値）、EXP-4 採用。mise 必須 lane（fmt-check・check/lint/test synvelis-player・check/lint/test synvelis-workspace）実行、`check synvelis-render` で `rcp07_gate.rs` literal 非破壊を確認。実機/RT（T3/T4・実 plugin/OS/PCM/RT）は synvelis#109/#296 cross-gate で NotRun（synvelis#530）。

## 採否・凍結・スコープ外化の判断記録

- synvelis#514: session engine selection は「実装不要」（判定分岐 B）で凍結 — rebuild 経路の挙動変更・dep 追加・テスト追加なし。`engine.rs` module doc に意図的未配線と将来の配線 trigger を明記（rollback は doc 削除のみ）。
- synvelis#516: interim mlua 実装の恒久維持と port 語彙の ruviform 型への再 binding を棄却し anti-corruption adapter を採用（ADR 記録）。host binding handler seam・named entry・Resume VM・instruction budget は中立契約に存在せず、追加時は binding の写像層を再評価。
- synvelis#517: `JournaledRuntimePublish` は `Published` を返さない typed `Pending` 設計に確定。engine/session 側の rebuild は synvelis#514 の SessionRoutingEngine 領域。
- synvelis#518: `JobExecutor`（ruviform#744）非採用を ADR に記録 — `JobOrchestrator` が single-writer scheduling authority であり per-job executor は責務重複。
- synvelis#521: WebDAV-over-HTTPS の解放は本 slice 対象外で synvelis#90 に残す。rollback は `lfs.rs` の `for_remote` から `Https` を外し `request` の `WebDavScheme::Https` arm を `UnsupportedCapability` に戻せば LFS 側は完全退避可能。
- synvelis#324: `PluginInstance` change の apply/restore 対応は別 Issue 候補 — 本 slice は apply surface fail-closed を維持。
- synvelis#262: 実 job step・named macro 実解決・approval→apply 継続・durable run journal は外部 gate（synvelis#267・AUTX-04 系）に deferred。
- synvelis#274: 実 Luau.Analysis FFI bridge 切替は ruviform#560・rf#692 gate で、`LuauPortAnalyzer` は port-level honest fallback のみ。
- synvelis#515: 対話存在証明は synvelis#114 系へ deferred — 本 slice は OS user account 束縛まで。macOS/Linux vault backend は Unsupported。
- synvelis#530: `SessionRoutingEngine` 配線は synvelis#514 で実装不要確定済みのため非変更。docs index（`docs/**/README.md`）は Coordinator が統合時に再生成。
- synvelis#365/#368 の audit 残件: `missing-experiment-link` warning 18 件・`broken-relation-link` warning 36 件・exp jsonl `invalid-event-shape` error 52 件は未解消で別 Issue 候補。task status 表記揺れ（`in_progress` 等 3 件・`completed` 3 件）も範囲外として残存。

## 源文書間の矛盾・異常

- synvelis#515 の task 文書は `status: "todo"`・全 checkbox 未チェックのままだが、IR・exp は completed で実装・検証済みと記録している — task 文書の status 更新漏れと思われる文書間 drift。
- synvelis#518 の IR は `status: "in-progress"` で「mise 正規検証・prek・review-protocol は PR 前に実施」と記録する一方、task 文書は全 checkbox checked・`status: done` — IR が最終検証完了前の状態で残っている可能性。
- synvelis#521 の lfs test 数が文書間で食い違う: task は「`cargo test --test lfs` exits 0（22 tests）」、IR/exp は「19/19 Pass」。
- synvelis#286 の IR 本文は「Related Experiments: なし」と記述するが、対応する exp 文書（EXP-1〜6 の採否記録）と exp.jsonl が実在し plan の file map にも列挙され、IR front matter の `metadata.experiments.adopted` にも exp 文書が登録されている — IR の Related Experiments 記述が実態と矛盾（おそらく exp 作成前の記述のまま残ったもの）。
- synvelis#283 の plan では T5（review-protocol）・T6（正規検証・merge・PR）が未チェック `[ ]` だが task 文書では全項目 checked — plan checkbox の更新漏れと思われる。
- synvelis#262 の IR `changes` には `docs/impl/exp/` 文書が列挙されるが、exp 文書側の `implemented-by` は IR を指し、IR 側 `implemented-by` は空 — relations の向きが slice により異なる（synvelis#286 では IR `depends-on` が IR 自身を指す等、front matter の relations 品質にばらつきあり）。

## 出処

この記録は synvelis#262・#274・#283・#286・#324・#365・#368・#514・#515・#516・#517・#518・#521・#530 の各 Issue に基づく。headless CLI 実行配線・script analyze、variant diff gateway exercise・Difference Explorer app wiring、plugin_instances VCS semantic diff、impl index/IR 文書衛生、session engine selection・stateless PluginStateDecl、OS credential session、mlua→ruviform adapter、render/branch pipeline 本番配線、production job system、Git LFS HTTPS/TLS transport の各作業から抽出した。
