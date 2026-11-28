---
id: "SPEC-sv601frmlvrf0slice1"
type: "spec"
status: "proposed"
title: "Issue #601 Formal Verification — journal wire codec + authorization boundary の proptest + Kani 検証基盤"
created: "2026-11-23"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/601"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-gateway/tests/formal_journal/"
      - type: "file"
        path: "crates/synvelis-recording/tests/formal_journal.rs"
      - type: "file"
        path: "tools/run-formal-kani.mjs"
      - type: "doc"
        path: "docs/specs/formal-verification.md"
      - type: "dir"
        path: "evidence/sv601/"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "crates/synvelis-gateway/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-recording/Cargo.toml"
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: ".agents/skills/verify-synvelis/SKILL.md"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - ".agents/skills/verify-synvelis/SKILL.md"
    - "https://github.com/scarecrow173/synvelis/issues/659"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/601"
    - "https://github.com/scarecrow173/ruviform/issues/806"
  derived-by: []
---

# Formal Verification — journal wire codec + authorization boundary（synvelis-gateway / synvelis-recording）

Issue `scarecrow173/synvelis#601` の実装 spec。
`scarecrow173/ruviform#1014` で決定され、`POLICIES.md` §6 に正本
として置かれた形式的検証原則が上位ポリシーである。本文書は Issue
#601 で導入した journal wire codec / authorization boundary の
Property・検証仕様を記録する。ここに記載する proptest + Kani lane
は固定昇格路ではなく、既存の issue-specific lane である。
Journal/Authorization 検証の再設計と移行は
`scarecrow173/synvelis#659` が担う。

slice 1 は unsafe を含まない 2 つの journal wire codec を、slice 2
は `synvelis-gateway` の authorization boundary（
`WireRequest::parse` と `GrantPolicy`）を対象に proptest を常設
する。Kani lane は slice 1 の journal codec harness を維持し、slice
2 対象は serde_json / `Arc<String>` / dyn trait 依存のため
bounded-proof 適用を保留する（tool-fit 候補評価を参照）。先行実装
`scarecrow173/ruviform#806` の lane 構成・証跡形式を踏襲する。

## 対象と検証する契約

