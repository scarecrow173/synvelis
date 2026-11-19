# メディア入出力とレンダリング — 実行証拠

この文書は、メディア入出力とレンダリング領域で実施された検証と、その結果・判断を記録する。対象は headless 縦断スライス L1/L2/X（synvelis#24/#25/#26）、Media & Render Lifecycle の設計契約文書化（synvelis#21）、target architecture の設計レビュー（synvelis#22）、および render checkpoint RCP-01〜RCP-07（synvelis#290〜#296）である。計画段階で未実施の検証は実績と混同しないよう明示する。全スライスとも headless のみの検証であり、実機・実 plugin・実 OS・実 GUI の受入は対象外である。

## L1: local WAV import・verified media・headless playback（synvelis#24）

検証構造は、決定的 PCM16 WAV fixture → nonRT staging → size/hash/container/codec/frame/channel verify → measured durability → digest-addressed Verified Blob → ExpectedRevision commit → save/reload → verified resolver → lowering → generic typed graph → deterministic headless playback であり、L1-01〜08 の acceptance を `l1_*` テストに写像した。

結果は `cargo test --workspace --locked` で全 19 suite pass・0 fail、`l1_e2e` 10/10 pass、fmt/check/clippy `-D warnings` すべて clean である。Ruviform lock は `1305538caeab62e6f491a1d9a2497a1f76909d04` であった。

fixture determinism（`wav_pcm16` canonical bytes の sha256）は、mono 960 frames/1964 bytes = `b39236c4064dfddfe2ab92b2486fe3e417428a6cb80887c28e63194016d217ae`、stereo 3000 frames/12044 bytes = `25225bcbb635702e1647154d59b7de773907ffe965a4467b07152ab0c2f1c76f`、stereo 1024 frames/4140 bytes = `aba3fb1fdcb6534809e89964f856cd23d8ad44074e703a720d16493c8c81d042`、mono 300 frames/644 bytes = `6913deafb2282b7dc17f44d04651fc5f473281c7f1338f266778fc860fdf5799` である。seek/replay determinism は `driver.seek(0)` 後の replay `pcm`/`crc` が first run と byte 一致した。failure atomicity は部分 mutation なし・blob/orphan の明示分類を `l1_04_*` で検証し、capability/latency evidence・last-good activation を `l1_06_*`/`l1_compat_*` で検証した。

status は headless のみで、実 OS/device/plugin/GUI 受入は対象外であり `Pass(部分)` 相当の記録である（synvelis#24）。

## L2: master sequential offline PCM WAV export（synvelis#25）

検証構造は、Frozen `RenderSnapshot` → static/canonical preflight → L1 semantic lowering 再利用 + master-bus insert chain → fresh render processors → Ruviform `PreparedRender` + sequential `render()` + `StagedOutput` → 実 io-wav encoder/muxer → 独立 `SourceReader` reopen verify → safe publish → typed outcome であり、L2-01〜08 の acceptance を `l2_*` テストに写像した。

結果は `cargo test --workspace --locked` で 121 pass / 0 fail（L2 suite 15/15、L1 suite 10/10、vs_a–d 全 suite 維持）、fmt/check/clippy `-D warnings`/build すべて clean である。toolchain は `rustc 1.99.0`、Ruviform lock は `322a661ac0177fc98f799b07ae591c98eb2ec845`（ruviform#198 含む）であった。

deterministic fixture/output evidence（同一入力・反復実行で同一 sha256）は、fixture mono 4800 = `23b5e3f4112c32969a25a36f76bce0690e6c8653464130bb708e34`、fixture stereo 4800 = `9ffc2f7b2f5c607d6951da638e4ee7a4ae8801878e165b899e226c5022b9b08e`、export mono 512+64tail = `6dcca5e8ae158324f30655fd68cf6e7900e2f2f9c1e3d556ef26d059b9317ec1`、export stereo = `3409ae06801237f2e09679df9ae4107c39b8c1bd2b453a8662e7df864c9e3eb9`、export 追加 fixture = `6c46aef08c565171254e56a1c9bf99a818a2c7408376f0f14d5b388451bf7648` である。

