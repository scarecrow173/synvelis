# SV266-NR-02/03 — theme status tokens + #274 parity check（検証記録）

作成: 2026-11-15 / branch: `scarecrow173/sv266-theme-parity`
対象: `assets/themes/synvelis-{dark,light}.dtcg.json`・
`crates/synvelis-theme/tests/theme_contract.rs`・
`crates/synvelis-workspace/tests/sv266_parity.rs`（新規）

語彙は POLICIES.md §2/§3 に従う。**headless Pass は実機 Pass ではない**。
`evidence/sv266/README.md` は前 slice（AUTX-07 headless GUI views）の
Case 台帳であり本ファイルはその続編 — NR-02/03 の充足結果のみを記録し、
既存台帳は改変しない。

検証環境: Windows 11 (x86_64)、rustc/cargo 1.99.1（`mise.toml`/
`mise.lock` 管理）、worktree `G:\orca\workspaces\synvelis\sv266-theme-parity`。
canonical 検証は `mise run` レーン、`cargo` 直接実行は補助証拠。

## 1. SV266-NR-03 — `color.status-*` token 値（充足: Pass）

### 変更

- `assets/themes/synvelis-dark.dtcg.json`・`synvelis-light.dtcg.json`
  の `color` group（`"$type": "color"`）に `StatusTone::ALL` 全
  **17** path の `status-*` `$value` を追加（`StatusTone::ALL` 順）。
  base/density 文書には `color` group が存在しないため変更なし
  （文書層規約: mode 文書のみが color を所有）。envelope・
  `$extensions`・`id`・`modes`・`"layer": "product"` は不変、
  `synvelis.provenance` に status ramp が provisional である旨を追記。
- `crates/synvelis-theme/tests/theme_contract.rs` に
  `shipped_assets_resolve_every_status_tone_for_every_mode_density_pair`
  を追加 — `StatusTone::ALL` 走査（件数 literal assert 無し）で
  `color.{token_path()}` を dark/light × compact/comfortable の
  4 組合せ全てで resolve し、density ペア内の値一致（density は
  status ramp を再着色しない）と mode 間 ramp 差異を assert。

### 17 token resolve 行列（Observed — 全 4 組合せで `Some`）

dark / compact = dark / comfortable、light / compact =
light / comfortable（density は status 値を変えない — test が assert）。

| token path | dark | light |
| --- | --- | --- |
| `color.status-ok` | `#6ac88a` | `#2a8a4a` |
| `color.status-unavailable` | `#767e88` | `#6e7680` |
| `color.status-unknown` | `#9aa0a8` | `#5a6068` |
| `color.status-pending` | `#8fb8e8` | `#5b84b8` |
| `color.status-running` | `#6aa9e8` | `#2a6bc2` |
| `color.status-completed` | `#4ec9a4` | `#1f7a5c` |
| `color.status-failed` | `#e86a6a` | `#c23a3a` |
| `color.status-cancel-requested` | `#f0a868` | `#b06a20` |
| `color.status-cancel-unconfirmed` | `#e0aa4e` | `#9a6f1e` |
| `color.status-cancelled` | `#c9725f` | `#a8543c` |
| `color.status-indeterminate` | `#d9b44a` | `#8a6d1f` |
| `color.status-unsupported` | `#b48ae0` | `#7a4fa8` |
| `color.status-stale` | `#8f987c` | `#6e7346` |
| `color.status-dirty` | `#a08f68` | `#7a6a48` |
| `color.status-diagnostic-information` | `#7aa8d8` | `#3d6ea8` |
| `color.status-diagnostic-warning` | `#f0a868` | `#b06a20` |
| `color.status-diagnostic-error` | `#e86a6a` | `#c23a3a` |

注: `status-diagnostic-warning`/`status-diagnostic-error` は意味的に
`warn`/`err` 系と同 hue を意図的に共有する（diagnostic severity と
status tone の写像 — `status.rs` の `of_label` が diagnostic label を
対応 tone に落とす設計と整合）。`cancel-unconfirmed`/`indeterminate`
は warn/err とは別 hue で「警告でもエラーでもない」意味を色相分離。
色値は provisional — native design review で調整され得る
（token path vocabulary が stable contract）。

### 結果

`mise run test synvelis-theme`: **15 passed / 0 failed**（exit 0）。
新規 test を含む全 `shipped_assets_*` が Pass。NR-03 の
「`status-*` token 値追加 + `shipped_assets_*` test 拡張」は充足。

## 2. SV266-NR-02 — Action Palette ⇔ Script Editor parity（部分充足: Pass）

### 構成

`crates/synvelis-workspace/tests/sv266_parity.rs`（新規、6 test）。
`crates/synvelis-script/tests/fixtures/registry_snapshot.json` を
`RegistrySnapshot` に復元し、`ScriptApiModel::build(&snapshot,
&surface)`（script 側 source）と `snapshot.capabilities` を
`resolve_capability` の入力（palette 側 source）に共有させる。
surface は 12 entry: available×6・planned・disabled・retired・
version drift（`add-note@9.9`）・未登録 id・hidden。