| 対象 | ファイル | 契約 |
| --- | --- | --- |
| `replay(&[u8])` | `crates/synvelis-gateway/src/journal.rs` | 任意入力で panic しない・`stop` は閉じた `StopReason` 集合（`CleanEnd`/`TornTail`/`BadMagic`/`BadVersion`/`BadReserved`/`BadLength`/`BadChecksum`/`SequenceGap`）・`prefix_len ≤ len`・`CleanEnd` iff 全 byte 検証・検証 record の seq は 0 から連番・検証済み prefix 単体は必ず `CleanEnd` |
| `encode_frame` | 同上 | 記述 layout（LE magic `0x4A4A`/version/reserved=0/seq/payload_len/payload/crc32）を emit し、`payload_len ≤ MAX_PAYLOAD` で `Some`、超過で `None`、seq0 frame は replay で payload 同一復元 |
| `replay_journal(&[u8])` | `crates/synvelis-recording/src/journal.rs` | 同上の分類契約（`StopReason` 集合・`prefix_len`・連番 seq・`last_seq` = 最終 intact record） |
| `encode_record` / `JournalAppender` | 同上 | 記述 layout（LE magic `0x4A52`/version/kind∈{1,2,3}/seq/len/payload/crc32）。`JournalAppender::encode_next` が seq 採番を所有し、連番 stream は全 3 kind（`Chunk`/`Checkpoint`/`Finalize`）で `CleanEnd` 往復。`JournalAppender::resumed(n)` は n から採番するが、その stream 単体は seq0 起点を要求する replay に対し `SequenceGap` で止まる（途中再開の非互換を pin） |
| `JournalPayload` serde | `crates/synvelis-gateway/src/commit.rs` | 全 6 variant（`Committed`/`Rejected`/`Conflicted`/`Indeterminate`/`Policy`/`Saved`）が `serde_json` を無損失往復し、inner record の `deny_unknown_fields` が注入未知 field を拒否 |
| u128 tagged-id serde | `crates/synvelis-gateway/src/id.rs` | `"<kind>-<32 lowercase hex>"` 文字列として直列化・無損失往復・誤 tag・malformed hex（非 hex 文字・長さ 32 以外）を拒否 — u128 を JSON number として運ばない wire 契約 |
| `WireRequest::parse` | `crates/synvelis-gateway/src/wire.rs` | 任意 `serde_json::Value` で panic しない・`Err` は閉じた 3 分類（`MalformedRequest`/`SelfDeclaredAuthority`/`ImplicitTargetRejected`）のみ・`FORBIDDEN_AUTHORITY_FIELDS` の全 key は top-level で `SelfDeclaredAuthority`（注入 field 名を echo、implicit より先に評価）・`FORBIDDEN_IMPLICIT_TARGET_FIELDS` は `ImplicitTargetRejected`・同 key を `args` 等の非走査位置に nest しても拒否にならない（top-level 限定）・受理入力は allow-list 構造契約を満たす（test 内の独立 model checker で検証）・`capability`/`schema`/`target`(既知 shape)/`consistency`(既知 kind) 内部の未知 key は `MalformedRequest`（internally tagged enum で `deny_unknown_fields` が無効な分を explicit scan で補う） |
| `GrantPolicy`（`decide`/`evaluate_commit`/`evaluate_effects`） | `crates/synvelis-gateway/src/policy.rs` | 決定は閉じた `AuthorizationDecision` 集合・deny reason は契約文字列集合（test 内 `DENY_REASONS` 13 個に pin）・未 bind agent は全 8 stage で一律 `"no applicable authorization"`（存在秘匿）・trusted human は descriptor 一致で全 stage `PermitForStage`（descriptor mismatch は human 短絡より先に deny）・`DelegatedApply`+`Commit` は apply budget を単調消費（Permit 1 回 = 1 unit・枯渇後は `"apply budget exhausted"`・`try_consume_apply` と整合）・preflight と全 `Deny` は `consumed_applies` を動かさない・`evaluate_commit` ≡ `decide(Commit)`（同一評価・同一 spend）・scope coverage は再評価・resource 重複 dedup で不変・未解決/未カバー resource は `target outside grant scope`・create 欠陥は `creation outside grant constraints`・`effects.unsupported` 非空は `unverifiable effects`・disclosure/transfer が `data_access` 外なら `data access outside grant` |
| `crc32` / `Crc32` | `crates/synvelis-recording/src/crc32.rs` | 一括 `crc32` と逐次 `Crc32::{new,update,finish}` が任意分割点で一致（table-driven 同一実装の 2 経路） |
| `digest_fold` / `IntegrityDigest` | 同上 | `acc*K^v` fold が決定的（raw `digest_fold` 連鎖 = `IntegrityDigest::fold` 連鎖）・`to_hex` は固定 16 文字 lowercase hex |
| session SM | `crates/synvelis-recording/src/session.rs` | `legal_transition` と `require_transition` が 4×4 pair 全域で一致・legal は `Created→Open`/`Open→Closed`/`Closed→Finalized` の 3 forward edge のみ・`accepts_registration` iff `Open`・`accepts_take_verification` iff `Open\|Closed` |
| corruption 耐性 | 両 journal | frame の全 byte は直接 field 検査または末尾 CRC で覆われるため、record k 内部の 1 byte 変異は絶対に k 以降を intact 化できない（replay は k の直前で必ず停止） |

## Lane 構成

| Lane | 実行 | 環境要件 |
| --- | --- | --- |
| `mise run test-formal` | `cargo test --locked -p synvelis-gateway -p synvelis-recording --test formal_journal` | なし（pure Rust・MSRV 内・CI と同一入口） |
| `mise run test-formal-kani` | `node tools/run-formal-kani.mjs` → `cargo kani --tests -j4 --output-format terse -p synvelis-gateway -p synvelis-recording` | ネイティブ `cargo kani`、または `cargo` + `cargo kani` を持つ WSL2 ディストロ |

`test-formal` は追加環境なしで常時実行できる property-test
lane。`test-formal-kani` は環境検出つきの bounded-proof lane
（manual/opt-in — CI-required gate ではない）で、
`tools/run-formal-kani.mjs` が次の順に解決する。

1. ネイティブ `cargo kani` があればそれを使う。
2. なければ `wsl.exe -l -q` でディストロを列挙し、`cargo` と
   `cargo kani` の両方が PATH にあるものを先着で採用する
   （`docker-desktop` は除外 — kani を抱かない実行専用 distro）。
