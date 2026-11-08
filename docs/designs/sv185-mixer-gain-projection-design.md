---
id: "DESIGN-Xk9mQ2vR7wTzLp4nHs8dFj"
type: "design"
status: "approved"
title: "SV185-01 mixer.gain projection 設計 — ControlModel bootstrap・wire target・display_value"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/185"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv185-01-mixer-gain-domain-projection.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv185-01-mixer-gain-domain-projection.md"
    - "docs/designs/sv175-native-shell-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/sv185-mixer-gain-projection.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV185-01 mixer.gain projection 設計

## 決定

### D1. projection 元は `ParameterTarget`（既存 domain 概念）

`mixer.gain` という別名・別 state を新設しない。domain の既存
抽象である `ParameterTarget`（`ParameterRole::Fader` singleton、
`ControlOwner::Strip`）を master strip に対して bootstrap する。
これにより automation lane・modulation assignment・gesture
（`gesture_begin`/`gesture_update`/`control_gesture_end` →
`SetParameterBase`、1 undo）・inspector projection が全て既存の
authoritative 経路にそのまま乗る。UI 側に第二の mixer state を
作らない（AGENTS「正しい境界で修正する」）。

### D2. ID は reserved well-known range の定数

`MASTER_FADER_RAW = WELL_KNOWN_BASE + 6`（`synvelis-project::control`）。
routing bootstrap が `+1..=+5` を使うため control 側は `+6` を継続
割当する。`is_issuable_raw` の外なので `id_watermark` 比較・
`IdGenerator` 発行域と衝突しない。`RemoveParameterTarget` の
validate で `!is_issuable_raw(target.raw())` を拒否し、bootstrap
entity が op で消えない不変条件を守る（master bus exactly-one と
同じ構造的不変条件）。

wire target は `ParameterTargetId::new(MASTER_FADER_RAW).to_tagged()`
= `param-<32hex>`。`mixer.gain` alias は作らない — Issue の
「または同等の domain 概念」に該当し、`syn.control.targets`・
`ControlsUi::parse_target`・`SetParameterBase` が全て同じ tagged
id で解決する。

### D3. 生成点は `Project::new`（`RoutingModel::bootstrap` と同型）

`ControlModel::bootstrap(master_strip: StripId)` を新設し、
`Project::new` が `RoutingModel::default()` 構築後に
`routing.buses[&routing.master].strip` を渡す。理由:

- gesture commit は `gesture_begin` が target の存在を要求し、
  `SetParameterBase` も `targets.get_mut` 前提 — lazily-create
  （mute/solo の `set_strip_bool` 型）では gesture が target 不在で
  `NotFound` になり経路が成立しない。存在は project 生成時に
  保証する必要がある。
- 「合法 project が必ず持つ最小構成」という routing bootstrap と
  同じ責務であり、同じ場所（`Project::new`）が正しい境界。
- `Project::new` 以外の構築経路（snapshot load）は deserialize
  された control model をそのまま使う — ただし load 時の
  `validate_references` が well-known fader の存在を要求するため、
  target を持たない v7 snapshot は migration なしでは load 不能に
  なる。そこで schema version を v8 に上げ、v7→v8 step
  （`migrate_v7_bootstrap_fader`）が canonical target を補う
  （D7）。

### D4. target の属性

- `owner: ControlOwner::Strip(master_strip)` — bus の DSP 権威は
  strip（`ControlOwner` に Bus variant が無いのは意図的）。
- `role: ParameterRole::Fader` — strip fader の singleton role。
  同一 strip への 2 個目の Fader target は `validate_control_binding`
  が拒否する。
- `kind: ValueKind::Continuous`、`mapping: ParamMapping::Linear`、
  `capability: None`（plugin write contract を持たない pure
  internal target）。
- `time_scope: AutomationTimeScope::ProjectMusical`。
- `name: "Fader"`（`ops::role_name(ParameterRole::Fader)` と一致）。
- `base: 1.0` — normalized domain での unity/full gain を既定と
  する（plain=normalized の線形 [0,1]、本 slice では dB カーブを
  持たない）。

### D5. controlled display は `display_value = {"normalized": host_normalized}`