review は、Reviewer A の P1 2件（`lowered.skipped` 未検査で silent drop 可能、impl record 不在）と P3 6件を修正した。Reviewer B の adversarial 15件のうち確認済み実害（decode 無制限、Delay{0}、tail 整合、TOCTOU、zero-clip、RIFF 8B 固定、magnitude bound、lead overflow、inline comment、codec class 揺れ、cancel gap）はすべて修正済みである。修正しない判断 4件（call-counter 位置導出・default verify structural のみ・publish InvalidValue remap・解析 model 結合）は検証済みのうえ制約として記録した。

未計測・未実施の明示記録は、cancellation は prepare/process phase のみ実測（upstream の drain/finalize/verify/publish cancel point は本層では未注入）、publish-stage の `DestinationExists` 競合は upstream `staged.rs` の再検査に委譲、`wire_apply` model の S16 quantize は clamp のみ、`Estimated` tail は evidence 記述まで、である。status は headless のみで、実 plugin/OS/GUI/実 codec 受入は対象外である（synvelis#25）。

## X: recorded source → generic playback/export（synvelis#26）

検証構造は、mock input → writer/journal → finalize → canonical publish → commit → generic graph → export まで実 symphonia/io-wav 経路・GUI/device 不要の headless E2E であり、X-01..X-05 を `x_*` テストに写像し、X-06 は経路構造で静的立証した。

結果は `mise run test`（`cargo test --workspace --locked`）で全 test binary green / 0 fail — `x_e2e` 5/5（約 1.8s）、`vs_d` 22/22（regression なし）、`l1_e2e` 10/10、`l2_e2e` 15/15、unit/doc tests 含む全 23 suite `ok`。fmt-check/check/lint すべて clean。toolchain は `rustc 1.99.0`、Ruviform lock は `322a661a`（変更なし）であった。

deterministic evidence は、x_01 normal は 48,000×2 frame で playback/export 全 sample == `MockPcm{seed:0}`、blob digest == source.content_digest、x_02 punch-in-loop + pre-roll は 691,950×2 frame 全域で `clip → segment.source_range → session_range → MockPcm input_frame` の bit-exact 対応（3 segments・3 takes・auto-clip、負 tick placement・source offset 含む）、x_04 crash/retry は 24,000×2 frame で `AlreadyApplied` + reload 後 playback bit-exact、である。

fault 分離は、capture-fault marker → publish `capture-fault:` skip + commit recoverable（typed 分離）、engine underrun → zero-fill + `stats.underrun`（content 化しない）、export 既存 destination → `ExportOutcome::Failed` + file 無改変、である。crash 分岐は、crash-before-commit（publish 済みでも未 commit なら source 未登録・resolve 不可）、WIP（`missing-finalized` skip・recoverable・未 commit）、journal 末尾 truncation → `Partial` → publish skip・commit recoverable・未 commit、crash-after-commit+reload → `AlreadyApplied` + playback 継続、`blobs/` は canonical digest のみ、である。

未計測・未実施は、mono/multi-channel(>2) 録音の publish（`UnsupportedChannels` は型付き refuse 済み）、media anchor clip（録音 commit は Musical anchor のみ生成 — L1 側で網羅済み）、export `Estimated` tail・inserts 付き構成（L2 側で網羅済み・本件スコープ外）、である。status は headless のみで、実 device/plugin/OS 受入は対象外である（synvelis#26）。

## Media & Render Lifecycle 設計契約の文書化（synvelis#21）

性質は docs/ 設計契約 + evidence のみの slice であり、実装コード・GUI・実 device/plugin/OS 受入は対象外である。spec/design/plan/task/test-spec/impl-ir/impl-exp の 7 文書鎖 + `evidence/sv21/`（case ledger・verification）を作成した。