3. カレントの Windows パスを `wslpath -a` で `/mnt/<drive>/...` に
   写像し、WSL 側は
   `CARGO_TARGET_DIR=$HOME/.cache/synvelis-kani/target` の WSL
   ネイティブ領域で実行する（`/mnt` 越しのビルド IO を避け、
   Windows 側の managed target dir も不変に保つ）。
4. どちらも見つからなければ `not_run`（`environment_blocked`）を
   JSON verdict で報告して exit 0 — 環境欠如は fail でも pass でも
   ない。真の検証失敗（CBMC が反例を返した場合）のみ `fail` +
   exit 1。

並列度: runner は `cargo kani -j${KANI_JOBS}`（default 4、rf806
precedent と同一）を渡す。CBMC solver は 1 プロセスあたり数 GiB を
消費するため、共有・小 RAM ホストでは `KANI_JOBS=2`（完全 serial は
`KANI_JOBS=1`）で peak memory を抑える。これは証明 bound ではなく
環境リソースの調整であり、証明内容は変わらない（slice-1 実測:
`-j4` 時に sibling CBMC ワークロード併存の WSL2 31 GiB ホストで
OOM-kill が発生 → `-j2` で再実行）。

## proptest の bound と coverage 仮定

proptest は pseudo-random 生成 + shrink で「緑」を得る — bound 内の
網羅ではなく、bound を正直に記録する。

| 生成器 | Bound | 補完 |
| --- | --- | --- |
| 任意 byte 列（replay 系） | 0..=512 byte | 構造を持つ入力は encode 済み stream + 変異系 property がカバー |
| journal payload body（gateway） | 0..=384 byte/record・0..=8 record | `MAX_PAYLOAD`(16 MiB) cap は `encode_frame_at_cap_roundtrips`（実測）+ `encode_frame_over_cap_refused`（4 case）で担保 |
| `RecordPayload`（recording） | spans 0..=6・0..=8 record | `u16` count field 飽和 + `MAX_PAYLOAD`(1 MiB) cap は `chunk_record_at_cap_boundary`（fixed 58 byte + 48 byte/span → exact 21844 = in-cap・21845 = over-cap）で担保 |
| `JournalPayload` | 全 6 variant・nested policy graph は bounded（refs ≤32 char・sets ≤4・state ≤256 byte） | `CommitRecord`/`Grant` 全フィールド生成 |
| u128 tagged-id | 任意 u128 | tag 文字列形状は `check_tagged_id` が kind 別に pin |
| 任意 JSON（`WireRequest::parse` no-panic/閉集合） | 深さ ≤4・container あたり ≤8 child・文字列 ≤24 char・object key は通常識別子に forbidden key を低頻度混合 | near-valid `arb_wire_request` が受理側構造をカバー・forbidden 注入系 property が分類を個別 pin |
| near-valid wire request | 構造適合の required 5 field + optional（target/args/task/grant/idempotency_key） | 受理判定は test 内 model checker — 製品内部表現の写しではなく契約の独立再実装 |
| policy spine（`GrantPolicy`） | task/grant/principal/project/adapter の id 列のみ生成。record/Oracle/Rig は defect 軸で構築し fresh `InMemoryPolicyStore` ごと使用（budget ≤5・commit 反復 ≤budget+2・request は `parse_wire`+`bind` の実経路のみ） | 権限・binding・scope の組合せ爆発は defect 駆動生成で抑止 — 境界到達を保証するため生成時に整列させる |

仮定: proptest の case 数は既定（256/property、cap 系のみ 4 case）。
policy suite のみ case ごとに rig 全体を構築するため 64 case に
抑える。「閉じた `StopReason` 集合」は型の exhaustiveness で
compile-time に保証される（新 variant 追加はコンパイルエラー）
— runtime assertion ではなく型で pin する設計。
`WireRequest::parse` の「閉じたエラー分類」と `GrantPolicy` の
「閉じた決定集合」も同じく exhaustive match と deny-reason 文字列
集合への pin で担保する。

## Kani の bound と仮定

証明は全入力ではなく bounded symbolic input に対する exhaustive
検証（CBMC の性質上、未証明領域は残る — 正直な bound 記録が必須）。

