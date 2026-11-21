# sv571 verification — SetProcessorBinding rebind の SemanticPort.binding 再導出

対象: `bugfix/571-processor-binding-stale`（base `d68f0e54`）
OS: Windows（worktree `G:\orca\workspaces\synvelis\sv571-processor-binding-stale`）
Toolchain: mise 管理（`mise.toml`/`mise.lock` 正本、Rust CI pin `1.99.0`）
検証日: 2026-11-22

## 実行した検証

| コマンド | 結果 |
| --- | --- |
| `mise run test synvelis-project` | **Passed** — exit 0。全 test binary ok。新規 pin 4 件（`rebind_to_unresolved_marks_every_port_unresolved`・`rebind_to_unresolved_rejects_endpoints_like_insert_path`・`rebind_resolved_to_resolved_restamps_every_port`・`rebind_unresolved_to_resolved_stamps_and_reopens_endpoints`）と `vcs06_plugin_instance_matrix` 9 件（fixture を実 op 経由に修正）を含む全既存 pin の非回帰を確認 |
| `mise run check synvelis-project` | **Passed** — exit 0（`cargo check --locked --all-targets -p`） |
| `mise run lint synvelis-project` | **Passed** — exit 0（clippy `-D warnings`） |
| `mise run fmt-check` | **Failed（環境起因）** — `cargo fmt --all` が worktree の深いパス配下で `ファイル名または拡張子が長すぎます。 (os error 206)`。原因は cargo-fmt が全 workspace の .rs（658 件）を `\\?\` 絶対パスで 1 回の rustfmt spawn に積むコマンドライン長上限超過 — ファイル内容非依存・本変更起因ではない（補助証拠は下表） |
| `mise exec -- prek run --all-files` | **Passed** — 全 hook Passed（trim trailing whitespace・fix end of files・check yaml/toml・mixed line ending・utf-8 BOM・detect private key・actionlint） |

## 補助証拠（canonical = mise、以下は cargo 直接実行）

| コマンド | 結果 |
| --- | --- |
| `cargo fmt -p synvelis-project -- --check` | **Passed** — 変更対象 crate の rustfmt 準拠を確認（`cargo fmt -p synvelis-project` で整形適用後） |
| `cargo fmt --all -- --check`（`G:\repositry\audio-products\synvelis` の浅いパスで実行） | **Passed** — 同一コードでも worktree 外なら os error 206 不発。fmt-check 失敗が worktree パス深さ起因の環境制約であることを確認 |
| `cargo test -p synvelis-project --test sv571_rebind_port_binding --test vcs06_plugin_instance_matrix --locked` | **Passed** — 13 passed（新規 4 + vcs06 9） |
| `cargo test -p synvelis-project --test sv571_rebind_port_binding --locked`（修正前 `ops.rs` に stash 退避して実行） | **Failed（意図どおり）** — 0 passed / 4 failed。stale stamp と拒否迂回の red 再現。詳細は `reproduction.md` |

## pin が固定する契約

- `rebind_to_unresolved_marks_every_port_unresolved`:
  `Resolved→Unresolved` rebind で全所有 port が
  `PortBinding::Unresolved` になり insert-fresh shape と一致
  （slot 名・順序を含む）。
- `rebind_to_unresolved_rejects_endpoints_like_insert_path`:
  rebind path と `InsertProcessor`+`Unresolved` path が同一 id・
  同一 `port_capability_generation` を持ち、sidechain wire に
  同一の `UnresolvedPort` 拒否（同一 port 名指し）を返す。
  commit path も `routing-unresolved-port` に畳まれる。
- `rebind_resolved_to_resolved_restamps_every_port`:
  `Resolved(A)→Resolved(B)` で全 port に B の descriptor・
  `capability_revision` が stamp され、`advertised` slot 名が
  維持され、generation が insert-fresh `Resolved(B)` と一致。
- `rebind_unresolved_to_resolved_stamps_and_reopens_endpoints`:
  `Unresolved→Resolved` で `PluginDeclared` が復元され
  sidechain endpoint が再び resolvable。
- `vcs06_plugin_instance_matrix` 9 test: rebind fixture が実
  `SetProcessorBinding` op 経由の after 状態で matrix 非回帰
  — fixture と実 op の乖離が解消。

## 修正の形（`ops.rs`）

`SetProcessorBinding` apply arm が `proc.binding` 差替と同じ
atomic 適用内で `proc.port_ids` の各 port を
`processor_port_binding` で再 stamp する。`advertised` は既存
port の値を維持、`PortBinding::Internal` は plugin stamp 非所持
で対象外。`sync_processor` の PC-C4-05 semantics は非接触。

## 未検証の項目

| 項目 | 区分 | 理由・follow-up |
| --- | --- | --- |
| GUI-L2（実ウィンドウ操作） | NotRun | POLICIES.md §5 により日常必須検証に含めない。headless domain のみの変更で UI 経路に触れない |
| EXT-L3（外部プラグイン実機） | NotRun | 同上。`PluginRef` は fixture のみで実 plugin runtime を介さない |
| MSRV（rust-version 1.89） | NotRun | CI toolchain pin は 1.99.0。edition 2024 構文の新規使用なし（`let-else` 等は既存コードと同水準）。必要なら CI で確認 |
| 3OS 横断（Linux/macOS） | NotRun | POLICIES.md §5 により日常必須検証に含めない。OS 依存コードに触れない |
| `mise run fmt-check` | Failed（環境起因） | 上表のとおり worktree パス深さ + cargo-fmt の単一 spawn 設計による os error 206。変更 crate 個別の fmt check は Pass。統合 CI（浅い checkout パス）では成立見込み |
| Full CI | NotRun | 日常検証は crate 単位の check/lint/test + prek。Full CI は Coordinator 統合時 |
| docs index 再生成 | Coordinator 作業 | `docs/**/README.md` は生成物で worker は編集・再生成しない。統合時に Coordinator が整備 |

## 隣接論点（本 slice 範囲外）

`synvelis#572` — `port_capability_generation` が同一
`capability_revision` の rebind を fingerprint できない
staleness gap。`routing_tx.rs` は本 slice の lease 外で非接触。
本修正で rebind path の generation 入力（port stamp）は正しく
なるが、同 revision rebind の検出漏れは残る。
