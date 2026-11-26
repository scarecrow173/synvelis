---
id: "DESIGN-sv659VerificationRedesign01"
type: "design"
status: "draft"
title: "Issue #659 Journal・Authorization 形式的検証再設計 — slice1 ledger/matrix"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/659"
  changes:
    added:
      - type: "file"
        path: "docs/designs/issue-659-verification-redesign.md"
      - type: "file"
        path: "docs/test-specs/issue-659-verification-redesign.md"
      - type: "dir"
        path: "evidence/sv659/"
    modified:
      - type: "file"
        path: "docs/specs/formal-verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "POLICIES.md"
    - "docs/specs/formal-verification.md"
    - "evidence/sv601/verification.md"
    - "evidence/rf1014/verification.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "https://github.com/scarecrow173/synvelis/issues/601"
    - "https://github.com/scarecrow173/ruviform/issues/1014"
    - "https://github.com/scarecrow173/ruviform/issues/1015"
    - "https://github.com/scarecrow173/synvelis/issues/662"
---

# Issue #659 Journal・Authorization 形式的検証再設計 — slice1 ledger/matrix

## 1. 目的と境界

本書は `scarecrow173/synvelis#659` の slice 1 として、Issue #601 で導入された Journal・Authorization 検証を Property 起点で棚卸し、後続 slice が実装する representative proof・runner 分類・mutation 設計の基盤を定め、slice 2 の selective proof 結果を同期したものである。上位の規範は `POLICIES.md` §6.1〜§6.6 と `evidence/rf1014/verification.md` であり、本書はそこから派生する製品固有の検証設計である。

slice 1 は設計と inventory に限定し、製品コード、proof harness、Property Test、`tools/run-formal-kani.mjs`、mise lane、CI/workflow を変更しなかった。slice 2 は selective runner interface と `K-RJ-03`/`K-RJ-04` の実走結果だけを同期し、製品コード、proof harness、Property Test、bound、assume、unwind、mise lane、CI/workflow は変更しない。`docs/specs/formal-verification.md` の最終的な Verification Specification 再構成と証跡/手順分離は `scarecrow173/synvelis#662` が担うため、本書はその構成を先取りせず、Issue #601 当時の spec と slice 1/2 で確定した設計判断を分けて記録する。

## 2. 入力正本と inventory の作り方

inventory の入力正本は次の通りである。

- `docs/specs/formal-verification.md` — Issue #601 の Property・bound・lane・tool-fit 評価。
- `evidence/sv601/verification.md` と `evidence/sv601/runs.jsonl` — slice 1/2 の verdict、Kani 環境断、mutation 証跡。
- `crates/synvelis-gateway/tests/formal_journal/main.rs` — gateway journal 14 件と Kani 3 件。
- `crates/synvelis-gateway/tests/formal_journal/wire.rs` — WireRequest 9 件。
- `crates/synvelis-gateway/tests/formal_journal/policy.rs` — GrantPolicy 12 件。
- `crates/synvelis-recording/tests/formal_journal.rs` — recording journal/CRC32/session 15 件と Kani 4 件。
- `tools/run-formal-kani.mjs` と `.agents/skills/verify-synvelis/SKILL.md` — 現行 lane の実行形状と verdict 語彙。

全件カバーの根拠は、上記 4 つの Rust test ファイルについて `#[test]` と `#[kani::proof]` に続く関数名を抽出し、件数を gateway journal 14、wire 9、policy 12、recording 15、Kani 7 と数えたことである。合計 50 件の Property/Example Test と 7 件の既存 Kani proof を対象にする。補助の machine-readable 対応表を `evidence/sv659/inventory.json` に置くが、判断を含む正本は本書である。

`evidence/sv601/verification.md` の Kani bound 要約には、現行コードと一致しない古い数値（例: payload 4 byte、CRC 32 byte、seq 1..=4）が残っている。現行 harness は `docs/specs/formal-verification.md` の表と一致し、gateway round-trip は固定 12 byte、nonzero-seq は任意 `u64` の非零 seq、CRC32 は固定 16 byte である。本書の matrix は現行コードと現行 spec を正本とし、sv601 evidence は実行結果・環境断・mutation の証跡として使う。

## 3. 用語と判定軸

- **assurance 種別**: `Example Tested`（具体例）、`Property Tested`（生成検査）、`Bounded Exhaustive`（bound 内の尽きる検査）、`Model Checked`（状態/順序モデル）、`Theorem Proved`（数学的定理）を区別する。実行 verdict ではない。
- **必要保証**: `Required` は Issue #659 の受入に必要な保証、`Deferred` は今回の代表proofでは必須としないが後続で再評価する保証、`PT十分` は Property Test と具体境界で必要保証を満たすものである。
- **risk**: 破損時のユーザー影響・権限昇格可能性・検出困難性から `致命/高/中` に分類する。
- **現行 verdict**: sv601 時点の記録と、slice 1/2 で実施した `mise run test-formal` の結果を区別する。slice 1 では Kani を実走せず 2 pass・5 not_run の sv601 verdict を引き継ぎ、slice 2 では `K-RJ-03`/`K-RJ-04` だけを実走した。not_run は証明成功でも失敗でもない。
- **局所証明の転用禁止**: 固定長・固定 kind・小 stream・test double 上で成功した proof を、可変長 payload、Chunk allocation、永続化 recovery、認可製品全体、Project 整合性の証明として扱わない。

## 4. 入力/状態領域プロファイル

matrix で使う領域 ID を先に定義する。