結果は、MR-RCP-01・MR-001〜028 の 29 受入ケースを既存 Synvelis/Ruviform fixture・Issue・Receipt へ写像し、27 ケースが headless fixture で `Pass(部分)`、MR-017（stem/bounce）・MR-022（bounce/freeze）・MR-RCP-01 が未実装で `NotRun` である。MR 写像の内訳は、MR-001〜004 は synvelis#24（L1）+ #17/#86 が headless Pass(部分)、MR-005〜010 は #9/#85〜#91（asset-library）が headless Pass(部分)・実 server NotRun、MR-011〜016・018〜020 は #25（L2）+#20 が headless Pass(部分)、MR-017/MR-022 は未割当（#23 配下）で NotRun、MR-021 は #127/#128 が Pass(部分)（mock のみ・実 plugin NotRun）、MR-023 は #114/#23 が Pass(部分)（gateway job+close_veto、unified orchestrator 未実装）、MR-024〜027 は #107 系・#114 が Pass(部分)（headless・実 device NotRun）、MR-028 は #9/#85〜#91 が Pass(部分)（headless・実 credential/SSRF NotRun）、MR-RCP-01 は #290〜#297 + ruviform#289 で NotRun（RCP 未実装 — 当時点）、である。

Ruviform generic capability の対応は、WAV Encode/Mux ruviform#191・Reader/Decoder ruviform#197・Sequential Offline Driver ruviform#198 は closed、Probe/Dock/Clock/UI data（ruviform#181/#182/#184/#185/#186/#187/#188/#189/#190/#196/#282/#308/#309）は closed、Generic job/cancellation・bounded data（ruviform#253/#275/#294）は open、である。

検証は `mise run fmt-check`、`doc_status.js lint --type {spec,design,plan,task,test-spec}`、`doc_maintenance.js index apply`、`audit_impl_record.js`/`audit_experiment_log.js --json`、`audit_docs.js --type all --json`、`route_graph.js --focus media-render --json`、`review-protocol` を実行し verification に記録した。

ステータス不整合（要報告）は、本 slice の task 文書 front matter が `status: "in-progress"` のままだが対応する impl-ir/impl-exp は `status: "completed"` で文書鎖完了を記述している点であり、task 側の status 更新が追いついていないか PR/worker_done 未了を意味するかは文書からは判別不能である（synvelis#21）。

## RCP-01: Canonical Render Recipe（synvelis#290）

性質は headless 実装スライスで、`synvelis-project`（Semantic ID/snapshot read adapter）+ `synvelis-render`（Recipe/Dependency Resolver）に canonical `RenderRecipe` モデル + typed dependency closure + temporal invalidation を実装した。Artifact Store/GC/remote（RCP-02）・Render Job/Policy（RCP-03）・実行経路選択（RCP-04）・workspace/Gateway projection・実 Plugin/実 OS Gate は後続 Issue の範囲である。

結果は `tests/rcp01_recipe.rs` 11 fixture（RCP-REC-01..06 網羅）を含む全 suite green で、`mise run fmt-check`/`check synvelis-project`/`check synvelis-render`/`lint synvelis-project`/`lint synvelis-render`/`test synvelis-project`/`test synvelis-render` 全て Pass である。

fixture が固定した契約は、display rename・挿入順変更で key 不変、non-finite・schema skew は fail-closed、closure が signal path・send/return・sidechain・automation を含み無関係 track 追加で key 不変、lookahead/PDC/tail/loop/seek の各 temporal 拡張と未知 extent・temporal-domain の全域失効、shared edit が全 branch の key を動かし branch 内部編集は当該 branch のみ、digest 改竄・version skew の record は `Unknown`、である。

採用判断（exp）は、branch identity は provenance のみ（`RecipeBranchPin` を scope/shared-context digest の 2 要素に限定）、scope digest は canonical name-free fingerprint を別計算、shared_context_digest は補集合、branch 合成は既存 `plan_adopt_writes` + `entity::write_all` を再利用、non-finite は reject、temporal invalidation は over-approximate、record verify は canonical bytes からの key 再計算、である（synvelis#290）。

## RCP-02: Immutable Artifact Manifest・Verified Publish・GC（synvelis#291）

