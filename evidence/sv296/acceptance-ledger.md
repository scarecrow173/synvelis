# Acceptance Ledger — synvelis#296 RCP-07 cross-gate fixture/ledger slice

Date: 2026-11-15. Slice: RCP-GATE-01..12 の case-ID inventory +
Case→Fixture→Oracle→Result 台帳 + merged foundations 上の
runnable headless subset（`crates/synvelis-render/tests/
rcp07_gate.rs` 7 case）。実 Plugin/実 OS/実 GUI/RT/Consumer
受入は本 slice の対象外であり、headless Pass をそれらへ
昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/296
（長期 tracker・OPEN・`Refs` のみ・Close しない）。
親 spec: `docs/specs/render-checkpoint.md`（RCP-GATE-01..12
位置付け :640-650）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `test/rcp07-cross-gate` HEAD（ledger は同 commit 内。base `origin/main` = merge 取り込み済み） |
| OS / CPU | Windows 11 (Git Bash) |
| Toolchain | mise 管理 Rust（`mise run` 経由。version は verification.md 実測） |
| Ruviform rev | `Cargo.lock` 解決 revision（`fd1d4d5` 系、git 依存・lock 記録値） |
| doc graph route | `doc-driven-dev-graph` route は既存の unrelated broken relations で `blocked`/`focus-required` を返し得るため、RCP-03/RCP-06/AUTX-07 と同じ標準 doc chain（test-specs→impl/ir→impl/exp→evidence）で進行。relation 修復は本 lease 範囲外 |

## 判定語彙（Issue #296 検証成果物 schema 準拠）

- **Pass (headless fixture)**: `mise run test synvelis-render`
  で `rcp07_gate` が green、または既存 fixture（rcp01..06・
  cb01..06・sf06・rt contract）が当該 case を被覆済みである
  ことを本 slice が確認した場合。実測は verification.md。
- **NotRun**: 本 slice で実行していない lane（実 plugin/OS/
  GUI/RT/consumer/3OS）。所有 Issue・gate を明記する。
- **Deferred**: 前提 Issue 未完了により後続 slice へ送る部分
  （RCP-05 #294 未整備部等）。
- **Blocked**: 前提となる test-spec/fixture が存在せず本 slice
  では pin 不能な部分（#294 完了待ち）。
- **Unsupported**: 環境/能力が非対応で実行不能な lane。
- **Fail**: fixture が contract 違反を検出した場合（現時点 0 件）。

**判定規則**: 実測（コマンド・SHA・toolchain・OS・exit code）
のない Pass を記録しない。headless/Fake/GPUI-simulated の
成功を実 Plugin・実 OS device・実 Window・consumer 受入へ
換算しない。数値 SLO は実測と別承認であり、本 ledger は
未測定を Success と記録しない。

## RCP-GATE-01..12 inventory

凡例 — Oracle 種別: 独立 oracle（実装出力を期待値に使わない
独立導出）/ assessment（契約レベル判定）/ 外部 oracle 必須。
Runnability: Runnable headless now / Blocked on unmerged
Issue / NotRun external gate / Deferred。