| ID | 入力/状態領域・assume・bound |
| --- | --- |
| D-GJ-RANDOM | gateway `replay` への任意 byte 列 0..=512、proptest 既定 256cases。assume なし。 |
| D-GJ-PAYLOAD | payload 0..=384 byte/record、stream 0..=8 record。cap は `MAX_PAYLOAD` exact/over の少数例で補う。 |
| D-GJ-MUT | encode 済み 1..=4 record stream に対し、record index・byte offset・非零 xor を生成して 1 byte 変異する。 |
| D-GJ-TRUNC/TAIL | encode 済み stream の切断点全域から 1 点、または 0..=48 byte の非 magic 末尾を生成する。 |
| D-GJ-SERDE | `JournalPayload` 全 6 variant。nested refs ≤32 chars、sets ≤4、state ≤256 byte。u128 tagged id は任意 u128。 |
| D-WIRE-JSON | JSON 深さ ≤4、container ≤8 child、文字列 ≤24 chars。通常識別子と forbidden key を混合する。 |
| D-WIRE-NEAR | required 5 field を持つ near-valid request に optional target/args/task/grant/idempotency_key を付ける。 |
| D-POL-SPINE | fresh `InMemoryPolicyStore`/`ManualClock`/`StaticOracle`/fixture `Gateway` を case ごとに構築。budget ≤5、commit 反復 ≤budget+2、64cases。 |
| D-POL-BUDGET | `DelegatedApply` budget 0..=5 または None、直列 commit、store counter を独立観測する。 |
| D-POL-SCOPE/CREATE/EFFECTS/DIMENSION | Exact scope、create defect、effects defect、grant dimension defect を構造化して生成する。ID 列以外の文字列・数値空間は全網羅しない。 |
| D-RJ-RANDOM | recording `replay_journal` への任意 byte 列 0..=512、proptest 既定 256cases。assume なし。 |
| D-RJ-PAYLOAD | 全 3 record kind。Chunk spans 0..=6、record 0..=8。span count cap と payload cap は concrete boundary test で補う。 |
| D-RJ-CRC | proptest は入力 ≤256 byte・分割点 ≤4。Kani は固定 16 byte・split ≤16。 |
| D-SESSION | `SessionState` 4 状態。遷移は 4×4 の有限全域、acceptance gate は 4 状態全域。 |
| K-STREAM44 | Kani stream 長 ≤44。`assume(len ≤ 44)`。replay/crc loop に `unwind(64)`。最小 record 2 件と slack を含み、recording では Chunk `span_count` の symbolic allocation 経路に到達しない。 |
| K-FIXED12/8 | gateway round-trip は固定 12 byte payload、nonzero-seq は固定 8 byte payload。可変長 Vec allocation を避ける。 |
| K-FIXED-KINDS | recording round-trip は Checkpoint/Finalize のみ。Chunk の `Vec<JournalSpan>` を除外する。 |
| K-CRC16 | 固定 16 byte 任意入力、split ≤16、`unwind(24)`。 |
| K-SESSION4 | 4×4 pair と acceptance gate を concrete 展開、`unwind(8)`。bound ではなく有限全域。 |

## 5. ledger/matrix

各列は次の意味を持つ。「現行」は test/proof 名、実行種別、verdict、既存コストをまとめて示す。「領域」は入力/状態領域、assume、Bound、抽象化、除外、未証明領域をまとめて示す。個別セルの省略はなく、共通 profile ID を使う場合も §4 の定義が本書内にある。

### 5.1 gateway journal — Property/Example Test 14 件

| Property ID / test | 旧契約 | 現行検証・verdict・cost | 必要保証・risk・assurance | subject/境界 | 領域・除外・未証明 | 新検証案 |
| --- | --- | --- | --- | --- | --- | --- |
| GJ-P01 `replay_arbitrary_bytes_contract` | 任意入力で panic せず、stop closed、prefix_len≤len、CleanEnd iff 全検証、seq 0連番、prefix再replay CleanEnd | PT 256cases・Pass・秒級〜数十秒級 | Required・致命・PT + 代表BE | 製品 `replay` のみ。file/recovery は境界外 | D-GJ-RANDOM。512byte超・cap・recovery順序は除外/未証明 | K-GJ-01 を代表proofとして完走。PTとmutationは継続 |
| GJ-P02 `encode_frame_layout` | magic/version/reserved/seq/len/payload/crc32 layout | PT 256cases・Pass | PT十分・高・PT | 製品 `encode_frame` pure layout | D-GJ-PAYLOAD。cap exactは別test。全長網羅は未証明 | PT維持。K-GJ-02とcap例証で補完 |
| GJ-P03 `encode_seq0_replays_clean` | seq0 frameはpayload同一でCleanEnd | PT 256cases・Pass | Required・高・PT+BE(固定長) | encode/replayのみ | D-GJ-PAYLOAD。可変長・複数recordは未証明 | K-GJ-02を代表proofとして維持し、全長へ転用しない |
| GJ-P04 `encode_nonzero_seq_is_gap` | seq≠0の単一frameはSequenceGap・records空・prefix0 | PT 256cases・Pass | Required候補・高・PT+BE | encode/replayのみ | D-GJ-PAYLOAD。複数record欠番は未証明 | K-GJ-03を低コストproofとして維持 |
| GJ-P05 `encode_stream_roundtrip` | 連番streamが順序・payloadを保ちCleanEnd | PT 256cases・Pass | PT十分・高・PT | encode列+replay | D-GJ-PAYLOAD。9件以上・任意長は未証明 | 単record/seq規律BEの接続条件としてPT維持。全stream単一proofにしない |
| GJ-P06 `single_byte_corruption_never_verifies_past_it` | record k内1byte変異はk以降をintact化できない | PT 256cases・Pass。CRC除去mutationはexit101で期待Fail | Required・致命・PT（mutation検証済） | 有効stream変異→replay | D-GJ-MUT。複数byte/悪意的再構成/大規模streamは未証明 | PT+mutationをRequired維持。固定小stream変異BEの費用を次slice評価 |
| GJ-P07 `truncation_is_torn_tail_or_boundary_clean` | record途中切断はTornTail、境界切断はCleanEnd、完全frameのみ検証 | PT 256cases・Pass | PT十分・高・PT | prefix切断 | D-GJ-TRUNC。FS喪失モデル合成は未証明 | PT維持。Recovery orderingへ接続条件を引き継ぐ |
| GJ-P08 `trailing_garbage_stops_at_boundary` | 有効stream後の非magic tailはframe境界で停止 | PT 256cases・Pass | PT十分・中・PT | replay末尾分類 | D-GJ-TAIL。magic一致後の複合破損は未証明 | PT維持。K-GJ-01の任意streamBEで補完 |
| GJ-P09 `journal_payload_serde_roundtrip` | 全6variantがserde_json無損失往復 | PT 256cases・Pass | PT十分・高・PT | serde boundaryのみ | D-GJ-SERDE。巨大/深いJSONとserde内部は未証明 | PT維持。Kani対象外判断を維持 |
| GJ-P10 `journal_payload_rejects_unknown_fields` | inner recordの未知fieldを拒否 | PT 256cases・Pass | PT十分・高・PT | serde boundary | D-GJ-SERDE。leaf variantは注入不可。serde内部は未証明 | PT維持。variant追加時にgenerator同期 |
| GJ-P11 `u128_tagged_id_serde_contract` | `<kind>-<32 lowercase hex>`往復、誤tag/malformed hex拒否 | PT 256cases・Pass | PT十分・中・PT | id serde | 任意u128。対象は3種id。他id種は未証明 | PT維持。id種追加時に対象追加 |
| GJ-P12 `encode_frame_over_cap_refused` | payload_len>MAX_PAYLOADでNone | PT 4cases・Pass | Example/PT十分・高・Example+PT | payload長gate | MAX+1..+64。cap超全長は未証明 | 具体境界を維持。BE対象にしない |
| GJ-P13 `encode_frame_at_cap_roundtrips` | payload_len=MAX_PAYLOADちょうどはroundtrip成功 | 固定例1case・Pass | Example十分・高・Example | exact boundary | 固定16MiB。cap超は別test | exact boundaryを維持 |
| GJ-P14 `saved_record_rejects_unknown_field` | SavedRecordの未知fieldを拒否し未注入はparse成功 | 固定例1case・Pass | Example十分・中・Example | `JournalPayload::Saved` serde | 固定record。他variantはGJ-P09/P10 | pin維持 |