| Harness | Bound | 未証明領域 | 補完 |
| --- | --- | --- | --- |
| `gateway::replay_contract_bounded` | stream = 長さ ≤44 byte の任意 byte 列。`#[kani::unwind(64)]` | 44 byte 超 stream・`MAX_PAYLOAD` cap 経路（bound 内では構造的に到達不可） | proptest `replay_arbitrary_bytes_contract`（≤512B）+ cap 実測 test |
| `gateway::encode_replay_roundtrip_bounded` | payload = 固定 12 byte 任意列・seq0。`#[kani::unwind(48)]` | symbolic 長 payload（`Vec` 確保・memcpy の symbolic bound unwind を避けるため固定長） | proptest `encode_seq0_replays_clean`/`encode_stream_roundtrip` |
| `gateway::nonzero_seq_is_gap_bounded` | payload = 固定 8 byte 任意列・seq = 0 以外任意 u64。`#[kani::unwind(48)]` | —（全 seq≠0 を symbolic に尽くす） | proptest `encode_nonzero_seq_is_gap` |
| `recording::replay_journal_contract_bounded` | stream = 長さ ≤44 byte の任意 byte 列。`#[kani::unwind(64)]` | 44 byte 超 stream・cap 経路 | proptest `replay_journal_arbitrary_bytes_contract` + cap 実測 test |
| `recording::encode_replay_roundtrip_bounded` | `Checkpoint`/`Finalize`（ともに固定長 payload）の全 field symbolic・seq0。`#[kani::unwind(64)]` | `Chunk` kind（`Vec<JournalSpan>` は symbolic span count に対し `Vec::with_capacity` で symbolic 確保 — CBMC コストのため除外）・symbolic payload 長 | proptest `encode_record_seq0_replays`（全 3 kind）+ Chunk cap 境界 test |
| `recording::crc32_split_equivalence_bounded` | input = 固定 16 byte 任意列・split ≤16 symbolic。`#[kani::unwind(24)]` | 16 byte 超入力・3 以上の分割 | proptest `crc32_incremental_matches_oneshot`（≤256B・≤4 分割） |
| `recording::session_transitions_exhaustive` | 4×4 = 全 16 pair concrete 展開 + 各 state の acceptance gate。`#[kani::unwind(8)]` | —（状態空間を全域網羅 — bound ではなく全数証明） | proptest `session_transition_agrees_with_require`/`session_acceptance_gates_are_total` |

仮定:

- `kani::assume(len <= STREAM_LEN)` で stream 長を bound 内に制限
  （それ以外の入力制限 assume は置かない — 契約を弱めるため）。
  `nonzero_seq_is_gap_bounded` の `assume(seq != 0)` は証明対象の
  前件（non-zero seq の場合の契約）であって入力絞り込みではない。
- `STREAM_LEN = 44`（gateway/recording 共通）は「最小 record 2 件
  （header 16 + crc 4）+ slack」を根拠に選定。recording 側では追加の
  効果がある: Chunk payload の `span_count` field は payload offset
  26 にあり、`payload < 28` byte なら decode cursor が先に尽きて
  `Vec::with_capacity(span_count)`（symbolic count への symbolic
  確保）へ到達しない。44 < 48 byte によりこの CBMC 高コスト経路を
  構造的に排除しつつ、全 `StopReason` 経路は到達可能のまま。
- `#[kani::unwind(N)]` は replay/crc loop（gateway crc は bitwise
  実装で byte あたり内側 8 loop、recording は table-driven で byte
  あたり 1 loop）の unwind 上限。bound 超過は unwinding
  assertion として検出される — silent な未完検証ではない。
- serde_json は Kani 対象外: `JournalPayload` の serde 往復・
  `deny_unknown_fields`・tagged-id 契約は proptest 専任（serde の
  CBMC 変換コストが非実用的 — rf806 の実測所見と同判断）。
- symbolic `&str`/`String` を必要とする型は harness に持ち込まず、
  固定長 byte 列と Copy struct 群に限定する。

## Model 対 Product の区別