性質は headless 実装スライスで、`synvelis-render::artifact` module を新設した。実 OS/filesystem store・実 Plugin・RT lanes・VCS integration（RCP-05）・cross-gate（RCP-07）は後続 Issue/Gate の範囲である。

結果は `tests/rcp02_artifact.rs` 20 fixture（RCP-STORE-01..06 網羅）で、`mise run fmt-check`/`check synvelis-render`/`lint synvelis-render`（clippy `-D warnings`）/`test synvelis-render`/`check synvelis-project`/`check synvelis-player` 全て Pass である。suite 内訳は l2_e2e 17, rcd_04 2, rcp01_recipe 11, rcp02_artifact 20, sf03 2, tim04 5, x_e2e 5（全 green）である。

fixture が固定した契約は、同 recipe 異 PCM は別 artifact ID、同一 blob digest を共有する 2 artifact が manifest ID で区別され index に両方残る、古い expected_index の commit が `Conflict` で reject され先行 artifact 保持、corrupt readback → `PublishStage::Readback` の `HashMismatch` で旧 verified artifact の index/blob/manifest 不変、全 FaultPoint が型付き error で失敗し partial state を残さない、lossy→Preview・PreviewOnly→Preview・Unknown repro→Unknown・incomplete receipt→Unverified・denied ACL→Denied、span/port/profile 不足で `EligibleExact` にしない + `FitRejection` evidence、remote の BlobMissing/BlobMismatch/ManifestInvalid/AccessDenied/ClaimMismatch/BlobInvalid（receipt 嘘）reject、remote verified 登録の冪等（Registered→AlreadyPresent）、GC の grace/tombstone/reference lifecycle/Pending・external root conservative/shared blob refcount/quarantine、external manifest attach で commit SHA/manifest bytes 不変 + unsafe locator reject + 重複 attach 冪等 + detach で GC 対象、である。

review 是正（exp E8）は、codec class↔codec identity・port channel tiling・per-port PCM digest の検証が publish のみ存在し commit/remote/lookup が publish 迂回 manifest を受理しうる gap を、manifest-domain 共有判定として集約し `register_remote` は blob 実測で全照合（`BlobInvalid`）、`lookup` は manifest 自己申告レベルで `Corrupt` 判定に修正したものである。`semantic_contradiction`（Lossless+mp3・Lossy+pcm・非 tiling ports）と `remote_media_contradiction`（codec class/frame count/port tiling/forged port digest/forged chunk digest/receipt flag 偽がすべて `BlobInvalid`）で検証し、既存 18 fixture はすべて green 維持である。

status は headless のみで、実 filesystem/OS atomicity・fsync・ACL・disk-full semantics は未検証であり、実 OS/storage Gate の PASS に昇格しない（synvelis#291）。

## RCP-03: Render Policy・Trigger・Async Job・Completeness（synvelis#292）

性質は headless 実装スライスで、`synvelis-render` に `policy/`（model・trigger）と `render_job/`（model・orchestrator）を閉じて実装した。実 OS/VCS/async/RT/Gateway/GPUI は対象外である。

結果は `tests/rcp03_render_policy.rs` 21 fixture（RCP-POL-01..07）で、`mise run test synvelis-render` で test 83 件・うち rcp03 21/21 green、`check synvelis-project`・`check synvelis-player` も Pass である。docs index 再生成 + `doc_status lint`（範囲内 finding 0）+ `audit_impl_record`（範囲内 finding 0）を実施した。review-protocol adversarial pass で attach 失敗の無記録破棄・receipt 遷移の逆行/欠落を修正した（詳細は当該 Issue の verification 記録）。

fixture が固定した契約は、3 層 PolicyPatch 解決 + per-field FieldProvenance + resolution_digest、policy 変更は新 job identity だが RecipeKey 不変・artifact は lookup reuse で二重 publish しない、job key に action+grant を含め entry surface を跨いだ coalesce と grant 分離、completeness が job 終端状態と直交（lookup 結果から独立評価・Share gate が BestEffort/RequireComplete を適用）、crash が typed executor outcome + CrashObserved receipt + resume(key) 冪等、OnDemand は authorizer 非設定で `Denied`（fail closed）、terminal job への同一 key 重複 event は coalesce（`Coalesced` 記録）、retryable worker failure は pending 理由記録で Queue に戻すが quota/storage gate は retry() bypass しない、である。

