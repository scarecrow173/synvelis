# sv572 verification — `port_capability_generation` への plugin identity 混入

対象: `fix/572-port-capability-generation`（base `de2c2424` = origin/main）
OS: Windows（worktree `G:\orca\workspaces\synvelis\sv572-port-cap-gen`）
Toolchain: mise 管理（`mise.toml`/`mise.lock` 正本、Rust CI pin `1.99.0`）
検証日: 2026-11-24

## 実行した検証

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | **Passed** — exit 0（`cargo fmt --all` 適用後に再実行で緑） |
| `mise run check synvelis-project` | **Passed** — exit 0（`cargo check --locked --all-targets -p`） |
| `mise run lint synvelis-project` | **Passed** — exit 0（clippy `-D warnings`） |
| `mise run test synvelis-project` | **Passed** — exit 0。全 test binary ok。新規 pin 5 件（`same_revision_different_descriptor_rebind_moves_generation`・`same_revision_different_version_rebind_moves_generation`・`unresolved_descriptor_rebind_moves_generation`・`identical_rebind_keeps_generation`・`intent_authored_before_same_revision_rebind_rejects_stale`）と `sv571_rebind_port_binding`（parity pin を含む 4 件）・`sf_02`（`a_stale_port_capability_generation_rejects` 等の staleness pin）・`sf06_oracle` 等の既存 generation/staleness pin の非回帰を確認 |
| `mise run test-rt` | **Passed** — exit 0（`cargo test --workspace --locked rt_`） |
| `mise exec -- prek run --all-files` | **Passed** — 全 hook Passed（trim trailing whitespace・fix end of files・check yaml/toml・mixed line ending・utf-8 BOM・detect private key・actionlint。illegal windows names は no files で Skipped） |

## 補助証拠（canonical = mise、消費側の回帰確認）

`port_capability_generation` の意味変更は呼出側が全て実行時に再計算
するため破損は想定しにくいが、消費 crate の test lane を追加実測:

| コマンド | 結果 |
| --- | --- |
| `mise run test synvelis-player` | **Passed** — exit 0。`routing_control.rs`/`routing_engine.rs` の publish/reconcile gate、pin/binding の `capability_generation` 比較は全て同一 project の二重計算で非回帰 |
| `mise run test synvelis-workspace` | **Passed** — exit 0。`routing_gesture` fixation の drift pin（`routing_gesture_capability_generation_drift_drops_stale` — rev 1→2 で依然 generation が動く）を含む非回帰 |
| `cargo test -p synvelis-project --test sv572_port_capability_generation --locked`（修正前 `routing_tx.rs` に対して実行） | **Failed（意図どおり）** — 1 passed / 4 failed。same-revision rebind で generation 不変・stale intent 受理の red 再現。詳細は `reproduction.md` |

## pin が固定する契約

- `same_revision_different_descriptor_rebind_moves_generation`:
  `Resolved(A, rev:r)` → `Resolved(B, rev:r)`（同一 revision・別
  descriptor）で generation が変わる — Issue AC の主ケース。
- `same_revision_different_version_rebind_moves_generation`:
  同一 descriptor・`version` のみ差・同一 revision でも generation
  が変わる。`version` は port stamp に伝播しないため、processor
  binding 側の identity 混入でしか検出できない residual case。
- `unresolved_descriptor_rebind_moves_generation`:
  `Unresolved` の last-known `descriptor`（と port の
  `Unresolved.plugin`）も authored view の一部 — 変更で
  generation が変わる（`Unresolved` を fingerprint に含める
  設計判断を pin。詳細は下記「修正の形」）。
- `identical_rebind_keeps_generation`:
  同一 `PluginRef` への rebind（persisted state 不変）では
  generation 不変 — generation は history ではなく state の
  純粋関数。
- `intent_authored_before_same_revision_rebind_rejects_stale`:
  rebind 前 generation を持つ intent が same-revision rebind 後に
  `RoutingEditError::StalePortCapability` で reject され
  （preflight 経路）、commit path も `routing-stale-capability`
  に畳まれる。rebind 前に同 intent が staging できる sanity を併記。

## 修正の形（`routing_tx.rs`）