本 slice が検証するのは **製品実装そのもの** （`replay` /
`encode_frame` / `replay_journal` / `encode_record` / `JournalAppender` /
`crc32` / `digest_fold` / session SM / `JournalPayload` serde は全て
製品 source を直接呼ぶ）。codec の「モデル写し」は存在しない —
property suite の oracle は契約 predicate（`prefix_len ≤ len`・連番
seq・`CleanEnd` iff 全消費・期待 layout byte・期待 stop class）と、
期待値の独立再計算（`ends[]` からの record 境界再構成、独立な crc32
呼出しによる frame 検査）のみ。状態機械や数学的定理について
TLA+ spec・Lean proof 等の model-based technique を選ぶ場合は wire
layout 等の「モデル」を別途書くことになるが、本 slice の正本は製品
実装であり、model/product 乖離リスクは mutation-fail
証跡（下記）で「predicate が実装破壊を実際に検出できる」ことの側
から閉じる。

slice 2 も同じ方針で、加えて 3 つの独立判定を test 側に置く。
`WireRequest::parse` の受理側は test 内の **独立 model checker**
（top-level/必須/nested allow-list を契約から再実装した
`model_allows`）を通し、「parse が受理した入力は必ず model を
満たす」向きで pin する — 製品内部の `deny_extra_keys` 表の写し
ではなく、乖離があれば property が fail し、mutation-fail 証跡が
検出能力を裏付ける。拒否側の注入は製品 const ではなく test-local
な literal list（`AUTHORITY_FIELDS` / `IMPLICIT_TARGET_FIELDS`）
から生成し、`documented ⊆ product` の subset property で pin する
— 製品表から entry が消えても注入は契約どおり残り、削除が
「test 空間の縮小」として隠れない（`DENY_REASONS` の literal pin
と同じ理屈。実際に `"token"` 削除 mutant が検出不能だった探査 run
を経てこの構造になった。`evidence/sv601/verification.md` slice 2
節参照）。`GrantPolicy` の oracle は契約 predicate（閉じた決定集合・
deny reason 集合・一律 NO_AUTH・scope 欠陥の固定 reason）と
store 側 counter（`consumed_applies` の単調性・
`try_consume_apply` との整合）の独立再検査であり、これも製品実装
を直接呼んで判定する。

## Kani 環境のプロビジョニング

`test-formal-kani` が `not_run`（`environment_blocked`）を返す環境
では、次のいずれかを用意する。

- Linux / macOS: `rustup` + `cargo install --locked kani-verifier` +
  `cargo kani setup`（CBMC 等の solver は `kani-verifier` の
  setup が取得する）。
- Windows: WSL2 ディストロ内で同上。lane は `wsl.exe -l -q` で
  列挙した各ディストロに `cargo` + `cargo kani` があるかを先着
  検出し、カレント Windows パスを `wslpath` で `/mnt/<drive>/` へ
  写像して実行する。ビルド成果物は WSL ネイティブ側
  （`CARGO_TARGET_DIR=$HOME/.cache/synvelis-kani/target`）に置き、
  `/mnt` 越しの遅い IO と Windows 側 managed target の混線を避ける。

solver 取得はネットワーク依存のため、CI で Kani を必須化する場合は
イメージ焼き込み等の別途検討が必要（本 slice の scope 外 — lane は
manual/opt-in）。

## Mutation-fail 証跡（「fail できること」の証明）

プロパティ suite は緑であることだけでなく、壊れた実装を検出できる
ことが証拠として必要。手順:

1. 対象関数の契約を 1 箇所だけ一時的に破る（例: `replay` の CRC
   検査を skip する、seq 欠番検査を外す、recording の kind gate を
   緩める）。
2. `mise run test-formal`（または対象 `--test` 単独）を実行し、
   該当 property が shrink つきで `fail` することを記録する。
3. 変異を完全に revert し、緑に戻ったことを再実行で確認する。

変異・観測した fail・revert の 3 点セットを `evidence/sv601/` に
記録する。変異の持ち越し（revert 漏れ）は禁止 — verification.md に
revert 後の再検証結果を残す。source 一時変異の占有は coordinator
へ申請してから行う。

## 判定語彙と fail-closed

`pass` / `fail` / `not_run` / `unsupported` / `blocked` — verify-
synvelis skill の実行 verdict 語彙に従う。環境欠如は `not_run` +
`environment_blocked` で exit 0、証拠のない lane を `pass` と記録
しない。ローカル検証は変更範囲の検証であり、merge 後の対象 SHA
に対する必須 CI 結果が統合検証の証拠（本記録はそれを代替しない）。