### 5.2 gateway WireRequest — Property Test 9 件

| Property ID / test | 旧契約 | 現行検証・verdict・cost | 必要保証・risk・assurance | subject/境界 | 領域・除外・未証明 | 新検証案 |
| --- | --- | --- | --- | --- | --- | --- |
| GW-P01 `documented_forbidden_lists_stay_in_product` | documented forbidden keyは製品constから削られない | Just(())実質1case・Pass。token削除mutationはexit101で期待Fail | Required・致命・PT（mutation検証済） | wire scan表。serde内部は境界外 | D-WIRE-FORBIDDEN。製品側追加は許容。未記載keyの意味論は未証明 | test-local契約を維持。pure key scan分離後に有限key集合BE化 |
| GW-P02 `parse_never_panics_and_errors_are_closed` | 任意JSONでpanicせずErrは3分類に閉じる | PT 256cases・Pass | Deferred BE・致命・PT必須 | `serde_json::Value`→`WireRequest`解釈 | D-WIRE-JSON。深い/巨大JSON、duplicate key、serde内部は未証明 | key scan・形状検査・serde結合を分離し、pure部分だけBE化 |
| GW-P03 `accepted_values_satisfy_allowlist_model` | 受理入力は独立allow-list modelを満たす | PT 256cases・Pass | Required・高・PT+independent model | near-valid requestとtest-local model | D-WIRE-NEAR。逆方向の全JSON一致は未証明 | 独立modelを維持し、pure structural checkerへ分解してBE候補化 |
| GW-P04 `authority_fields_rejected` | top-level authority keyはSelfDeclaredAuthorityでfield echo | PT 256cases・Pass。token削除mutationは期待Fail | Required候補・致命・PT（mutation検証済） | top-level scan | 13key×任意JSON値。nested位置とwire外transportは未証明 | pure key scan抽出後、有限key×代表値BEを次slice評価 |
| GW-P05 `implicit_target_fields_rejected` | top-level implicit-target keyはImplicitTargetRejectedでfield echo | PT 256cases・Pass | Required候補・高・PT | top-level scan | 5key×任意JSON値。nested位置は未証明 | GW-P04と同じpure scan proofへ統合 |
| GW-P06 `authority_precedes_implicit_target` | 両方存在時はauthority拒否が優先 | PT 256cases・Pass | Required候補・高・PT | scan順序 | authority13×implicit5。serdeエラーとの全優先順位は未証明 | scan順序をpure coreで有限BE候補化 |
| GW-P07 `forbidden_keys_nested_under_args_not_rejected` | args配下の同名keyはtop-level拒否にしない | PT 256cases・Pass | PT十分・中・PT | top-level限定契約 | D-WIRE-NEAR+nested注入。args schema意味論は未証明 | PT維持。top-level scan BEの境界外条件として記録 |
| GW-P08 `unknown_nested_keys_are_malformed` | capability/schema等の未知nested keyはMalformed | PT 256cases・Pass | Deferred BE・高・PT必須 | nested allow-list scan+serde | 4箇所重点。target/consistency全経路は未証明 | nested allow-listをpure structural checkerへ分離後にBE再評価 |
| GW-P09 `missing_required_keys_are_malformed` | required key欠落はMalformed | PT 256cases・Pass | PT十分・高・PT | serde shape validation | required keys有限集合。全欠落組合せは未証明 | PT維持。pure parse分離後に有限集合BEを再評価 |

### 5.3 gateway GrantPolicy — Property Test 12 件