`port_capability_generation` を revision 和から、plugin identity を
含む決定論的 fingerprint へ変更。`routing_fingerprint`
（`routing_build.rs` — lease 外のため同方式をローカル実装）と同じ
FNV-1a 64（offset basis `0xcbf2_9ce4_8422_2325`・prime
`0x0000_0100_0000_01b3`）を、`BTreeMap` key 順の決定論的
iteration で processor binding・port binding の identity 項目に
適用する。混入対象:

- `ProcessorBinding::Resolved` — `descriptor`・`version`・
  `capability_revision`（tag `0x01`）
- `ProcessorBinding::Unresolved` — last-known `descriptor`
  （tag `0x02`）
- `PortBinding::PluginDeclared` — `plugin`・`advertised`・
  `capability_revision`（tag `0x11`）
- `PortBinding::Unresolved` — last-known `plugin`・`advertised`
  （tag `0x12`）
- `PortBinding::Internal` — plugin stamp を持たないため非混入

文字列は u64 LE length prefix 付きで feed し、隣接 field の
曖昧性（"ab"+"c" と "a"+"bc"）を排除。variant tag で型由来の
衝突を防ぐ。plugin binding を持たない project は従来どおり
generation 0。

設計判断（doc comment に記録）:

- `Unresolved` の last-known identity も混入対象とした —
  `Unresolved{A}`→`Unresolved{B}` は同一クラスの identity 変更
  であり、port の `Unresolved.plugin` stamp も更新されるため、
  authored view の一部として扱うのが "any reconfigure or rebind
  changes it" の文意に整合する。
- generation は永続化されず常に project 状態から再計算される
  （`RoutingEditIntent` は generation 値のみを保持）。FNV-1a は
  process・Rust version 間で安定し、save→load を跨いでも同一
  project に同一値を返す。衝突は原理上可能だが、既存
  `routing_fingerprint` と同じく change detector として許容し、
  不一致は `StalePortCapability` への fail-safe 方向にのみ働く。

## `synvelis#571` との関係評価

`synvelis#571`（ADJ-1）は PR #615（`db0ef99a`）で修正済み —
`SetProcessorBinding` apply arm が port binding を
`processor_port_binding` で再導出するため、rebind 後の port
stamp は正しく、generation の**入力**は正しい。本 Issue の対象は
residual gap のみ: fingerprint 関数が `capability_revision` しか
見ないため、同一 revision の別 plugin rebind（および version
差・Unresolved descriptor 差）を検出できない。本修正で identity
混入により residual gap を解消した。`sv571` の parity pin
（rebind path と insert-fresh path で同一最終状態→同一
generation）は generation が純粋関数である限り成立し、実測で
非回帰を確認した。

## 未検証の項目

| 項目 | 区分 | 理由・follow-up |
| --- | --- | --- |
| GUI-L2（実ウィンドウ操作） | NotRun | POLICIES.md §5 により日常必須検証に含めない。headless domain のみの変更で UI 経路に触れない |
| EXT-L3（外部プラグイン実機） | NotRun | 同上。`PluginRef` は fixture のみで実 plugin runtime を介さない |
| MSRV（rust-version 1.89） | NotRun | CI toolchain pin は 1.99.0。edition 2024 構文の新規使用なし。必要なら CI で確認 |
| 3OS 横断（Linux/macOS） | NotRun | POLICIES.md §5 により日常必須検証に含めない。OS 依存コードに触れない |
| Full CI | NotRun | 日常検証は crate 単位の check/lint/test + test-rt + prek。Full CI は Coordinator 統合時 |
| docs index 再生成 | Coordinator 作業 | `docs/**/README.md` は生成物で worker は編集・再生成しない。統合時に Coordinator が整備 |
| spec doc 同期 | 範囲外（未対応） | `docs/specs/sf-03-slice9-capability-firing.md`・`sf-02-routing-transaction-commands-semantic-validation.md` 等が generation を「revision 値の合算」と記述 — identity 混入で記述が stale。lease 外のため非接触、PR 本文の未対応作業に記載 |

## 隣接論点（本 slice 範囲外）

- fingerprint が identity まで見るようになったため、
  `port_capability_generation` は「capability 変更」より広い
  「plugin/port binding identity 変更」の検出器となった。doc
  comment の契約記述を実装に合わせて更新済み（"Any reconfigure
  or rebind changes it" が identity 差を含めて成立）。
