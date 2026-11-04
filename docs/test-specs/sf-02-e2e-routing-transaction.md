---
id: "TSPEC-034SF02Xa1sPq8rU1wC6dEgHi"
type: "test-spec"
status: "approved"
title: "SF-02 E2E/契約テスト仕様 — RoutingTransaction Reject/不変/Undo/Revision"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/105"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/designs/sf-02-routing-transaction-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/designs/sf-02-routing-transaction-design.md"
  verified-by: []
  references: []
---

# SF-02 テスト仕様 — RoutingTransaction / Semantic+Generic Validation

対象: `crates/synvelis-project/tests/routing_sf02.rs`。
実行: `mise run test`(headless)。実機/実 Plugin/GUI は NotRun/Unsupported。

## Feature: 原子 Commit と Preview(SF-TX-01/02, SF-VALID-01)

| Rule | Example | 期待 |
| --- | --- | --- |
| Preview は Project 不変 | `preflight` 実行後に live project/revision/undo を確認 | 全て不変・TempRef→Stable ID receipt のみ返却 |
| CreateBus+Connect が 1 Commit | `CreateBus{temp}+CreateConnection{OfStrip(Bus(Temp))}` を apply | revision+1・undo 深さ+1・接続確立 |
| CreateSend が 1 Commit | source/dest + Create level を apply | SendConfig+connection+gain target が 1 undo で生成・undo で全消滅 |
| 途中失敗は全不変 | 後続 edit が不正な intent を apply | reject・project/revision/undo/接続/target 数不変 |
| 同一 gesture で bus strip の main へ接続 | `PortRef::OfStrip` が新 strip の main_out を解決 | receipt と接続端点が Stable ID で一致 |

## Feature: Reject 分類(SF-TX-03..06, SF-VALID-06/07)

| Rule | Example | 期待 |
| --- | --- | --- |
| stale revision | 古い expected_revision で apply | `StaleRevision`・Project 不変 |
| stale port generation | binding 再解決後に旧 generation で apply | `StalePortCapability` |
| dangling/unknown port | 存在しない SemanticPortId を端点に | `DanglingReference`/`UnresolvedReference` |
| unresolved plugin port | Unresolved binding の port を端点に | `UnresolvedPort`・保存可能だが可聴接続に使えない |
| invalid source role | `main_in` を source にした Main 接続 | `InvalidDomainOrDirection` |
| domain 混線 | Audio→Control/TimedEvent 端点(型がある範囲) | `InvalidDomainOrDirection` |
| multiplicity 超過 | 一意 role の port へ 2 本目接続 | `GenericGraphRejected` または DuplicateOwnership |
| channel map 未同意 | 2ch→6ch、map なし consent None | `RequiresUserDecision` |
| channel map 不正 | map size ≠ src.channels | `UnsupportedChannelMap` |
| 同意+正規 map | map+ExplicitMaps | 受理・ExplicitChannelMap が保持 |
| cycle | A→B→A の enabled main | `FeedbackCycle` または `GenericGraphRejected` |
| bus 参照の dangling send | RemoveBus 後に残る send 参照 | `DependencyInUse`/`DanglingReference` |

## Feature: シナリオ受入(SF-VALID-02/03)

| Rule | Example | 期待 |
| --- | --- | --- |
| Bass→Bus→Master | track strip→bus main→master main | 受理・send/connection 1:1 |
| Pre/Post send→return | 同一 track から pre と post の 2 send→return bus | 2 SendId/ConnectionId で独立・tap 保持・Main と混在しない |
| Kick→Compressor sidechain | track main_out→compressor sidechain_in(Sidechain kind) | 受理・Main graph には含まれず audibility 依存に現れる |
| disabled send | SetConnectionEnabled(false) 後に audibility | 該当経路は不可聴・物理削除ではない |
| 重複非 Send pair | 同一 source/dest で 2 本目の Main | reject |

## Feature: Mute/Solo audibility(SF-TX-07/08, SF-VALID-03/04)

| Rule | Example | 期待 |
| --- | --- | --- |
| 初期状態 | 新規 strip | mute/solo target は未生成・audible |
| SetMute(true) | track A mute | A の main+send 出力不可聴・master 経路のみ可聴 |
| sidechain 追従 | sidechain source を mute | その sidechain 供給は停止(destination は可聴のまま) |
| SetSolo(track) | A solo・B 非 solo | A と A→master 経路のみ可聴・B 不可聴 |
| 共有 return | A,B→同一 return、A solo | return/master 可聴・B 不可聴 |
| bus solo | bus solo で上流 track が供給 | 上流 track 経路可聴 |
| 複数 solo | A,C solo | 論理和 |
| solo 解除 | solo off 後 | mute 状態を除き全可聴へ復元 |

## Feature: Undo / Revision / 保存(SF-TX-10, SF-VALID-07)

| Rule | Example | 期待 |
| --- | --- | --- |
| 1 gesture = 1 undo | transaction commit→undo | 全変更が一度に戻る・revision 前進 |
| redo | undo→redo | 同じ Stable ID で復元 |
| Undo は Project-only | undo 後に runtime 状態は問わない | Project 内容復元のみを観測 |
| save/load 往復 | transaction 適用→save→load | receipt 済み接続/send/target が保持・channels 保持 |

## NotRun / Unsupported

- 実 PCM 可聴・実 Device/Plugin/GUI/OS 検証・multi-window 実 UI:
  headless 範囲外(NotRun)。SF-03/#114 で別途検証。
- Raw Recording PCM 不変・Hardware direct monitor: 範囲外と明示し
  Synvelis mute/solo の影響を偽らない。