### parity 照合 matrix（Observed — `cargo test -p synvelis-workspace
--locked --test sv266_parity`: 6 passed / 0 failed、exit 0）

| 照合項目 | 手法 | 結果 |
| --- | --- | --- |
| capability identity | 全 public `ScriptApiRow.capability` を `resolve_capability(&snap.capabilities, ..)` に通し、`entry.descriptor.is_some()` ⟺ `Ok` の同値性 + descriptor 完全一致 + `capability_label()` 一致を assert | **Pass** — 11 public row 全件で両側が同一 descriptor source に解決 |
| fail-closed（version drift / 未登録 id） | `wrongVer`（`add-note@9.9`）→ verdict `unsupported-version`（detail=`"1.0"` supported 開示）・palette `UnknownCapability`。`ghostOp` → `unknown-capability` / `UnknownCapability` | **Pass** — 両側 fail-closed で一致、実行可能表示を生まない |
| availability | verdict label ↔ `Availability::name()` 写像（`invocable`⇔`available` 等）+ `ActionRow.availability` verbatim | **Pass** — disabled/planned/retired が同名 state で一致 |
| executability 行列 | 全 descriptor × grant {Satisfied, Missing, Unknown} で `executable == (Available ∧ satisfied)`、`blocked.is_none() == executable` を assert（SV266-AC-01 再確認） | **Pass** — fail-closed 行列全セル一致 |
| schema | `SchemaBinding.schema`（input/output）== descriptor の `SchemaRef`、`ActionRow::schema_label` == `SchemaRef::to_string()` | **Pass** — `ScriptApiRow` は schema を直接保持しないため descriptor 経由で照合（public API 到達可能範囲） |
| target/domain/effects | `ActionRow.target`/`domain`/`effects` が descriptor の verbatim 投影 | **Pass** — 投影は権威値を改変しない |
| analyzer ≠ executability | `ScriptEditorDocument` を Idle→Ready→Dirty→Stale→Unavailable→Unsupported→Rejected と遷移させ `doc.api()` 不変 + palette `executable`/`blocked` 不変を assert。`invocable` verdict と grant Missing `executable=false` の共存を明示 | **Pass** — analyzer PASS は Gateway 認可 PASS を意味しないことを headless で固定 |
| hidden entry | `ScriptVisibility::Hidden` が public rows に現れない | **Pass** — `hiddenOp` は model 内監査可能だが投影されない |

src 側契約 bug の露呈: **無し**（`automation_ui.rs`/`script_editor.rs`/
`synvelis-script` の public API のみで全 invariant が Pass。
src 変更は sv#112 lease のため実施せず・必要も無し）。

### 結果

NR-02 の「#274 実照合」は **契約レベルで部分充足** — 両 surface が
同一 `RegistrySnapshot`/`CapabilityDescriptor` source から同じ
capability/schema/availability verdict を示すことを headless で固定。
Script Editor surface 本体の実装・実 UI での文言照合は #274 Owner
領域のため残件（§4）。

## 3. 実行した検証（canonical = `mise run`、全て exit 0）

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows 11 x86_64 | rustc 1.99.1 | 0 | Passed |
| `mise run check synvelis-theme` | 同上 | 同上 | 0 | Passed |
| `mise run lint synvelis-theme` | 同上 | 同上 | 0 | Passed（warning 0） |
| `mise run test synvelis-theme` | 同上 | 同上 | 0 | Passed — 15 tests |
| `mise run check synvelis-workspace` | 同上 | 同上 | 0 | Passed |
| `mise run lint synvelis-workspace` | 同上 | 同上 | 0 | Passed（warning 0） |
| `mise run test synvelis-workspace` | 同上 | 同上 | 0 | Passed — 全 binary ok（`sv266_parity` 6 tests 含む） |
| `mise run test-rt` | 同上 | 同上 | 0 | Passed（workspace `rt_` filter） |
| `mise exec -- prek run --all-files` | 同上 | 同上 | 0 | Passed |
| `cargo test -p synvelis-workspace --locked --test sv266_parity` | 同上 | 同上 | 0 | Passed — 6 tests（**補助証拠**） |

`*-msrv`・`*-full` 系は Daily Full CI 委譲のため未実行（NotRun、
タスク指定どおり必須検証に非含有）。

## 4. NotRun / Deferred（正直記録）

| Case | 内容 | 状態 | 後続 |
| --- | --- | --- | --- |
| SV266-NR-01 | 実 GPUI Windows/macOS/Linux Pointer/Keyboard/IME/Focus/DPI/2Window/WindowClose・Alert・Undo/Save/Reload・Window 断継続 Job | **NotRun** | synvelis#114/#267 別 Gate。headless Pass を実機 Pass に昇格しない |
| SV266-NR-02 残件 | Script Editor surface 本体での実 UI 文言・実表示照合 | **Deferred** | synvelis#274 Owner。契約レベル parity は本 slice で固定済み |
| SV266-NR-04 | `synvelis-automation` live adapter 接続（#264） | **NotRun** | 別 slice・別リース |
| SV266-NR-05 | composition mount 配線（serial-after-sv112） | **NotRun** | `crates/synvelis-app` — 本 slice 範囲外 |
| SV266-NR-06 | `gpui-test` GPUI Semantic scenario | **NotRun** | 任意 lane・別 slice |
| `status-*` 色値の確定 | provisional 値の native design review | **Deferred** | design review lane。token path は stable |

## 5. 補足

- parity test は public API のみを使用 — `ScriptApiRow` が schema を
  直接保持しないため schema 照合は「entry.descriptor（script 側が
  resolve した descriptor）↔ `resolve_capability` 解決先 ↔
  `ActionRow` 投影」の三点一致として記録（契約到達可能範囲）。
- `entry.descriptor.is_some()` ⟺ `resolve_capability` Ok の同値性は
  exact `(id, version)` 解決の両実装が一致することの帰結 — version
  表示の `major.minor` 往復に依存しない文字列比較ではなく型レベルの
  descriptor 一致を assert している。