実行 verdict は保証そのものではない。保証種別（Example Tested /
Property Tested / Bounded Exhaustive / Model Checked / Theorem
Proved 等）、対象（実装 / モデル / 抽象化）、同期状態、失敗原因
（Product Defect / Model Error / Resource Limit / Environment 等）
は独立軸として記録する。既存語彙と機械可読形式は維持し、不足情報
を補う。Timeout・OOM・環境不足・exit 0・CI green を Proof Pass と
しない。現行 runner の失敗原因分類は限定的であり、再設計は
`scarecrow173/synvelis#659` が担う。

## synvelis#659 slice 1 の同期点（2026-11-26）

本節は Issue #601 当時の記録を置き換えるものではない。slice 1 で確定した
設計判断のうち、本 spec と直接関係する同期点だけを記録する。完全な
inventory、必要保証、境界、未証明領域、Authorization 分離案、Recovery
ordering inventory、失敗分類、再実行設計は
`docs/designs/issue-659-verification-redesign.md` を正本とする。最終的な
Verification Specification の再構成と証跡/手順分離は `scarecrow173/synvelis#662`
が担うため、本節はその構成を先取りしない。

- 対象 inventory は Property/Example Test 50 件（gateway journal 14、wire 9、
  policy 12、recording 15）と既存 Kani proof 7 件である。対応表の補助証跡は
  `evidence/sv659/inventory.json`。
- Journal/CRC32/session の代表的重要 Property は `K-RJ-01`、`K-RJ-02`、
  `K-RJ-03`、`K-RJ-04`、`K-GJ-01` とする。`K-GJ-02`/`K-GJ-03` は pass 済みの
  回帰 proof として維持する。いずれも bound 内の局所保証であり、任意長、
  cap 経路、Chunk allocation、永続化 recovery、製品全体への転用はしない。
- Authorization は WireRequest 解釈、bind/Operation 解釈、純粋認可判定、
  外部 store/oracle/clock の 4 層に分けて検証する。serde 結合部は proptest、
  pure structural scan と pure decision core を bounded proof の候補とする。
- Recovery/操作順序は crash point をまたぐため TLA+ または同等の ordering
  model が必要と判定した。model 本体、atomic action、同期仮定は次 slice で確定する。
- Kani 失敗は timeout/OOM/環境不足/exit 0 と証明結果を分け、
  `environment_absent`、`path_mapping`、`compile`、`symex`、`solver`、`memory`、
  `parallelism`、`infrastructure`、`product_oracle_failure` の原因分類で記録する
  設計とした。runner 実装は slice 1 では変更しなかった。

Issue #601 当時の lane・bound・tool-fit 評価は引き続き現行実装の説明として
有効だが、新規 proof の優先順位と保証転用禁止の解釈は #659 の matrix に従う。

## synvelis#659 slice 2 の同期点（2026-11-26）

本節は selective proof の再実行方法と実走結果だけを同期する。Verification
Specification の再構成と証跡/手順分離は引き続き `scarecrow173/synvelis#662`
に委譲する。

- `tools/run-formal-kani.mjs` は env による選択実行を受け付ける。
  `KANI_PACKAGE=synvelis-recording` または `synvelis-gateway` は 1 package
  だけを `-p` 対象にし、`KANI_HARNESS=<filter>` は `cargo kani --harness`
  へ 1 回だけ渡す。未知の package、空文字、whitespace-only harness は
  verification 開始前の `configuration_error` で nonzero 終了する。複数
  filter、glob、正規表現、exact 指定の独自文法はない。`KANI_JOBS` の既定は
  4 で、本 slice の selective run は 1 を使った。
- 選択実行の JSON verdict は lane、verdict、reason、package/packages、
  harness、jobs、environment、elapsed seconds、exit code、match した
  harness 件数を含む。zero-match probe では Kani が `Please specify the
  fully-qualified name of a harness.` を返し、runner は
  `harnesses_matched=0`・exit 1 を記録した。0 harness を pass としない。
- `K-RJ-03`（`crc32_split_equivalence_bounded`）は sync
  `24d2e2a25291d4e8ec75c7f1ea41bad049421be5`・WSL2 Ubuntu・Kani 0.68.0・
  CBMC 6.11.0 で Pass した。結果は `VERIFICATION:- SUCCESSFUL`、0/131
  failed（1 unreachable）、Verification Time 282.0517s、runner wall-clock
  310.115s である。保証は固定 16 byte・split ≤16 の incremental/one-shot
  同値に限られ、任意長、3 以上の分割、暗号学的衝突耐性は対象外である。