`push_subscription_rows` の param row に `display_value` を付与する。
`observed.value` は evaluation の多キー object のまま残し（他の
読み手の契約）、view wire の `value` には adapter が normalized
単一キー形を選ぶ。normalized を選ぶ理由: gesture 書き込みが
`value.normalized` を domain に渡すのと対称で、capability の
plain domain（将来 dB 等）に依らず normalized [0,1] が全 param
target 共通の contract domain だから。Fader descriptor
`ranged(0,1)` では plain=normalized で一致する。

### D6. blast radius とその処理

- `control.targets` は `BTreeMap<ParameterTargetId,_>` — well-known
  ID は最大値側に並ぶため、「追加した target を `.keys().last()`/
  `.next_back()` で拾う」test helper が bootstrap target を誤認する。
  該当 helper（`aut_02.rs`/`ctl_01.rs`/`sf_02.rs`/`sv47_controls.rs`）
  は `is_issuable_raw` で filter してから末尾を取る形に修正する。
  `.keys().next()`（最小値）は issued ID が常に well-known 未満なので
  影響なし。
- `syn.control.targets` listing は param target を自動的に含む
  （`>= 2` assert の既存 test は影響なし）。
- `EvalPlan`/inspector/snapshot metadata は bootstrap target を
  通常の target として数える — 実 entity なので正しい。
- `syn.control.targets` listing は `TARGET_CAP`(256) で打ち切るが、
  well-known ID は id 順で常に末尾に並ぶため cap が構造 target を
  先に落とす。issued target のみを cap し、well-known（非
  `is_issuable_raw`）は常に列挙する。

### D7. schema v8 migration と構造 target の保護境界

load oracle（`validate_references`）が well-known fader の存在を
要求する以上、v7 snapshot は migration なしで load できない —
「旧 bundle は正直に失敗する」degrade は、合法だった v7 project を
読めなくする regression であり採用しない。v7→v8 step
（`migrate_v7_bootstrap_fader`）は:

- master bus の strip を引き、canonical target（`master_fader_target`、
  `ControlModel::bootstrap` と同一 constructor）を挿入する。
  `id_watermark` は触らない（reserved range の ID は発行域外）。
- v7 body が既に master strip の *issued* `Fader` target を持つ
  場合（v7 では singleton rule 上合法だった）、それは project の
  mixer gain そのものなので well-known ID へ **re-key** する —
  `targets` の key/`id` に加え、`lanes[].target`・
  `assignments[].target`/`depth_source`・`sends[].gain_target` の
  参照を全て張り替え、base・kind・name 等のユーザー値を保持する。
  2 個目の `Fader` を挿入すると singleton rule で load 不能になる
  ため。
- well-known ID が既に存在する body は冪等にそのまま返す
  （load oracle が形状を検査する）。

構造 target の保護は「除去できない」だけでは不十分 — send gain
adoption（`AddSend`/`SetSendGainTarget`）は apply で target の
owner/role を `Send`/`SendLevel` に書き換えるため、well-known
fader を adopt すると master strip が fader を失い、send teardown
（`remove_send_owned`）が target を削除して oracle が project を
reject — send が永久に除去不能な wedge になる。よって両 op の
validate で `!is_issuable_raw` を `RemoveParameterTarget` と同じ
guard で拒否する。oracle 側も well-known ID の存在だけでなく
`role == Fader`・`owner == Strip(master)` を要求し、ID に擬態した
foreign entry（crafted snapshot）を拒否する。cosmetic field
（name/kind/mapping/base）は re-key されたユーザー fader を通すため
拘束しない。

## 代替案と却下理由

- **`ControlOwner::Project` + `role: Generic`**: owner が strip で
  ないと inspector/mixer の owner 別集約から漏れ、Fader singleton
  意味論にも合わない。却下。
- **app 層での ensure（open 後に `AddParameterTarget`）**: load に
  対する暗黙 write で revision/dirty/undo を汚す。境界違反。却下。
- **schema v8 migration で旧 snapshot にも注入**: 当初は blast
  radius を理由に却下したが、load oracle が target の存在を要求
  する設計に変えた時点で「migration なしの degrade」は合法 v7
  project を読めなくする regression になるため採用した（D7）。
- **wire target に `mixer.gain` alias を残す**: `parse_target` が
  tagged id しか解さないため alias 層が必要になり第二の対応表を
  生む。tagged id 直結が最小で一貫。却下。