| Property ID / test | 旧契約 | 現行検証・verdict・cost | 必要保証・risk・assurance | subject/境界 | 領域・除外・未証明 | 新検証案 |
| --- | --- | --- | --- | --- | --- | --- |
| GP-P01 `unbound_agent_gets_uniform_no_auth` | authority chain失敗は全8stageで一律 `no applicable authorization` | PT 64cases・Pass。full rig/caseで高め | Required・致命・PT→BE候補 | `authorize_agent` chain。store/oracle/clockは注入境界 | D-POL-SPINE+UNBOUND。durable store・並行revoke・全id/string空間は未証明 | pure判定coreとsnapshotを分離し、stage×defect有限行列をBE化 |
| GP-P02 `human_permitted_at_every_stage` | descriptor一致のtrusted humanは全stage/effects/commitでPermit・budget非消費 | PT 64cases・Pass | Required・高・PT→BE候補 | human短絡とdescriptor gate | trusted callerはAdapterSeal前提。caller attestation自体は未証明 | descriptor一致条件付き有限stage proofへ分解 |
| GP-P03 `descriptor_mismatch_denies_first` | descriptor mismatchはhuman短絡より先にdeny | PT 64cases・Pass | Required・致命・PT→BE候補 | `evaluate_with`先頭gate | id mismatch/version driftの2形態。registry解決は未証明 | 有限id/version不一致モデルでBE化 |
| GP-P04 `delegated_apply_commit_spend_is_monotone` | DelegatedApply Commitは1permit=1unit・枯渇deny・store spend整合 | PT 64cases・Pass | Required・致命・PT→BE/Model候補 | budget状態とcommit評価。storeはatomic test double | D-POL-BUDGET。crash・並行commit・無限budgetは未証明 | 小さいbudget state machineをKani化し、revoke/commit並行性はTLA+判定へ |
| GP-P05 `preflight_and_deny_never_spend` | 非Commit/effects/Denyはbudgetを消費しない | PT 64cases・Pass | Required・致命・PT | stage×entrypoint消費規則 | D-POL-BUDGET。commit fence原子性・例外中断は未証明 | GP-P04と同じstate machine proofへ統合 |
| GP-P06 `commit_entry_points_agree` | `evaluate_commit`と`decide(Commit)`は同じdecision/spend | PT 64cases・Pass | PT十分・高・PT | 2つの公開entrypoint | identical twin rig。並行store変更は未証明 | 公開API接続条件としてPT維持 |
| GP-P07 `decisions_stay_in_closed_set` | decisionは閉集合、deny reasonは契約13集合 | PT 64cases・Pass | PT十分・高・PT | decision出力 | DENY_REASONS literal pin。新reason審査は運用外 | literal pin維持。reason追加時は契約変更として同期 |
| GP-P08 `unresolved_or_uncovered_target_denies` | unresolved/scope外targetは全stageで`target outside grant scope` | PT 64cases・Pass | Required候補・致命・PT | target coverage+oracle resolution | Exact scope中心。Dynamic/Snapshot/relatedは未証明 | Exact membershipをpure scope algebraへ分離しBE。Dynamic関連はPT/別proof |
| GP-P09 `create_target_defects_deny` | create_within欠如/親未解決/親不許可/kind不許可はcreate拒否 | PT 64cases・Pass | Deferred BE・高・PT必須 | create axis+owner constraint | 4 defect構造化。placement全形態・実project作成後状態は未証明 | pure判定化後にBE再評価。現行はPT必須 |
| GP-P10 `effects_recheck_actual_resources_and_classes` | unsupported/disclosure/transfer/uncovered readは対応reasonでdeny | PT 64cases・Pass | Required候補・致命・PT | `ScopedEffects`照合 | 4 defect。covered/stray source固定軸。effect導出と全DataAccess組合せは未証明 | effect照合をpure coreへ分離し、resource/data-access algebraの代表BE候補 |
| GP-P11 `coverage_stable_under_repeat_and_dedup` | 重複resourceと反復評価でdecisionが変わらない | PT 64cases・Pass | Deferred BE・中・PT必須 | scope counter/dedup | reads重複+writes、非commit stage。dynamic max_members全域は未証明 | 集合へ正規化できれば代数的proof候補。現行はPT |
| GP-P12 `grant_dimension_denies_stay_in_contract` | revoked/expired/superseded/capability/permissions/data_accessは対応reasonでdeny | PT 64cases・Pass | Required・致命・PT→BE候補 | grant dimension gate | 6 defect。clock/epochは決定的注入。連続時刻・並行更新は未証明 | 有限次元×境界前後clockでBE。clock連続域は単調性Propertyへ分離 |

### 5.4 recording journal/CRC32/session — Property/Example Test 15 件