status は headless のみ（同期 scheduler 等価物）で、実 OS/VCS/Job Gate の PASS に昇格しない（synvelis#292）。

## RCP-04: Missing Plugin Subgraph Resolver（synvelis#293）

性質は headless 実装スライスで、`crates/synvelis-player` に `rcp` module を新設した。`synvelis-workspace` semantic→selection integration は別 Worker lease のため deferred residual である。

結果は `tests/rcp_04.rs` 26/26 deterministic oracle test（RCP-EX-01..05 の headless 検証可能分）で、`mise run fmt-check`/`check synvelis-player`/`lint synvelis-player`（warning 0）/`test synvelis-player`/`check synvelis-render`/`check synvelis-app` 全て Pass である。OS は Windows、toolchain は mise 管理 Rust 1.99、Ruviform `fd1d4d5`（変更なし）であった。

fixture が固定した契約は、plugin identity drift・availability・state restore・port layout・host capability の順序付き検証、cutset の sink/unknown node・全 output boundary の artifact output 被覆・domain/format/channel/latency(PDC) 整合・control 誤配送・boundary input pinned 要件・sidechain 区別・required live input feed 維持・optional `min==0` 許容・dangling/double-feed/multi-output 網羅、deterministic greedy set-cover（correctness gate 通過後に covered 数→region サイズ→artifact id で安定化）、edit 分類（NoRebuild/ControlOnly/Rebuild）と単調 generation の plan prepare、publish 前の project/branch revision・capability generation・graph fingerprint・engine/source generation・clock epoch・runtime instance 再検証、stale/superseded 拒否、Published と RT 観測 Active の分離、QueueFull/Retire/restore failure/epoch drift での last-good 保持、旧 generation への param/automation delivery gating、である。

採用/棄却（exp）は、`edge_ok` による covered_sources 限定記録・optional `min==0` 入力の dangling 判定除外・`LayoutKey` による決定的 port layout 比較を採用した。「preview + Unknown plugin state を Blocked に倒す」初期期待は棄却し NonEquivalent+skip の fail-closed 設計を採用した（テスト期待を実装契約に合わせて修正）。

status は headless のみで、実 plugin の多様な port 構成での網羅は実機 Gate（synvelis#296）依存である。RCP-EX-06 実機 Gate（PCM・実 Plugin・実 OS・実 RT）は NotRun である（synvelis#293）。

## RCP-06: Missing Plugin/Render Availability・Policy UI・Action 契約（synvelis#295）

性質は headless-first 投影スライスで、authority は RCP-01..04（recipe/lookup、`ExecutionSelector`、render policy/job orchestrator）の既存契約を消費するのみで、第二正本・再判定・独自 Gateway は作らない。

実装は `synvelis-workspace::render_ui`（presentation model 正本・5 軸 enum・RenderReason 語彙・policy/job/request/branch-diff projection・TriggerReport 単一 lowering・view-local RenderViewState/RenderViewAction）+ `synvelis-app::render`（AUTX-07 パターンの driver-neutral ViewSpec 層・decode_render_command 単一 decode・executable candidate のみ button 化）+ `synvelis-theme::status`（5 軸 + job state label の StatusTone mapping、未知 label は Unknown に fail-closed）である。

結果は headless/Fake-driver の contract test が Pass（当該 Issue の evidence 記録）である。実 GPUI/実 OS/IME/DPI/2Window/pixel/RT lane（RCP-UX-06）は Case ID 付き NotRun/Deferred であり、CLI/Workflow/Luau/Rhai/Fake AI の実 surface は synvelis#273 他 lease で deferred、本スライスは共有 action id 語彙と TriggerReport lowering の contract test のみである。cross-gate RCP-07・adopt（RCP-05 scope）は対象外である。

