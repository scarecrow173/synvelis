---
id: "EXP-svscript01scriptapimodel001"
type: "experiment"
status: "completed"
title: "SV-SCRIPT-01 ScriptApiModel・digest pin 二重生成・wire lower の試行"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/271"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/sv-script-01-script-api-model.md"
  depends-on:
    - "docs/automation/design-script.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv-script-01-script-api-model.md"
    - "evidence/sv271/sv-script-01-script-api-model-ledger.md"
  refines: []
  refined-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/automation/design-script.md"
    - "crates/synvelis-gateway/src/capability.rs"
    - "crates/synvelis-gateway/src/registry.rs"
    - "crates/synvelis-gateway/src/schema.rs"
    - "crates/synvelis-gateway/src/request.rs"
    - "crates/synvelis-gateway/src/wire.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-SCRIPT-01 ScriptApiModel・digest pin 二重生成・wire lower の試行

## 問題設定

Issue #271（SV-SCRIPT-01）の headless slice は、Gateway の
`CapabilityRegistry`/`SchemaRegistry` を read-only 正本として
versioned `RegistrySnapshot` を capture し、script 名→exact
`CapabilityRef` の `ScriptSurfaceSpec` と合わせて一つの決定的
`ScriptApiModel` を build し、同一 model digest から Runtime
Binding（Gateway wire `OperationRequest` 相当の lower 形）と
Luau 型定義を同時生成することが契約
（`docs/automation/design-script.md`）。設計上の未確定点は
4 つあった:

1. snapshot の構成要素 — `CapabilityRegistry::resolve` は
   `pub(crate)` で公開面に無く、handler へ到達する経路は
   使えない。discovery metadata のみで何を凍結するか。
2. i64 の Luau 投影 — Luau `number` は f64 で i64 全域を
   表現できない。brand 型・文字列・number のどれに落とすか。
3. `InvocationDraft` の wire 適合 — gateway の
   `WireRequest`/`WireTarget`/`WireConsistency` を直接再利用
   するか、serialize 互換の独自 wire 形を持つか。
4. Analyzer port の形状 — ruviform/Luau.Analysis 依存を
   この crate に入れず、RF-SCRIPT-00 契約に合わせた薄い
   consumer adapter をどう定義するか。

## 試行と結果

### E1: snapshot 入力 — resolve 経路 vs discovery metadata 凍結

`CapabilityRegistry::list`/`describe`/`versions` と
`SchemaRegistry::resolve`/`versions` の公開 read API のみで
`RegistrySnapshot::capture` を構成した。descriptor は
`Clone` で verbatim 凍結し、input/output `SchemaRef` は
`SchemaRegistry::resolve` で mirror する。dangling schema
参照は capture 時に落とさず model 側で `UnresolvedSchema`
verdict に写像する（snapshot 全体を fail させると 1 件の
schema 欠落で surface 全体が死ぬため）。digest は canonical
JSON（object key 再帰 sort）+ SHA-256 lowercase hex。
代替案（registry への live 参照保持）は versioned snapshot
契約を壊すため棄却。

### E2: i64 投影 — `SynvelisI64` 10 進文字列

Luau `number`（f64）で表現不能な `Integer`（min/max が f64
正確整数範囲外、または無制限）は opaque brand 型でも
number でもなく **10 進文字列 `SynvelisI64`** に落とした。
`assemble_call` は schema が `Integer` の位置にある文字列
のみを strict decimal-i64 decode し、`TimeRange` の
`start`/`len` も同経路で decode する。f64 正確に収まる
`Integer` は `number` のまま。代替案（全 Integer を文字列化）
は stub の可読性を損ない、範囲内整数の自然な Luau 表現を
失うため棄却。

### E3: InvocationDraft — wire 型再利用 vs serialize 互換形

`InvocationDraft` は gateway の `WireRequest` 構成要素
（`CapabilityRef`/`SchemaRef`/`Consistency`/`SnapshotRef`/
`RequestId`/`ProjectRevision`）をそのまま field に持ち、
target/consistency は wire JSON shape に serialize する。
`verify_wire()` が `WireRequest::parse` へ通して authoritative
境界を確認する。`task`/`grant`/`idempotency_key` は opaque
`serde_json::Value` で保持し、`CallerIdentity`/`Grant`/
`Approval`/`Principal` の trusted 型は field に存在しない
（型で表現不能）。代替案（`WireRequest` 自体を返す）は
script 側が wire の全責任を負い、task/grant の opaque 受け渡し
境界が曖昧になるため棄却。

### E4: Analyzer port — trait のみ・依存ゼロ

`ScriptAnalyzer` trait + `AnalyzeRequest`/`AnalyzeResult`/
`TypeDefinition`/`Diagnostic`/`CompletionItem` の consumer
側型を定義し、`DefinitionInjection` で生成 stub の
model/stub digest を pin して注入する。`verify_result_revision`
で source/stub revision ずれを stale として拒否する。
ruviform・Luau.Analysis への依存は追加していない（実測は
ruviform#560 の領域として Deferred）。fake analyzer で未知
API・誤 arity・completion・stale の契約試験を行い、
「解析成功 ≠ 実行権限」を fixture で固定した。

## 採用結果

E1〜E4 すべて採用。`ScriptApiModel`/`BindingSet`/`LuauStub`/
`GeneratedArtifacts` の digest 連鎖と、typed fail-closed
verdict・wire-shaped draft・analyzer port が実装され、
56 test（analyzer 7・binding 13・golden 10・luau 9・
model 17）が green。

棄却した代替: registry live 参照（E1）・全 Integer 文字列化
（E2）・`WireRequest` 直接返却（E3）・ruviform 直接依存の
analyzer（E4）。