| Property ID / test | 旧契約 | 現行検証・verdict・cost | 必要保証・risk・assurance | subject/境界 | 領域・除外・未証明 | 新検証案 |
| --- | --- | --- | --- | --- | --- | --- |
| RJ-P01 `replay_journal_arbitrary_bytes_contract` | 任意入力でpanicせず、prefix/seq/last_seq/CleanEnd/prefix再replay契約 | PT 256cases・Pass | Required・致命・PT+代表BE | 製品`replay_journal`のみ | D-RJ-RANDOM。512byte超・cap・FS統合は未証明 | K-RJ-01を代表proofとして完走。PT継続 |
| RJ-P02 `encode_record_layout` | magic/version/kind∈{1,2,3}/seq/len/payload/crc32 layout | PT 256cases・Pass | PT十分・高・PT | `encode_record` pure layout | D-RJ-PAYLOAD。span count飽和・cap exactは別test | PT維持。K-RJ-02とcap例証で補完 |
| RJ-P03 `encode_record_seq0_replays` | seq0 recordは全3kindで同一payloadに復元 | PT 256cases・Pass | Required・高・PT+BE(固定長kind) | encode/replayのみ | D-RJ-PAYLOAD。Chunk symbolic span count/Vec allocationは除外・未証明 | 固定長2kind BEを代表proof。ChunkはPT+cap境界+必要時harness分割 |
| RJ-P04 `encode_record_nonzero_seq_is_gap` | seq≠0の単一recordはSequenceGap・last_seq None | PT 256cases・Pass | Required候補・高・PT | encode/replay | seq任意非零。複数record欠番・録音側Kaniは未証明 | gateway側同型proofの実測を使い、録音側小harness追加を次slice評価 |
| RJ-P05 `appender_stream_roundtrip_and_seq_discipline` | JournalAppenderが連番採番し連結streamが順序・payloadを保つ | PT 256cases・Pass | PT十分・高・PT | Appender+encode/replay | D-RJ-STREAM。9件以上・resume接続は未証明 | 単record/seq規律の接続条件としてPT維持 |
| RJ-P06 `resumed_appender_frames_gap_as_fresh_stream` | resumed(n)はnから採番するがfresh replayではSequenceGap | PT 256cases・Pass | PT十分・高・PT（契約pin） | resumed appenderとfresh replay | start任意非零、record1..3。正しいresume手続きは未証明 | 現行契約pinを維持し、Recoveryモデルの前提にする |
| RJ-P07 `single_byte_corruption_never_verifies_past_it` | record k内1byte変異はk以降をintact化できない | PT 256cases・Pass。seq系mutation証跡あり | Required・致命・PT（mutation検証済） | 有効stream変異→replay | D-RJ-MUT。複数byte/媒体相関破損は未証明 | PT+mutation維持。小stream変異proofを費用評価 |
| RJ-P08 `truncation_is_torn_tail_or_boundary_clean` | record途中切断はTornTail、境界切断はCleanEnd | PT 256cases・Pass | PT十分・高・PT | prefix切断 | D-RJ-TRUNC。FS喪失/power-loss順序は未証明 | PT維持。Recovery orderingへ接続 |
| RJ-P09 `trailing_garbage_stops_at_boundary` | 有効stream後の非magic tailはrecord境界で停止 | PT 256cases・Pass | PT十分・中・PT | replay末尾分類 | D-RJ-TAIL。magic一致後の複合破損は未証明 | PT維持。K-RJ-01で補完 |
| RJ-P10 `crc32_incremental_matches_oneshot` | incremental Crc32とone-shot crc32が任意分割で一致 | PT 256cases・Pass | Required・高・PT+代表BE | crc32/Crc32のみ | D-RJ-CRC。256byte超・多数分割BE・暗号学的耐性は未証明/契約外 | K-RJ-03を代表proofとして完走。既知vectorと併用 |
| RJ-P11 `digest_fold_chain_is_deterministic` | raw foldとIntegrityDigest::foldが一致・hex固定16 lowercase | PT 256cases・Pass | PT十分・中・PT | digest fold/newtype/hex | values≤16。衝突耐性は契約外。全長chain証明は未証明 | PT維持。必要保証は決定性と用途分離であることを明記 |
| RJ-P12 `session_transition_agrees_with_require` | legal/requireが4×4で一致し合法は3 forward edgeのみ | PT 256cases・Pass | Required・高・PT+全域BE | session state machine | D-SESSION。上位操作列/crash/recoveryは未証明 | K-RJ-04をRequired proofとして完走。TLA+対象は上位順序に限定 |
| RJ-P13 `session_acceptance_gates_are_total` | registration iff Open、take verification iff Open/Closed | PT 256cases・Pass | Required・高・PT+全域BE | session state machine | D-SESSION。操作受付の文脈・副作用は未証明 | K-RJ-04に統合。上位workflowは別Property |
| RJ-P14 `chunk_record_over_cap_refused` | payloadがMAX_PAYLOAD超ならencode_recordはNone | 固定例1case・Pass | Example十分・高・Example | Chunk payload長gate | 30000 spans。u16=65535付近全境界は未証明 | 具体例を維持し、span count制約を明記 |
| RJ-P15 `chunk_record_at_cap_boundary` | 21844 spansはin-cap roundtrip、21845はover-cap拒否 | 固定例2case・Pass | Example十分・高・Example | Chunk exact boundary | 58+48n byte計算の2点。中間全nは未証明 | 具体境界を維持。BEへ拡大しない |

### 5.5 既存 Kani proof 7 件

| Property ID / harness | 旧契約 | 現行検証・verdict・cost | 必要保証・risk・assurance | subject/境界 | 領域・除外・未証明 | 新検証案 |
| --- | --- | --- | --- | --- | --- | --- |
| K-GJ-01 `replay_contract_bounded` | ≤44byte任意streamでGJ-P01相当の分類契約 | sv601ではsolver起動後OOM/VM crash・NotRun。cost不明 | Required・致命・Bounded Exhaustive(未完了) | 製品gateway replay | K-STREAM44。44byte超・cap・file/recoveryは未証明 | 代表proof。runner分割/選択実行/KANI_JOBS=1または2で再測定し、proof boundaryを縮めずに原因分類 |
| K-GJ-02 `encode_replay_roundtrip_bounded` | 固定12byte payloadのseq0 roundtrip | Pass・0/642 failed・45.5s | Required・高・Bounded Exhaustive | gateway encode/replay | K-FIXED12。可変長・複数recordは未証明 | 代表proofとして維持。passを全長証明へ転用しない |
| K-GJ-03 `nonzero_seq_is_gap_bounded` | 固定8byte payload・任意seq≠0がSequenceGap | Pass・0/610 failed・31.4s | Required候補・高・Bounded Exhaustive | gateway encode/replay | K-FIXED8-NONZERO。複数record欠番は未証明 | 維持。録音側同型proof追加は次sliceで費用比較 |
| K-RJ-01 `replay_journal_contract_bounded` | ≤44byte任意streamでRJ-P01相当の分類契約 | solver起動後OOM/VM crash・NotRun。cost不明 | Required・致命・Bounded Exhaustive(未完了) | 製品recording replay | K-STREAM44。44<48でChunk symbolic allocation経路を構造除外。長大stream/cap/FSは未証明 | 最優先代表proof。選択実行と資源分類で再測定 |
| K-RJ-02 `encode_replay_roundtrip_bounded` | Checkpoint/Finalizeの全field symbolic・seq0 roundtrip | solver起動後OOM/VM crash・NotRun。cost不明 | Required・高・Bounded Exhaustive(未完了) | 製品recording encode/replay | K-FIXED-KINDS。Chunk/Vec<JournalSpan>/可変長は未証明 | 代表proof。Chunkは別harnessまたはPTとして分離 |
| K-RJ-03 `crc32_split_equivalence_bounded` | 固定16byte・任意split≤16でincremental=one-shot | Pass・0/131 failed (1 unreachable)・Verification Time 282.0517s・runner wall-clock 310.115s・sync `24d2e2a25291d4e8ec75c7f1ea41bad049421be5` | Required・高・Bounded Exhaustive | 製品crc32/Crc32 | K-CRC16。16byte超・多数分割・任意長数学証明は未証明 | pass は K-CRC16 の内側だけの保証として記録。変更時は選択実行で再検証する |
| K-RJ-04 `session_transitions_exhaustive` | 4×4遷移とacceptance gatesを全網羅 | NotRun・solver が 300s 予算内に完了せず外部停止・runner wall-clock 634.49s・sync `24d2e2a25291d4e8ec75c7f1ea41bad049421be5` | Required・高・Bounded Exhaustive(有限全域・未完了) | 製品session SM | K-SESSION4。上位操作列/crash/recoveryは未証明 | solver cost の原因調査と harness 分割可否を次 slice で判断する。bound/weakening はしない |