発見した問題と修正は、`render_validity_of_lookup` が `EligiblePreview` を `Valid` に写像していた bug を `Preview` に修正（両経路の語彙一致）、`ToggleTrigger` の draft 初期化が Manual を除外し patch 化で Manual が失われる不具合を修正（draft は全 trigger を含み Manual off は guard で拒否）、である。status は headless projection のみで、実 GUI/OS 受入は NotRun/Deferred で昇格しない（synvelis#295）。

## RCP-07: cross-gate fixture/ledger（synvelis#296）

性質は fixture/ledger slice で、merge 済み RCP-01..05 基盤をまたぐ 7 つの headless cross-gate fixture（`synvelis-render` の `rcp07_gate` test、約 1700 行）と、全 case-ID の runnability を正直に記録する acceptance ledger・verification・test-spec・IR/exp を追加した。production code・既存 test・canonical doc は一切変更していない。

fixture 内容は、GATE-01 recipe→cache identity・false-valid 0、GATE-02 closure-scoped invalidation、GATE-03 verified→fault/GC/旧 Verified 保護・raw 不変、GATE-04 policy 認可→job 副作用 0・coalesce・cancel、GATE-05 branch edit→key 分離→adopt 単一 CAS→独立 oracle key 一致、GATE-06 store lookup verdict→resolver candidate の Exact substitution/fail-closed、GATE-07 job key/recipe key 分離と旧 key verified 保持、であり、すべて公開 API のみを使う。

結果は `cargo test --test rcp07_gate` 初回 5/7（gate04 cancel false — `handle` 同期 drive が原因、gate05 key 不一致 — branch pin hashed が原因）→ 修正後 7/7 green（exit 0）。`mise run lint synvelis-render` 初回 5 error（assert_is_empty×3・used_underscore_binding・match_wildcard）→ 修正後 clean。`mise run check synvelis-render` Pass（all-targets）である。

ledger は GATE-01..12 全 case の status・所有 gate・fixture 対応・NotRun/Deferred/Blocked/Unsupported 一覧・判定規則（実測なし Pass=0）を記録した。GATE-08..10（実 Window/GPUI・実 plugin ABI・実 PCM clock/consumer lane）は fixture を作らず ledger 分類記録のみである。GATE-05 の RCP-05 未整備部は synvelis#294 完了待ちとして Blocked 記録である（RCP-05 #294 は branch-pin plumbing のみ merge 済みで専用 test-spec 未整備、pin 可能な範囲のみ被覆）。

判断は、`synvelis-app` test は追加しない（依存境界と既存 fixture 被覆から proportionate）、`#[ignore]`/stub による GATE-08..10 疑似配置は実行証拠の捏造になるため却下、既存 rcp01..06 fixture helper の共有化は test file 間 helper 共有機構が存在せず contract 独立性のため却下、である。status は headless fixture + ledger のみで、実機/consumer gate 自体は NON-GOAL であり、headless Pass を実機受入へ誤昇格しない（synvelis#296）。

## Target architecture 設計レビュー（synvelis#22）

性質は docs/evidence-only slice で、コード・manifest・lockfile・CI・本番 crate は一切変更していない。TA-001〜012 + GUI 採用方針 2 条件を現行コード・依存 DAG・参照 Issue と突合する設計レビューの docs chain 7 文書 + evidence/sv22 台帳（case-ledger・dependency-dag）を作成した。

実測結果は、`cargo metadata --format-version 1 --locked --offline` + node DFS で normal-dep cycle = 0（Synvelis resolve 全体 + Ruviform workspace `fd1d4d5`・42 member）。dev/build 辺の cycle も観測上 0 で、Synvelis→Ruviform 一方向・Ruviform→Consumer 依存なし、plugin-host→audio の dev-dep 辺は normal 辺としては存在せず TA-005 の境界を満たす、である。

`mise run check-ruviform-rev` は初回 fail — vendored UI revision `20a24a66…` が Cargo.lock resolved `fd1d4d5…` と不一致（synvelis#230 の既知 stale vendor 事象）。`mise run sync-ui-deps` で `fd1d4d5` へ再同期後に再実行し Pass（27 lockfile sources、REV match）である。`ui/vendor/ruviform` は gitignore された生成物でコミット対象外である。`mise run fmt-check` は exit 0、`mise run check-headless-deps` は Pass（10 headless crate に GUI framework deps 非含有）である。