- `K-RJ-04`（`session_transitions_exhaustive`）は同じ sync・環境で solver
  が 300s 予算内に完了せず、runner wall-clock 634.49s で invocation を外部
  停止した。反例と unwinding assertion は出ていないため、cause class
  `solver` の NotRun である。製品欠陥や proof 成功として記録しない。
- 詳細な invocation 証跡は `evidence/sv659/formal-proof-runs.jsonl` と同じ
  ディレクトリの raw output artifact に記録する。full lane（全 7 proof
  一括実行）と `K-RJ-01`/`K-RJ-02`/`K-GJ-01` は本 slice では実行していない。

## synvelis#659 slice 3 の同期点（2026-11-26）

本節は selective proof の cost、mutation matrix、再実行方法だけを同期する。Verification Specification の再構成は引き続き `scarecrow173/synvelis#662` に委譲する。

- 低コスト selective proof は sync `34e69fb27774f41f6aded1a24320be90ffd7477a`・WSL2 Ubuntu・Kani 0.68.0・CBMC 6.11.0・`KANI_JOBS=1` で再実行した。K-GJ-02 は Verification Time 33.076668s・runner wall-clock 104.125s、K-GJ-03 は 24.030256s・67.862s、K-RJ-03 は 234.04366s・288.605s で、いずれも raw output に `VERIFICATION:- SUCCESSFUL` がある。runner wall-clock は compile/loading を含むため Verification Time と混ぜない。
- 接続条件は限定的である。K-GJ-02 は固定 12 byte・seq0・単 record、K-GJ-03 は固定 8 byte・任意非零 seq・単 record、K-RJ-03 は固定 16 byte・split ≤16 の 2 分割だけを示す。可変長、複数 record、full stream、resume/recovery ordering、Chunk/cap 境界、FS durability、暗号学的耐性は残る。
- mutation matrix は M-GJ-CRC、M-RJ-SEQ、M-WIRE-TOKEN、M-POL-DENY-ORDER、M-POL-BUDGET の 5 件で、すべて killed だった。各 mutation は 1 件ずつ適用し、canonical `mise run test-formal` で exit 101 と検出 test を記録し、完全 revert 後に exit 0 へ戻した。mutation killed は oracle の検出能力証拠であり、Property の assurance kind を引き上げない。
- machine-readable な正本は `evidence/sv659/formal-proof-runs.jsonl`、`evidence/sv659/mutation-runs.jsonl`、`evidence/sv659/cost-mutation-matrix.json` とする。historical baseline として sv601 の K-GJ-02 45.5s・K-GJ-03 31.4s、slice2 の K-RJ-03 282.0517s/310.115s を併記するが、環境差・cache 状態差から性能因果は断定しない。
- full lane（全 7 proof 一括実行）、K-RJ-01/K-RJ-02/K-GJ-01/K-RJ-04、Authorization pure core、Recovery ordering model は本 slice でも実行していない。

## synvelis#659 slice 4 の同期点（2026-11-28）

本節は K-RJ-04 `session_transitions_exhaustive` の boundary 再設計、実走結果、再実行方法だけを同期する。Verification Specification 全体の再構成は引き続き `scarecrow173/synvelis#662` に委譲する。

- selective rerun は `KANI_PACKAGE=synvelis-recording KANI_HARNESS=session_transitions_exhaustive KANI_JOBS=1 mise run test-formal-kani` である。slice 4 の実行では runner が timeout を持たないため shell 側の外部 wrapper で budget を監視したが、wrapper は発火せず自然終了した。Pass 判定は raw output の `VERIFICATION:- SUCCESSFUL`、0/269 failed（4 unreachable）、runner verdict `pass`、exit 0、runner JSON `harnesses_matched=1` である。
- 結果は sync `dbddb52f20d8a98e9f5a9d6e1a6571307cfb7659`・WSL2 Ubuntu・Kani 0.68.0・CBMC 6.11.0 で Pass、Verification Time 5.1956944s、runner wall-clock 30.248s、shell wall-clock 32s だった。runner/shell の時間には compile/loading を含むため Verification Time と混ぜない。
- boundary は 2 層である。test-only 側では proptest strategy・Property Test・concrete boundary test を `cfg(not(kani))` で Kani codegen 対象から外すが、通常の `mise run test-formal` では同じ 15 件を維持する。製品側では `require_transition` が返す illegal-transition error text を有限 static lookup から構成する。文字列契約、状態数、3 forward edge、acceptance gate、harness 名、`kani::unwind(8)` は変えない。
- 切り分けの結果、proptest-only test code は solver 支配因ではなかった。cfg-only でも proof は完了しなかった。支配要因は `format!` 経由の illegal-transition error construction で、GOTO symbol map では `format` 39 entry・`fmt` 738 entry・`alloc` 1109 entry だったものが、再設計後は `format` 0・`fmt` 42・`alloc` 618 へ減り、proof は 5.1956944s で完走した。peak memory は計測基盤がないため unmeasured とする。
- Pass の保証は `SessionState` 4×4 pair と acceptance gate の有限全域に限る。recording workflow、crash recovery、永続化 ordering、製品全体へ転用しない。