## 6. Journal Codec・CRC32・Session Transition の保証設計

### 6.1 必要保証と粒度

保証は 3 層に分ける。第 1 層は pure codec の byte 契約で、任意 byte 列の分類、encode/replay round-trip、sequence gap、破損以降の非検証を対象にする。第 2 層は数値境界で、gateway `MAX_PAYLOAD = 1 << 24`、recording `MAX_PAYLOAD = 1 << 20`、recording Chunk payload `58 + 48*n` byte、exact cap/over-cap を具体例で固定する。第 3 層は session の有限状態機械で、4×4 遷移と acceptance gate は全域証明できるが、上位の recording workflow や crash recovery は別の ordering property である。

代表的重要 Property は次の 5 件とする。選択理由は、いずれも製品の pure contract を直接検査し、現行 PT oracle と独立に bound を説明でき、かつ小さい実行予算で反復可能だからである。

1. `K-RJ-01` recording replay 分類契約 — journal 破損を安全側へ分類する中核。
2. `K-RJ-02` recording encode/replay round-trip — encode したデータが復元できる中核。ただし Chunk を除く境界を明示する。
3. `K-RJ-03` CRC32 分割同値 — incremental/one-shot の実装差を固定長で検査する。
4. `K-RJ-04` session transition exhaustive — 4×4 と gate が有限全域なので最も完全な保証を得られる。
5. `K-GJ-01` gateway replay 分類契約 — gateway journal の同型契約。`K-GJ-02`/`K-GJ-03` は既に pass 済みであり、代表性確認と回帰として維持する。

### 6.2 境界と実行予算

- Kani の budget は proof ごとに wall-clock 5 分、lane 全体 30 分を初期予算とする。これは実行計画であり、成功条件ではない。超過した場合は proof を弱めず、harness 分割・選択実行・並列度低下・環境増強の順で原因を切り分ける。
- `K-STREAM44` は最小 record 2 件と trailing slack を含むが、recording Chunk payload の symbolic allocation 経路には到達しない。したがって 44 byte proof の成功は Chunk allocation 安全性や `MAX_PAYLOAD` 近傍の証明ではない。
- 固定長 payload（12/8 byte）と固定 kind（Checkpoint/Finalize）は symbolic allocation を避けるための抽象化である。可変長・複数 record・Chunk は PT と concrete boundary test に残り、未証明領域として管理する。
- gateway cap は 16 MiB、recording cap は 1 MiB であり、値が違う。matrix と証跡で cap を一括りにしない。recording Chunk exact boundaryは 21844 spans が in-cap、21845 spans が over-capである。
- CRC32 は改竄検出のための integrity checksum であり、暗号学的衝突耐性を主張しない。`digest_fold` も決定的な integrity folding であり、SHA-256 等の content identity と混同しない。
- session proof が成功しても、ファイル作成、media sync、checkpoint append、commit 適用の順序は証明されない。これらは §8 の Recovery/ordering inventory で扱う。

slice 2 の selective proof 実走（sync `24d2e2a25291d4e8ec75c7f1ea41bad049421be5`）では、`K-RJ-03` が `VERIFICATION:- SUCCESSFUL`・0/131 failed・Verification Time 282.0517s で Pass した。runner wall-clock は 310.115s で、compile/loading を含むため verification time と区別する。`K-RJ-04` は solver が 300s 予算内に完了せず、10 分を超えた時点で invocation を外部停止した。反例や unwinding assertion は出ていないため、結果は Fail ではなく cause class `solver` の NotRun である。`format!` を含む illegal-transition error path が solver 入力を大きくしている可能性はあるが、完了ログがないため未確定として記録する。

これらの結果は保証範囲を広げない。`K-RJ-03` の Pass は固定 16 byte・split ≤16 の incremental/one-shot 同値だけを示し、任意長、3 以上の分割、暗号学的衝突耐性を示さない。`K-RJ-04` が将来 Pass しても、`SessionState` 4×4 と acceptance gate の有限全域だけを示し、上位 recording workflow や crash recovery を示さない。

### 6.3 保証制約の引き継ぎ

AC5/AC8 に対応する制約は次の通り matrix に固定する。固定長 proof の成功は固定長の保証、bounded stream の成功はその長さ以下の保証、Chunk を除いた proof は Chunk 非対象、session SM の成功は上位操作列非対象である。これらの制約を外した証拠がない限り、製品全体・任意長・永続化 recovery・RT 精度・Project 整合性の保証へ転用してはならない。

## 7. Authorization の分離案と Property 分類

### 7.1 責務分離

Authorization は次の 4 層に分離して検証対象を決める。現行コードをこの slice で変更する意味ではなく、検証の subject を分ける設計である。