| Case ID | 内容（Issue #296 受入条件） | Oracle 種別 | Runnability | 本 slice の fixture/証拠 | status |
| --- | --- | --- | --- | --- | --- |
| RCP-GATE-01 | 全 Recipe normal/unknown/canonical version/property-based test が positive/negative fixture を持ち False-Valid 0 | 独立 oracle | Runnable headless now | `rcp07_gate01_recipe_key_is_cache_identity_and_false_valid_zero`（実 project 再解決・recipe_record round-trip・改竄→Unknown・空 store→Missing）。既存被覆: `rcp01_recipe.rs` rec01..06 | **Pass (headless)** — false-valid 0 を本 fixture で確認。property-based 網羅は RCP-01 既存 fixture の範囲、本 slice は cross-gate 接合部のみ |
| RCP-GATE-02 | Temporal interval/Sidechain/Automation/Loop/Infinite Tail で無効化不足 0 | 独立 oracle | Runnable headless now | `rcp07_gate02_closure_invalidation_is_sound_and_complete`（closure 内/外・interval None・temporal→full）。既存被覆: `rcp01_recipe.rs` rec04（sidechain/automation/loop/tail 個別） | **Pass (headless)** — under-approximation 0 を接合部で確認。個別 temporal kind の網羅は RCP-01 fixture が保持 |
| RCP-GATE-03 | Artifact Verified publish/CAS GC/remote corrupt/ACL/lease fault で旧 Verified 保護・Raw 不変・独立 PCM reader readback | 独立 oracle + 外部 oracle（実 PCM readback） | Runnable headless now（manifest/digest 検証レベルまで）/ 実 PCM readback は NotRun | `rcp07_gate03_verified_artifact_survives_faults_and_gc_roots`（publish→fault→旧 Verified 無傷・LiveBranch GC root・release→collect・raw source 不変・quarantine→non-Exact）。既存被覆: `rcp02_artifact.rs` rcp_store_01..06 | **Pass (headless)** — 実 PCM reader readback・実 remote corrupt・実 ACL/lease は外部 gate（`scarecrow173/ruviform#628`・#630 系）で NotRun |
| RCP-GATE-04 | Save/Commit/Share BestEffort/RequireComplete・Manual/OnDemand 認可/Idempotence/Cancel/Retry で無許可副作用 0 | 独立 oracle | Runnable headless now | `rcp07_gate04_ondemand_denied_manual_idempotent_cancel_clean`（OnDemand Denied→job/executor 0・manual coalesce・queued cancel→Cancelled/artifacts 空）。既存被覆: `rcp03_render_policy.rs` pol01..07 | **Pass (headless)** — Save/Commit/Share の trigger 別契約は RCP-03 fixture が保持。本 fixture は cross-gate（認可→job→artifact 副作用 0） |
| RCP-GATE-05 | 2人 Missing Plugin 編集・非RT renderer 協業・late Attach・Adopt/Undo/Reload/Old Commit pin が再現可能 | 独立 oracle | **Partial**: merge 済み plumbing で pin 可能な範囲は Runnable、RCP-05 専用 test-spec/fixture 未整備部は Blocked（#294） | `rcp07_gate05_branch_edit_moves_key_adopt_pins_live`（fork→branch edit→key 分離・candidate key 配下 verified artifact・adopt 単一 CAS・独立 oracle key 一致・LiveBranch artifact adopt 後 reachable）。既存被覆: `rcp01_recipe.rs` rec05、`cb01_variant.rs` adopt/fork | **Pass (headless, partial)** — 2人協業・非RT renderer・late attach タイミング・Undo/Reload・old commit pin 完全 fixture 語彙は #294 完了待ち **Blocked** |
| RCP-GATE-06 | MainPlayback/EditingPreview/BranchAudition/Offline/Export の正確性/Preview/Unavailable 表示と cutset validation | 独立 oracle + assessment | Runnable headless now（resolver×projection の headless 接合部） | `rcp07_gate06_store_eligibility_drives_resolver_selection`（lookup verdict→RenderCandidate→Exact substitution・quarantine→non-Exact）。既存被覆: `rcp_04.rs` ex01..（intent 別 verdict/cutset）・`rcp06_render.rs`/`rcp06_render_ui.rs`（5軸投影・ViewSpec） | **Pass (headless)** — presentation 層の 5 mode 表示は RCP-06 fixture が保持。app 側 cross-gate fixture は依存境界上追加せず（test-spec Non-goals に判断記録） |
| RCP-GATE-07 | RT invalidated stale R1→R3・QueueFull/Retirement/Clock/Plugin restore failure で誤 Activation・誤 Control 配送・RT I/O が 0 | 独立 oracle（headless contract）+ 外部 oracle（実 RT 計測） | Runnable headless now（contract level）/ 実 RT 計測は NotRun | `rcp07_gate07_job_keys_separate_stale_and_current_recipes`（R1/R2 別 job key・別 artifact・R1 key で引き続き EligibleExact = stale 誤 activation 0 の headless 証拠）。既存被覆: `mise run test-rt`（workspace rt_ contract）・`rcp_04.rs` reconcile/cutset stale 拒否 | **Pass (headless contract)** — RT no-alloc/lock/IO・PDC 実測・実 clock は NotRun（`scarecrow173/ruviform#628`・#631 系）。数値 SLO は実測と別承認 |
| RCP-GATE-08 | GPUI Fake vs Native OS を分け、Missing Plugin/Last-good/Policy/Branch Difference の Semantic Oracle を独立記録 | 外部 oracle 必須（Native OS）+ 独立 oracle（headless 語彙） | headless fake 側のみ Runnable / Native OS は NotRun | 既存被覆: `rcp06_render_ui.rs`/`rcp06_render.rs`（5軸・LastGoodMismatch・BranchDiff 投影の headless oracle）。本 slice は新規 fixture を追加しない（GATE-06/08 の resolver→projection 両端は既存 fixture が保持） | **Pass (headless fake) / NotRun (Native OS)** — 実 Window/GPUI runner は `scarecrow173/ruviform#631`・実 OS GUI は synvelis#114/#176 |
| RCP-GATE-09 | VST3/CLAP/AU 実 Plugin OS 別の不確定 State/Non-determinism を Pass/NotRun/Unsupported 分離、bit-exact を偽装しない | 外部 oracle 必須 | **NotRun**（本 slice では分類様式のみ固定） | ledger 本表の status 語彙が分類様式。fixture stub・`#[ignore]` 疑似配置は作らない（実行しない証拠を fixture で偽装しない） | **NotRun** — 実 Plugin ABI は `scarecrow173/ruviform#630`・synvelis#176。3OS 横断は Daily/手動 Full CI |
| RCP-GATE-10 | Ruviform 候補 resolved SHA の Synvelis f32 RT と AudioLens f64/長尺/CLI/GUI が別 Oracle で PASS または理由付き未達 | 外部 oracle 必須 | **NotRun** | resolved SHA は `Cargo.lock` 記録値（`fd1d4d5` 系）として参照。本 slice は SHA 固定・oracle 分離の様式のみ | **NotRun** — AudioLens consumer は `scarecrow173/ruviform#294`・`scarecrow173/audio-lens#127`。Synvelis f32 RT 実測は `scarecrow173/ruviform#628` |
| RCP-GATE-11 | Gate A 既存基本 DAW 制作/録音/Playback は本機構未完成でも独立評価可能・新 per-PR Full CI 不要 | assessment | Runnable headless now（非回帰証拠） | 本 slice は既存 test を改変せず `mise run test synvelis-render`・`test synvelis-player`・`test-rt`・`check-headless-deps` 全緑に乗る（verification.md 実測）。新規 workflow/per-PR Full CI は追加していない | **Pass (headless 非回帰)** — 実機 DAW Gate A は既存 lane の責務、本 slice は依存しない |
| RCP-GATE-12 | Case→Fixture→Expected/Actual→Revision/OS/Plugin/Backend→Receipt→PR/Issue リンクが追跡でき、実測無しの Pass が 0 | assessment（ledger 自体が成果物） | Runnable headless now | 本 ledger の表（Case→fixture→status→所有 gate）+ verification.md（SHA/toolchain/OS/exit code）+ test-spec の rule 対応。PR/Issue リンクは PR 本文 `Refs #296` | **Pass (headless)** — 実測なし Pass=0 の判定規則を上記に明文化 |