## Tool-fit 候補評価（再設計は synvelis#659）

以下は固定順序ではなく、Property が要求する保証強度に対する
tool-fit 候補の記録である。適用可否・境界・予算は `POLICIES.md`
§6 に従って個別に決める。Journal/Authorization 検証の実際の再設計
と移行は `scarecrow173/synvelis#659` に委譲する。

### Kani（bounded model checking）候補

| 候補 target | 対象 crate / 契約 | Kani が向く理由 |
| --- | --- | --- |
| `WireRequest::parse` | `synvelis-gateway` wire layer — forbidden authority/implicit-target field gate・envelope 検証 | wire codec と同型 — bounded symbolic byte 列/field 列に対し「panic なし・拒否分類が安定集合・受理なら構造契約」を尽きられる。**slice 2 で proptest 化済み**（`tests/formal_journal/wire.rs`）。Kani への適用は保留: 入力が `serde_json::Value`（CBMC 変換コスト非実用 — slice 1 と同判断）で、JSON object の symbolic 生成は harness 側で bounded key/field 列に写す設計が別途要る |
| `GrantPolicy::evaluate_commit` | `synvelis-gateway` — grant/task/epoch/scope/effects の評価分岐 | 有限分岐の決定関数 — bounded symbolic `Grant`/`Task`/`scope` 入力で「決定が安定集合・deny なら拒否理由が契約内」を尽きられる。**slice 2 で proptest 化済み**（`tests/formal_journal/policy.rs`）。Kani への適用は保留: `Arc<String>`・`Arc<dyn PolicyStore>`/`Arc<dyn ResourceOracle>`/`Arc<dyn PolicyClock>` の trait object 境界が symbolic 化の前に決定的 mock 注入への設計を要求し、`String`/`BTreeSet` 系の CBMC コストも残る |
| CommitGate CAS / idempotent ledger | `synvelis-gateway` — expected-revision CAS・`JournalIdem` keyed retry の replay 契約 | 有限状態の ledger 不変条件（同一 key は同一 outcome を replay・CAS 衝突は Conflicted）を bounded trace で尽きられる |

### TLA+ 候補（ordering / state machine）

| 候補 target | 契約 |
| --- | --- |
| WorkflowCoordinator FSM replay | replay が FSM を一意に再構成し、中断再開で分岐しない ordering 契約 |
| Recording crash recovery | torn tail 切詰め・`OpenReport` 報告・reopen 後追記の crash/再起動 ordering |
| CommitGate ordering | revoke/commit の journal order・fence 内再評価と append の原子性 |

### Lean 候補（全サイズ算術不変条件）

| 候補 target | 契約 |
| --- | --- |
| PPQN / routing / mix-order 算術 | tick 変換・graph 辺順序・mix 累積の overflow なし・単調性など、bounded ではなく全サイズの証明が要る算術 |

### property-only（proptest 継続候補）

- `syn.*` operation-sequence 契約: `AppService` 経由の operation
  列に対する replay/冪等/outcome 安定性 — 生成列が複雑で symbolic
  化の費用対効果が薄いため proptest を主たる手法とし、契約の硬い core
  が切り出せた時点で Kani 適用を個別に評価する。

本 slice で確立した「coverage 記録 + sync commit + 1 コマンド
再実行 + mutation-fail 証跡」の型を、Property に対して選定された
検証手法にも適用する。