1. **WireRequest 解釈層**: 入力 JSON から `WireRequest` を作る。caller/authority/implicit target を受け取らない、unknown nested key を拒否する、エラー分類を閉じる。副作用なし。
2. **bind/Operation 解釈層**: `WireRequest` と Gateway 文脈から `OperationRequest` を作る。caller は transport/auth context からのみ注入され、wire 入力が target/principal/project を偽装できない。
3. **純粋認可判定層**: immutable snapshot 化した grant/store/oracle/clock 入力、`OperationRequest`、approval policy から decision/effects/budget消費を計算する。ここが最も proof 適性が高い。
4. **外部 store/oracle/clock 層**: revocation、permit spend、resource resolution、時刻を供給する。durable store・並行更新・時刻源は副作用を持つため、単体BEではなく test double との conformance、PT、必要なら ordering model で扱う。

serde 入力は `serde_json::Value` と internally tagged enum への依存が大きく、現行 tool-fit 評価どおり Kani 直証明には向かない。したがって serde 結合部は PT に残し、pure structural scan と pure decision core だけを BE 対象へ分離する。`Arc<String>` や dynamic trait object をまたぐ現行 `GrantPolicy::evaluate_commit` をそのまま Kani に入れない。

### 7.2 Property 一覧と分類

| 分類 | Property | 理由 | 残留リスク |
| --- | --- | --- | --- |
| Required | Wire authority/implicit scan（GW-P01/P04/P05/P06） | 権限昇格入口であり、key集合と優先順位は有限 | args nested の意図的許容と serde 後段は別保証 |
| Required | 存在秘匿の一律拒否（GP-P01） | agent failure を漏らさないことが認可境界の核心 | durable store/並行revokeはtest double外 |
| Required | descriptor gate（GP-P02/P03） | trusted human短絡より先の安全条件 | descriptor registry解決自体は別契約 |
| Required | budget消費規則（GP-P04/P05） | 二重消費/無消費は権限・課金相当の重大欠陥 | crash/並行commitは別モデルが必要 |
| Required | grant次元拒否（GP-P12） | revoked/expired/superseded等の核心gate | 連続clockの全値ではなく境界代表 |
| Required候補 | scope/effects照合（GP-P08/GP-P10） | resource/data-accessの権限拡大防止 | Dynamic/Snapshotとeffect導出は pure core 外 |
| Deferred BE | parse closedness/受理model（GW-P02/P03/P08） | 重要だが serde_json 依存が大きい | pure scan分離後に再評価するまでBEなし |
| PT十分 | nested args非拒否、required欠落、entrypoint同値、closed decision set、coverage冪等、create defect | 生成検査とliteral pinでリスクに比例した保証になる | 入力空間の完全網羅ではない |
| PT/統合 | bind/Operation解釈 | Gateway文脈・adapter metadata依存が大きい | wire-only proofではbindを保証しない |
| PT/conformance + Model候補 | 外部store/oracle/clock | durable状態と時間は副作用を持つ | test doubleと実storeの同値性は実装契約レビューに依存 |

この分類は slice 1 の設計確定であり、proof 実装・実走ではない。`Required` は後続 slice で pure core 抽出または既存 proof 維持の形で実走計画を立てる。`PT十分` は「形式的証明が不要」ではなく、Property Test・mutation・レビューで必要保証を満たすと判断したものであり、未証明領域を matrix に残す。

## 8. Recovery・操作順序の inventory と必要性判定

### 8.1 状態 inventory

Recovery の ordering を判定するには、少なくとも次の状態を区別する必要がある。ここでは製品コードを変更せず、観測済み契約から状態名だけを固定する。

- media bytes と media sync 完了フラグ。
- journal byte stream、intact prefix、last checkpoint position、appender `next_seq`。
- checkpoint record の append 完了、journal sync 完了、directory sync 完了。
- commit 適用中/適用済み、`applied_commits` entry、`commit-manifest.json`、`commit.json`。
- session state（Created/Open/Closed/Finalized）と take registration/verification acceptance。
- recovery scan が選ぶ durable frontier と、frontier 以降の torn tail/invalid tail。

### 8.2 crash point inventory

| crash point | 直前/直後の区別 | 期待する回復契約 |
| --- | --- | --- |
| CP1 media write後・media sync前 | bytesが安定していない可能性 | そのmediaに依存するcheckpointはdurable扱いしない |
| CP2 media sync後・checkpoint append前 | mediaはあるがjournal記録なし | checkpointを発明せず、frontierは進めない |
| CP3 checkpoint append中/append後・journal sync前 | torn tailまたは未耐久record | replayがintact prefixまでに限定される |
| CP4 journal sync後・dir sync前 | recordはあるがnamespaceが不安定 | platform可能範囲でdir syncし、不能環境は証拠へ記録 |
| CP5 commit副作用後・manifest更新前 | 副作用とidempotency記録がずれる | 再起動後に二重適用しない、または未適用として安全に再開できる |
| CP6 manifest後・commit.json/applied_commits更新前 | 観測可能な完了表示と再実行記録がずれる | commit完了の表示と再実行防止が矛盾しない |
| CP7 session close/finalize中 | state遷移と永続化の順序 | illegal transitionやfinalize済み操作の受付をしない |

### 8.3 ordering property inventory

1. checkpoint は依存する media sync 完了後にのみ durable frontier へ入る。
2. durable frontier は単調にのみ進み、corruption/torn tail を跨がない。
3. replay が報告する intact prefix 以外の record を committed state の根拠にしない。
4. commit idempotency key について、副作用適用と再実行防止記録の間に中途半端な観測があっても二重適用しない。
5. close/finalize/session acceptance は `SessionState` の合法遷移と矛盾しない。
6. resume 後の appender seq は既存 prefix と衝突せず、衝突し得る場合は fresh stream として明示的に gap になる。

### 8.4 必要性判定