型漏出走査（TA-002）は、Ruviform resolved rev の `src/` 全域を Synvelis/DAW 意味論型名で grep し、Synvelis crate 依存・Project/Track/Take/Comp/Automation/Undo/RenderRecipe 型の公開 API 侵入なしを確認した。`ruviform-ui-theme/src/generated.rs` の `pub mod synvelis_dark` と doc 例示の `synvelis::` 言及を検出したが DAW 型ではなく consumer 名 theme データ/doc のため P2 観測として分離した。

GUI 方針照合（C13/C14）は、`sv175-native-shell-driver-neutral-gpui.md` = accepted、`supersedes` で sv-gui-01 を明示（sv-gui-01 = superseded）、Ruviform#384 ADR = proposed（仮決定）で #175 と方向一致だが最終採用は ruviform#262/#388/#389 実機証拠後、plugin GUI embedding の ADR（wry `build_as_child`）= accepted だが旧経路で ruviform#389 で再設計予定、`synvelis-app` の `native`/`webview`/`gpui-test` feature が GPUI/WebView/theme を optional dep として分離、C13 = GUI policy 整合 Pass、C14 = Native Dock 実機 Gate Deferred（ruviform#388/#389/#262、synvelis#176 に接続）に分離 —「採用方針の整合」と「GPUI 最終採用の PASS」を混同しない、である。

証拠分類の境界は、ruviform fixture（rt_*/poc_ex_*/g2_*/roundtrip_*/staged_*/multi_window/close_veto 等）の存在をソース pin（契約実在 = Pass）し、実走は別 repo・build 対象外のため NotRun（先行 slice/CI の SHA 付き記録を参照）に分離した。alloc/block・実 OS/Plugin/GUI の実測を Pass にしない。Audio/Video/Plugin/CLI/UI/Synvelis/AudioLens を別責務・別 Suite・別 NotRun 根拠で記録し、AudioLens は Unsupported→ruviform#219/#294・audio-lens#127 Deferred とした。

発見した問題（範囲外・記録のみ）は、route graph blocked（pre-existing broken relations が原因・修復せず）、v0.7.1「現行 28 crate」記述 vs `fd1d4d5` の 42 member（M1〜M7 で増加した文書ドリフト・責務分担は整合 → P2 観測）、PPQN 範囲差（Ruviform `1..=65535` vs Synvelis `1..=32767` = consumer が厳しい部分集合で整合 → P3 記録）、legacy window close=終了 vs multi-window close≠Job Cancel = 別契約（TA-011 で区別記録）、である。

未実行の検証（NotRun/Deferred）は、`mise run check`/`test`/`test-rt`/`lint`（package 引数必須・docs slice で crate 変更なし）、ruviform fixture 実走（別 repo）、実 OS Device/実 Plugin/実 Native Dock/実 GUI（synvelis#176、ruviform#262/#388/#389/#294 に Deferred）、AudioLens 側証拠（別 repo）、merge 後対象 SHA の CI 証拠（Coordinator が確認）、である（synvelis#22）。

## 横断的な判断・既知の制限・文書不整合

Deferred/Blocked/Unsupported の記録は、stem/bounce/freeze・render-in-place（synvelis#23 配下・MR-017/022・NotRun）、unified job orchestrator（synvelis#114/#23・MR-023 は Pass(部分) 止まり）、RCP-05（synvelis#294 — branch-pin plumbing のみ merge、残り Blocked）、実 plugin/OS/GUI/RT/consumer lane（ledger のみ・NotRun/Deferred/Blocked/Unsupported）、AudioLens consumer（ruviform#294/audio-lens#127 Deferred）、asset-library 実 provider/preview（synvelis#9/#85〜#91・MR-005〜010 は headless Pass(部分)・実 server NotRun）、SRC/dither 実装（synvelis#20・MR-018 は Pass(部分)）、`synvelis-workspace` semantic→selection integration（別 Worker lease の deferred residual）、RCP-UX-06 実 GPUI/実 OS Gate、CLI/Workflow/Luau/Rhai/Fake AI 実 surface（synvelis#273 他 lease）、である。