## Case→Fixture→Expected/Actual 詳細（headless Pass 分）

### RCP-GATE-01 — `rcp07_gate01_recipe_key_is_cache_identity_and_false_valid_zero`

- Expected: 同一 project の `resolve_live` 再実行で `RecipeKey`
  一致。`recipe_record`→`evaluate_validity` が独立再解決に対し
  `Match`。tempo 追加で `Stale`。canonical bytes 改竄で
  `verify()` Err→`Unknown`。空 store の `lookup` が `Missing`。
- Actual: 全 assert 成立（verification.md 実測 exit 0）。
- Oracle: 独立再解決（実装出力を期待値に再利用しない）+
  contract enum の直接比較。
- Repo resolved SHA / OS / backend: verification.md の HEAD・
  Windows 11・headless（plugin なし・`NoPluginStates`/
  `NoExternalAssets`）。
- Issue link: synvelis#296 RCP-GATE-01。

### RCP-GATE-02 — `rcp07_gate02_closure_invalidation_is_sound_and_complete`

- Expected: closure 内 clip 変更（既知 interval）で
  `!full`・`affected_objects` 含有・dirty interval 非空。
  `interval: None`・`temporal: true` で `full`。foreign object
  で dirty interval 空・`!full`。
- Actual: 全 assert 成立。
- Oracle: `classify_invalidation` の plan を contract field で
  直接検証（出力の再利用なし）。
- Issue link: synvelis#296 RCP-GATE-02。

### RCP-GATE-03 — `rcp07_gate03_verified_artifact_survives_faults_and_gc_roots`

- Expected: 実 recipe key 配下の publish が `EligibleExact`。
  `FaultPoint::CommitIndex` 注入で 2 件目 publish 失敗・1 件目は
  引き続き `EligibleExact`。`LiveBranch` ref で `advance_clock
  (100_000)` + `sweep(grace 0)` でも `kept_reachable`。release
  → `collected`・tombstone・manifest/blob/index 消失。
  `c.live().sources[..].content_digest` 不変（raw immutability）。
  quarantine・収集後 lookup は `EligibleExact` でない。
- Actual: 全 assert 成立。
- Oracle: `InMemoryArtifactStore` の observable state
  （manifest/blob/index/tombstone/report）を直接検証。
- Issue link: synvelis#296 RCP-GATE-03（実 PCM readback・実
  remote/ACL/lease は外部 gate）。

### RCP-GATE-04 — `rcp07_gate04_ondemand_denied_manual_idempotent_cancel_clean`

- Expected: `TriggerEvent::Demand` + authorizer `Denied` →
  `TriggerOutcome::Denied`・`job_keys` 空・executor calls 0。
  同一 manual event 2 連続 → 2 件目 `coalesced`・`job_keys` 1・
  artifact 1。`submit`+`cancel`（queued）→ state `Cancelled`・
  `artifacts` 空・executor calls 合計 1。