TLA+ または同等の state/ordering model は **必要** と判定する。理由は、上記 property が単一 pure function の入出力ではなく、媒体・journal・manifest・session の複数状態と crash point の interleaving に関係し、Kani/proptest の局所 proof では crash 前後の順序を表現できないためである。ただし slice 1/2 では model を導入しない。後続 slice はまず CP1〜CP7 を最小限の atomic action へ写像し、どの副作用を atomic と仮定するか、fdatasync/fsync/dir sync をどこまで抽象するかをレビューしてから TLC/Apalache等の実用予算を決める。仮定を固定できない場合は model 化を行わず、不足情報として記録する。

## 9. 失敗分類・選択実行・再実行設計

### 9.1 分類軸

`POLICIES.md` に従い、assurance kind・target・synchronization state・failure cause・execution verdict を独立に記録する。timeout/OOM/environment absence/exit 0/CI green は証明成功ではない。失敗原因の初期分類は次の通りである。

| cause class | 判定材料 | 再実行設計 |
| --- | --- | --- |
| `environment_absent` | native `cargo kani` 不在、WSL distro 不在、cargo/kani probe失敗 | 環境 provision。proof結果ではなく `not_run/environment_blocked` |
| `path_mapping` | repo path が `/mnt/<drive>` へ写像できない、wslpath失敗 | 自動mount対象ドライブで再実行。target dirはWSL native固定 |
| `compile` | Rust/Kani version差、dependency/harness compile error | compile logを保存し、proof実行前失敗として分類。harness変更は別承認 |
| `symex` | unwinding assertion、loop unwinding不足、function contract/actor model不整合 | bound/unwindと実装責務を確認。proof weakenは禁止 |
| `solver` | solver timeout/unknown/反例なしの未完了 | 時間を伸ばすかharness分割。timeoutはpassにしない |
| `memory` | oom-kill、CBMC RSS、WSL VM crash、exit後unclean boot | `KANI_JOBS=2`→`1`、sibling負荷停止、選択実行、WSL memory増量を順に試す |
| `parallelism` | `-j`上げ時のみ再現、単発なら進行 | 並列度だけを変えて再現性を測る。boundは変えない |
| `infrastructure` | WSL `E_UNEXPECTED`、VM hard crash、CI runner kill | ツール/VM crashとしてproof failと区別。raw log必須 |
| `product_oracle_failure` | `VERIFICATION:- FAILED`、反例、oracle差分 | 製品欠陥または抽象化誤りとして解析する。environmentと混同しない |

slice 2 の runner は `pass`/`fail`/`not_run(environment_blocked)` の語彙を保ちながら、package/harness/jobs/environment/elapsed/exit code/harness件数を1行JSONへ追加した。cargo が非零で終わると証明失敗と環境/基盤失敗が `fail` に潰れる制限は残る。したがって slice 2 の cause class は raw output と実行観測から evidence 側で記録し、full cause classifier は実装しない。

### 9.2 選択実行と反復手順

1. 選択実行 interface は `KANI_PACKAGE` と `KANI_HARNESS` として実装済みである。`K-RJ-04`・`K-RJ-03` は `KANI_PACKAGE=synvelis-recording KANI_JOBS=1` で実行し、実測コマンドと raw output を `evidence/sv659/formal-proof-runs.jsonl` に記録した。`K-RJ-01` を含む重い replay/round-trip proof は未実行である。
2. 既定 `-j4` で memory class が疑われたら、proof boundを変えずに `KANI_JOBS=2`、次に `KANI_JOBS=1` で再実行する。
3. 同一 harness を同一環境で 2 回実行し、wall-clock/peak memory/終了形を比較する。1回だけ成功しても不安定なら pass とせず、再現条件を書く。
4. WSL VM crash が出たら同じVMで連続再試行せず、Windows event/WSL journal/sibling CBMC有無を記録してから再起動またはクリーン環境で再実行する。
5. 全件 lane の成功は代表 proof 群の合否集約であり、未対象領域の保証ではない。harnessごとの verdict を evidence に残す。

## 10. Ruviform への還元判定

Ruviform へ還元すべきものは、製品固有 Property ではなく、複数製品で再利用できる runner/evidence 基盤だけである。具体的には、harnessごとの verdict/cause/resource 使用量を機械可読に残す実行器、Kani/WSL環境検出、選択実行・再実行計画、assurance metadata schema が候補になる。一方、Journal frame、WireRequest、GrantPolicy、SessionState、Recording recovery は Synvelis の製品契約なので Ruviform に移さない。Ruviform 側の proof や runner pass を Synvelis 製品契約の pass へ転用もしない。

`rf#1015` は verification evidence/model 周辺の既存課題として参照するが、slice 1/2 だけでは要求差分と重複範囲を確定できない。したがって新規起票は行わない。後続 slice で runner 分類実装の必要差分が確定した時点で `rf#1015` と重複しないか確認し、必要なら「製品非依存の検証runner evidence schema」として起票し、Synvelis側 Issue #659 と相互リンクする。確定できないまま重複起票しない方が安全である。

## 11. 次 slice への引き継ぎ

- `K-RJ-04` の solver cost 原因調査と分割可否、`K-RJ-01`/`K-RJ-02`/`K-GJ-01` の実走、既存 pass 2 件を含む回帰、harnessごとの peak memory 計測と full cause classifier。
- Authorization pure core の抽出可否を判断し、Wire scan・存在秘匿・descriptor・budget・grant次元・scope/effectsの代表BEを設計どおり実装するかを決める。
- Recovery ordering model の atomic action/仮定を確定し、TLA+ 等の tool-fit と最小modelを作る。
- mutation は oracle 独立性を壊す自己参照を避け、少なくとも CRC除去、seq検査除去、wire forbidden token削除、deny先攻順序、budget消費条件の代表的変異を計画する。
- slice 2 は selective interface、`K-RJ-03` の bounded pass、`K-RJ-04` の solver NotRun までを確定した。重い replay/round-trip proof、Authorization、Recovery ordering、mutation、full cause classifier、全件 cost comparison は未実装または未実走であり、証明済みとして扱わない。