既知の制限（IR/exp に明示記録）は、`ClipRenderer` の位置は call counter 由来で `RtContext` に transport position API がない（非 contiguous drive で authoritative position と乖離し得る — upstream 提案候補）、default verify は structural のみで `expect_pcm` opt-in（`pcm_checked` flag が正直に区別）、`StagedOutput::publish` の `InvalidValue → DestinationExists` remap は ruviform variant 選択（Synvelis 側で重複実装せず報告対象）、`SourceReader::hold_frame` は packet gap で landing frame 長に clamp、stereo→mono downmix は channel-0 select で sum 不可、L2 cancellation は prepare/process phase のみ実測（drain/finalize/verify/publish は upstream hook 未注入）、`wire_apply` S16 quantize は clamp のみ、`Estimated` tail は evidence 記述まで、`PrefetchHandle::reopen` は position 0 再 anchor のみで mid-stream は reopen→seek 2 段、seek の quiesce drain は 1 block 分の追加 process を要し RT 適用時は非 RT 文脈での先行が必要、resync 中は 1 frame probe で block あたり充填効率が落ちる（ring=8・block=256 の headless 条件では実害なし）、RCP-01 の canonical entity 写像は手書きで新 kind 追加時に漏れリスク、RCP-02 の `InMemoryArtifactStore` は契約参照実装で実 filesystem/OS semantics 未検証、lookup は caller が渡す `RecipeResolution` の正しさを前提（防御は RCP-01 側責務）、GC grace は store logical tick 基準で実 lease 供給は後続 slice、external manifest registry は locator 安全性のみ検証で実在性・VCS provider 結合は RCP-05 責務、RCP-03 の `JobCostEstimate` は caller 宣言（default=0）で cost model は後続、RCP-04 の `RenderCandidateIndex` eligibility 分類は synvelis#290〜#292 正本実装を仮定した入力 contract、RCP-06 の authority 語彙は display 文字列到達で第二正本なし、`canonical.json` receipt は session-dir persist のため staging area を消すと publish は再 encode が必要（blob が残れば digest 一致で reused 相当）、である。

文書上の不整合・注意点（矛盾をそのまま記録）は、media-render の task 文書は `status: "in-progress"` だが impl-ir/exp は `completed`（status 不整合）、L1 の 5 task 文書は plan への stub 参照のみで個別の作業記述を持たない、L2 plan の file map は `output.rs` を挙げるが IR の added files に `output.rs` はなく `export.rs`/`verify.rs`/`job.rs` 構成（命名ドリフトの可能性）、L2 の l2_e2e は 15 テストと記録されるが RCP-02 期の IR validation では `l2_e2e 17` と記録（後続 slice でテスト追加された可能性・時系列差）、X plan の goal は「X-01..X-07 acceptance」と記すが test-spec/tasks/IR は X-01..X-06（X-06 は静的立証）を対象とし「X-07」は本 corpus 内で定義が見当たらない、exp/rcp-06 の front matter は `type: "impl"` だが impl/exp ディレクトリの exp 側に配置（メタデータの揺れ）、RCP-05（synvelis#294）は本 corpus に専用文書がなく RCP-07 のみが部分 merge 済み plumbing として言及、L1 plan は ruviform pin `c2a2f07` をリスクとして記すが IR は `1305538c` を記録（時系列の異なる解決結果と思われるが明記なし）、である。

## 出処

synvelis#21（Media & Render Lifecycle 設計契約）、synvelis#22（target architecture 設計レビュー）、synvelis#24/#25/#26（L1/L2/X headless 縦断スライス）、synvelis#290〜#296（RCP-01〜RCP-07 render checkpoint 実装）、および関連する upstream Issue（ruviform#181〜#198/#219/#253/#275/#282/#284〜#294/#308/#309/#384/#388/#389、audio-lens#127 等）。