- Actual: 全 assert 成立。
- Oracle: orchestrator の公開 receipt/state/store index を直接
  検証（副作用の実測カウント）。
- Issue link: synvelis#296 RCP-GATE-04。

### RCP-GATE-05 — `rcp07_gate05_branch_edit_moves_key_adopt_pins_live`

- Expected: `fork_variant_set(AudioClip)` → candidate へ
  `SetClipMuted` edit → `resolve_branch` key が live key と相違。
  candidate key 配下 publish → `EligibleExact`。`adopt_branch`
  → `project.committed`・`after_revision > before_revision`。
  adopt 後 live key が**独立構成した muted live project** の
  key と一致（acoustics のみの oracle）。`k_cand != k_live_after`
  （hashed `RecipeBranchPin` が cache を分離）。branch_ref
  artifact が `advance_clock`+`sweep` 後も `kept_reachable`・
  `EligibleExact`。
- Actual: 全 assert 成立。
- Oracle: 独立 project の独立再解決（branch resolution 出力を
  期待値に使わない）。
- Issue link: synvelis#296 RCP-GATE-05（2人協業・late attach・
  Undo/Reload・old commit pin 完全語彙は #294 Blocked）。

### RCP-GATE-06/07 headless — `rcp07_gate06_*` / `rcp07_gate07_*`

- GATE-06 Expected: `lookup` の `CandidateReport`（artifact id・
  eligibility・reproducibility・access）を `RenderCandidate` に
  転記 → `ExecutionSelector::select(MainPlayback)` が `Exact`・
  substitution 1 件・`artifact_id`/`content_hash` が publish 値
  と一致。quarantine 後の再 lookup verdict で non-Exact・
  substitution 0。
- GATE-06 Actual: 全 assert 成立。
- GATE-07 Expected: R1/R2 project の manual render が別
  `RenderJobKey`・別 `RecipeKey`・各 1 artifact。R1 key の
  lookup が R2 render 後も `EligibleExact`（stale 誤 activation
  0）。
- GATE-07 Actual: 全 assert 成立。
- Oracle: resolver verdict/substitution と store lookup outcome
  の直接比較（実装出力の再利用なし）。
- Issue link: synvelis#296 RCP-GATE-06/07（実 RT/PDC/実
  Window は外部 gate）。

## NotRun / Deferred / Blocked / Unsupported 一覧（所有 gate 明記）

| Lane | Case | status | 所有 Issue/gate |
| --- | --- | --- | --- |
| 実 Plugin ABI（VST3/CLAP/AU・stateful/fault plugin） | GATE-05/06/07/08/09 | NotRun | `scarecrow173/ruviform#630`・synvelis#176 |
| 実 signal/PCM clock・固定 PCM | GATE-07/10 | NotRun | `scarecrow173/ruviform#628` |
| 仮想 OS endpoint | GATE-05/07/08 | NotRun | `scarecrow173/ruviform#629` |
| 実 Window/GPUI runner・実 OS GUI | GATE-06/08 | NotRun | `scarecrow173/ruviform#631`・synvelis#114/#176 |
| 実 PCM reader readback・remote corrupt・実 ACL/lease | GATE-03 | NotRun（headless は manifest/digest レベルまで） | `scarecrow173/ruviform#628`/`#630` 系・synvelis#176 |
| AudioLens consumer（f64/長尺/CLI/GUI） | GATE-10 | NotRun | `scarecrow173/ruviform#294`・`scarecrow173/audio-lens#127` |
| RCP-05 専用 test-spec/fixture（late attach・2人協業・非RT renderer・Undo/Reload・old commit pin 完全語彙） | GATE-05 | **Blocked** | synvelis#294 |
| 実行/製品 Gate | GATE-11 前提の実機評価 | NotRun | synvelis#23 |
| 3OS 横断・MSRV | 全 case | NotRun | Daily/手動 Full CI（本 slice は Windows headless のみ） |
| 実 PCM bit-exact・plugin determinism | GATE-09/10 | NotRun/Unsupported（plugin 依存・未保証を偽装しない） | Issue #296 対象外規定 |

## Comparable baseline

- 先行 slice: `evidence/sv290/`（RCP-01）・`sv291/`（RCP-02）・
  `sv292/`（RCP-03）・`sv293/`（RCP-04）・`sv295/`（RCP-06）・
  `evidence/reports/sf-06-cross-gate-ledger.md`（cross-gate
  ledger 先例）。
- 本 slice はそれらの merged fixture を**再実装せず接合**し、
  cross-gate 固有の組合せ（recipe key→store index・lookup
  verdict→resolver candidate・job key→recipe key 分離・
  branch pin→cache 分離→adopt）のみを新規 pin する。
