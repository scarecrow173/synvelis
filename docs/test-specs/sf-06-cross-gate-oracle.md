---
id: "TSPEC-034V2wDAU9rjXjn7UX79qG"
type: "test-spec"
status: "approved"
title: "SF-06 cross-gate テスト仕様 — headless oracle・fault・migration"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added: []
    modified: []
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
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/designs/sf-06-cross-gate-oracle-domain-evidence.md"
    - "docs/tasks/sf-06-cross-gate-oracle-headless.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-06 cross-gate テスト仕様 — headless oracle・fault・migration

## Purpose

Issue #109 の headless slice が要求する「独立 oracle での
cross-gate 検証」と「拒否系 fault での状態不変」を fixture で
固定する。実 OS GUI・device・plugin・consumer・runtime engine
の lane は対象外（**NotRun**）。

## Feature

### Feature: persisted routing 契約の独立 oracle 検証

`RoutingModel`/`ControlModel` の persisted 公開フィールド
だけから期待値を導出し、SUT（`RoutingModel` の公開メソッド・
`Project::validate_references`）と field-by-field で比較する。

## Rules

- Rule: `AudibilityOracle` は `RoutingModel::audible_strips`/
  `enabled_strip_edges`/`strip_muted`/`strip_soloed`/
  `strip_of_port`/`strip_of_processor` を呼ばない — persisted
  `connections`/`ports`/`strips`/`chains`/`processors`/`buses`/
  `control.targets` の生値のみから port→strip 解決・mute/solo
  抽出（`base >= 0.5`）・enabled edge（muted source 除去）・
  upstream/downstream BFS・policy 合成を再実装する。
- Rule: audibility policy は SF-02 spec SF-TX-07/08 の
  documented 挙動 — mute は strip の全出力 edge（send・
  sidechain 供給を含む）を止め、solo が 1 つ以上あれば
  soloed strip の upstream supplier ∪（master に到達する）
  downstream のみ audible。muted strip は audible にも relay
  にもならない。solo 無しなら非 muted strip 全て audible。
- Rule: `routing_invariant_violations` は `validate_references`
  を呼ばず persisted 契約を独立に再検査する — map key==id、
  master 一意、bus↔strip 1:1、strip の chain/port
  back-reference、Main port 1in/1out、processor の単一 chain
  帰属、port owner 解決・sidechain=input のみ、connection の
  output→input 方向、send↔SendConfig 厳密 1:1（双方向）、
  非 send edge の (source,destination) 一意、gain_target 解決、
  enabled strip graph 非循環、strip 毎 Mute/Solo 権威の単一性。
- Rule: oracle は常に合格してはならない — corruption fixture
  （orphaned send・第二 master・duplicate edge・duplicate mute
  権威・dangling endpoint）で非空 violation を返すことを
  検証する。
- Rule: fault matrix の各拒否 gesture（stale CAS・stale
  capability generation・feedback cycle・duplicate temp ID・
  Send-kind の AddConnection・empty channel map・第二 master・
  参照中 delete）は reject され、かつ live revision と
  serialized project が byte 不変であること。事後の成功 op
  の undo は pre-fault project 内容へ正確に戻る（revision は
  apply+undo で +2）。
- Rule: v5→v6 migration oracle は宣言 fixture（track 数）から
  期待投影（strip 数 = track+1・Main edge 数 = track・
  processor/send 空・各 track strip→master 直結）を導出し、
  v5 envelope load の結果と比較する。再 migration は決定的で、
  save は新 snapshot head を発行する。

### Feature: Case ID registry と lane 別 evidence ledger

`evidence/reports/sf-06-cross-gate-ledger.md` が Issue #109 の
要求 schema 全 column を持ち、SF-MODEL/VALID/ACT/PROBE/UX/
COMPAT の Case ID を網羅し、実機・consumer lane を `NotRun`
で記録する。

- Rule: headless lane の Pass は `cargo test` の実測値のみを
  根拠にする。実 OS GUI・native plugin・device・consumer
  （Synvelis #219/#294）の行は `NotRun` であり、headless Pass
  から昇格しない。
- Rule: EX-001〜020（#18）・RF#193 G2/G7・RF#308/#309・
  Synvelis #114/#176/#219/#294 への link を ledger に含める。

## Examples

- Given mix topology（4 track + group + return + master =
  7 strip、pre/post send・sidechain・muted relay を含む）、
  when 全 2^14 mute×solo 組合せで persisted `ControlModel` を
  perturb、then `audible_strips` と oracle `audible` の
  `BTreeSet<StripId>` が全組合せで一致する。
- Given chain topology（a→b→c + auto→master、b の直接 edge
  無効化 variant 含む）、when 同様に sweep、then 同上。
- Given persisted model が契約を満たす fixture、when
  `routing_invariant_violations`、then violations は空かつ
  `validate_references` も Ok。
- Given 破損 fixture（send config のみ残し connection を除去
  等）、when `routing_invariant_violations`、then 非空の
  violation を返す。
- Given mix fixture に stale CAS・stale capability・cycle・
  dup temp・illegal kind/map・referenced delete を順に投げる、
  when 各 `apply`、then 全て `Err` で live revision と
  serialized project が before と一致し、末尾の成功 op +
  undo で project 内容が pre-fault へ戻る。
- Given 3 track の v5 envelope、when `load`、then routing が
  track 毎 strip + master 直結 Main edge で再構成され
  processor/send は空、再 load は `c2.live() == c3.live()`、
  save は新 head を発行する。

## Guarantees

- test は `synvelis-project` の persisted 層のみで完結し、
  GUI・engine・device・plugin・network に依存しない。
- oracle は SUT の公開 persisted フィールドと documented
  policy のみを読み、private 実装に依存しない。
- sweep は mask 昇順で決定的（`BTreeMap`/`BTreeSet` canonical
  順）。

## Non-goals

- SF-ACT-01〜08 の runtime publish/QueueFull/last-good/
  RT-Active receipt — #106 実装確定後の別 gate。
- SF-PROBE-01〜08 の capture/epoch/latency runtime 計測 —
  #107・RF#308/#309 の既存 evidence を link のみ。
- SF-UX-01〜06 の GUI/visual/Follow/Probe 共有 — #108・#114
  gate。
- SF-COMPAT-01〜06 の DAW Gate/Recording/AudioLens/lockfile —
  各担当 issue の evidence link のみ。
- EX-001〜020・RF#193 G2/G7・RF#308/#309 の再採取。
- #106/#107/#108 実装本体・Ruviform generic primitive の変更。

## Risk

- oracle が SUT と同じ誤読を共有 → corruption fixture で
  oracle 自体の検出能力を検証し、documented policy のみを
  根拠にする。
- headless Pass の実機への誤昇格 → ledger で lane を列分離し
  `NotRun` を明示。
- #106/#107/#108 の契約変更で test が陳腐化 → ledger の
  `fixture & command`/`expected vs observed` 列で再評価点を
  記録。
