# sv571 reproduction — SetProcessorBinding rebind の SemanticPort.binding stale

対象: `scarecrow173/synvelis` issue `#571`（`Closes #571` で close）
ブランチ: `bugfix/571-processor-binding-stale`（base `d68f0e54` = origin/main）
再現日: 2026-11-22
OS: Windows（worktree `G:\orca\workspaces\synvelis\sv571-processor-binding-stale`）

## 再現方法

`crates/synvelis-project/tests/sv571_rebind_port_binding.rs`
の pin 4 件を、**修正前の `ops.rs`（`SetProcessorBinding` が
port を再 stamp しないコード）**に対して実行した — 修正を
`git stash` で退避し、base `d68f0e54` の production コード +
新規 pin test の構成で実測（cargo 直接実行は補助証拠、
canonical は mise — 検証 lane 記録は `verification.md`）。

```text
cargo test -p synvelis-project --test sv571_rebind_port_binding --locked
→ test result: FAILED. 0 passed; 4 failed
```

## 失敗内容（全 4 件 — 欠陥の各面を個別に固定）

### rebind_to_unresolved_marks_every_port_unresolved

`Resolved(vendor.a, rev=3)` → `Unresolved{vendor.gone}` rebind
後も port は plugin A の `PluginDeclared` のまま:

```text
left:  [PluginDeclared { plugin: "vendor.a", advertised: "in",  capability_revision: 3 },
        PluginDeclared { plugin: "vendor.a", advertised: "out", capability_revision: 3 },
        PluginDeclared { plugin: "vendor.a", advertised: "sidechain", capability_revision: 3 }]
right: [Unresolved { plugin: "vendor.gone", advertised: "in" },
        Unresolved { plugin: "vendor.gone", advertised: "out" },
        Unresolved { plugin: "vendor.gone", advertised: "sidechain" }]
```

### rebind_to_unresolved_rejects_endpoints_like_insert_path

stale stamp が capability fingerprint に混入 —
`port_capability_generation` が rebind path `9` / insert path
`0` で発散（同一論理状態で同一 fingerprint になるはず）:

```text
assertion `left == right` failed
  left: 9
 right: 0
```

### rebind_resolved_to_resolved_restamps_every_port

`Resolved(vendor.a, rev=3)` → `Resolved(vendor.b, rev=4)`
rebind 後も port stamp は plugin A のまま:

```text
left:  [PluginDeclared { plugin: "vendor.a", ..., capability_revision: 3 }, ×3 slots]
right: [PluginDeclared { plugin: "vendor.b", ..., capability_revision: 4 }, ×3 slots]
```

### rebind_unresolved_to_resolved_stamps_and_reopens_endpoints

`Unresolved{vendor.gone}` → `Resolved(vendor.b, rev=4)` rebind
後も port は `Unresolved` のまま — 回復方向でも発散:

```text
left:  [Unresolved { plugin: "vendor.gone", advertised: "in" }, ×3 slots]
right: [PluginDeclared { plugin: "vendor.b", advertised: "in", capability_revision: 4 }, ×3 slots]
```

## 根因（修正前コードの確認）

`ops.rs` の `SetProcessorBinding` apply arm は `proc.binding`
差替と `plugin_instances.sync_processor` のみを実行し、
`proc.port_ids` 経由の `routing.ports` を一切更新しない。
port stamp は `InsertProcessor` 時に `processor_port_binding`
で一度だけ導出され persisted されるため、rebind 後も旧
binding 由来の stamp が残る。`routing_tx.rs` の
`resolved_endpoints` は `PortBinding::Unresolved` のみを
`UnresolvedPort` で拒否する設計のため、stale `PluginDeclared`
port はこの fail-closed を素通りする — `Resolved→Unresolved`
rebind の `UnresolvedPort` 拒否迂回。

## 補足: `vcs06` fixture の乖離

`vcs06_plugin_instance_matrix.rs` の rebind fixture は
`capability_revision` のみ手動 stamp して「the shape a real
rebind op would produce」と自称していたが、上記のとおり実 op
は port stamp を一切更新しない — fixture と実 op が乖離。
本 slice で fixture を実 `SetProcessorBinding` op 経由に修正
し、乖離を解消した。
