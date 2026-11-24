# sv572 reproduction — 同一 `capability_revision` rebind が `port_capability_generation` を動かさない

対象: `scarecrow173/synvelis` issue `#572`（`Closes #572` で close）
ブランチ: `fix/572-port-capability-generation`（base `de2c2424` = origin/main）
再現日: 2026-11-24
OS: Windows（worktree `G:\orca\workspaces\synvelis\sv572-port-cap-gen`）

## 再現方法

`crates/synvelis-project/tests/sv572_port_capability_generation.rs`
の pin 5 件を、**修正前の `routing_tx.rs`（`port_capability_generation`
が `capability_revision` のみを合算するコード）**に対して実行した
— pin test を先に書き、production コード未変更の状態で実測
（cargo 直接実行は補助証拠、canonical は mise — 検証 lane 記録は
`verification.md`）。

```text
cargo test -p synvelis-project --test sv572_port_capability_generation --locked
→ test result: FAILED. 1 passed; 4 failed
```

## 失敗内容（4 件 — 欠陥の各面を個別に固定）

### same_revision_different_descriptor_rebind_moves_generation

`Resolved(vendor.a, 1.0.0, rev=3)` → `Resolved(vendor.b, 1.0.0, rev=3)`
rebind 後も generation は `12` のまま（processor rev 3 + port rev
3×3 slots = 12。`synvelis#571` の port 再 stamp は反映されるが、
revision の和は同一値）:

```text
assertion `left != right` failed
  left: 12
 right: 12
```

### same_revision_different_version_rebind_moves_generation

`Resolved(vendor.a, 1.0.0, rev=3)` → `Resolved(vendor.a, 2.0.0, rev=3)`
— `version` は port stamp に伝播しないため、port 側の identity を
見ても検出できない residual case。generation は `12` のまま:

```text
assertion `left != right` failed
  left: 12
 right: 12
```

### unresolved_descriptor_rebind_moves_generation

`Unresolved{vendor.a}` → `Unresolved{vendor.b}` rebind — last-known
descriptor と port の `Unresolved.plugin` stamp は更新されるが、
revision-only の fingerprint は両方とも 0 を返す:

```text
assertion `left != right` failed
  left: 0
 right: 0
```

### intent_authored_before_same_revision_rebind_rejects_stale

`Resolved(A, rev=3)` の view で authored された intent（
`expected_port_capability_generation` = rebind 前の値）が、
same-revision `Resolved(B)` への rebind 後も `StalePortCapability`
を経ずに受理される — generation が `12` で不変のため
preflight の staleness 検査を通過する（Issue 本文の acceptance gap
そのもの）:

```text
assertion `left != right` failed
  left: 12
 right: 12
```

## 通過した pin（対照）

`identical_rebind_keeps_generation` — 同一 `PluginRef` への rebind
（project 状態が同一）は修正前後とも generation 不変。generation
が「history ではなく persisted state の純粋関数」である契約は
修正前から成立しており、修正後も維持される。

## 根因（修正前コードの確認）

`routing_tx.rs` の `port_capability_generation` は
`ProcessorBinding::Resolved` の `capability_revision` と
`PortBinding::PluginDeclared` の `capability_revision` の saturating
和のみを返し、plugin identity（descriptor・version・port stamp）
を一切見ない。`synvelis#571`（PR #615）で rebind 時の port 再
stamp は修正済みのため generation の**入力**は正しいが、
fingerprint が revision しか見ないため、同一 revision の別
plugin rebind は検出不能なまま残る。`Unresolved` バリアントは
常に 0 を返し、last-known identity の変更も検出できない。
