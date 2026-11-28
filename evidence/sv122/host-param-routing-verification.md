# CTL-02 Strip/Send Host Parameter・Routing Atomic Commit 検証記録

対象 Issue: `scarecrow173/synvelis#122`（部分対応）  
対象ブランチ: `feat/122-host-param-routing`  
OS: Windows 11（OS-L1、headless/in-process）  
正本評価: 実行した結果だけを Pass とし、未実行・Unsupported・環境起因 Fail を Pass にしない。

## 観測行列

| 面 | layer / endpoint | 観測 | 結果 |
| --- | --- | --- | --- |
| Fader/SendLevel の begin/preview/commit | `syn.control.subscribe` + `syn.control.dispatch`（app headless） | `commit` receipt が `applied` / `base edit`、project `ParameterTarget::base` が Fader=0.625・SendLevel=0.375 になる | Pass（test `strip_and_send_endpoint_authority_survives_history_and_reload`） |
| Fader/SendLevel の Undo/Redo | `syn.project.undo` / `syn.project.redo` | SendLevel→Fader の順に独立して戻り、redo で同じ target/value に復帰する | Pass（Pass（exit 0）） |
| Fader/SendLevel の Save/Reload | `syn.project.save` / `syn.project.reload` + fresh view subscription | 同一 `ParameterTargetId` が残り、reloaded base が initial projection で観測される。`SendConfig.gain_target` は同じ target を指し、第二の persisted gain scalar はない | Pass（同上） |
| Pan/Mute/Solo authority 観測 | `syn.control.targets` + `syn.control.subscribe` | 3 role が `kind=param` で列挙され、owner が対象 strip、projection normalized が persisted base（Pan=0.5、Mute/Solo=0.0） | Pass（test `pan_mute_solo_endpoint_authority_observation`） |
| `syn.control.fast_control` の internal target 非使用 | app tests + player ctl_02 | internal target を fast_control 経路に流用していない。player 側では internal target が `UnboundTarget` で plugin ParamId へ届かない | Pass（既存 player tests） |
| CreateSend atomic commit | production `RoutingEditIntent` / `ProjectOp::Routing` | Send-kind connection・`SendConfig`・`SendLevel` target が 1 CAS/1 Undo で commit される | Pass（既存 test `send_create_commits_gain_target_in_one_undo`） |
| CreateSend 後方失敗の atomicity | production routing intent | 有効な `CreateSend` の後に未知 send の `RemoveSend` を置いた intent が失敗し、live project・revision・undo history・target 数が不変、Send-owned orphan target が残らない | Pass（新規 test `failed_multi_edit_send_create_leaves_no_orphan_target`） |
| Automation lane identity | project `AddAutomationLane` / `WriteAutomationRegion` / `evaluate_control` / Save/Reload | Fader/SendLevel の lane が同じ `ParameterTargetId` を参照し、automation 値が評価され、save/reload 後も同じ target/value/capability None を保つ | Pass（新規 test `strip_and_send_automation_lanes_keep_one_target_identity`） |
| plugin binding generation separation | player routing control | plugin parameter は stage-1/stage-2 binding と generation 一致でだけ Observed/Delivered。 Pending/Stale/Blocked と internal Unbound は既存 tests で分離 | Pass（既存 player ctl_02） |
| GUI-L2 / EXT-L3 / 実 plugin ABI / 2Window / AudioLens | 外部 gate | この slice では実行せず。headless/in-process evidence のみ | NotRun（merge 後 CI・別 slice） |
| Send authoring endpoint | `syn.signal_flow.routing` / workspace / ui_ops | endpoint を新設・拡張していない。Send 作成検証は production routing intent に限定 | Pass（設計制約の遵守） |

## 実行コマンドと結果

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Fail（Windows 環境で workspace-wide rustfmt が `os error 206`。コード違反とは切り分け） |
| `cargo fmt --package synvelis-project --package synvelis-app -- --check`（補助証拠; canonical = mise） | Pass（exit 0） |
| `mise run test synvelis-project --test ctl_02` | Pass（24 passed, exit 0） |
| `mise run test synvelis-app --test pc_c4_06_control_op_surface` | Pass（6 passed, exit 0） |
| `mise run check synvelis-project` / `lint synvelis-project` / `test synvelis-project` | Pass（exit 0） |
| `mise run check synvelis-player` / `lint synvelis-player` / `test synvelis-player --test ctl_02 --test sf_xr_01` | 同上 |
| `mise run check synvelis-app` / `lint synvelis-app` / `test synvelis-app --test binding --test autx07_automation --test pc_c4_06_control_op_surface` | 同上 |

## 判断・残課題

- `crates/synvelis-app/src/automation.rs` は parameter automation lane ではなく AUTX surface adapter であり、今回の root cause ではないため編集しなかった。
- mode 選択 endpoint はこの slice の前提どおり新設せず、app fixture では `Off` mode の session setup だけを直接行い、値確定 gesture は endpoint wire shape で実行した。
- `evidence/sv122/canonical-persistence-verification.md` と `param-history-verification.md` が既に pin していた persisted canonical spec・Fader/SendLevel history を再実装せず、不足していた endpoint 統合・後方失敗 atomicity・automation positive persistence だけを追加した。
- 静的調査で、crafted persisted snapshot が same-send owner だが非 `SendLevel` role の target を `gain_target` に指す場合に project oracle が role を fail-closed 検査していない可能性が残る。該当ファイルは本 lease 外であり、本 PR では修正していない。後続で `crates/synvelis-project/src/project.rs` の oracle 検査と regression test が必要。
- docs index（`docs/**/README.md`）は編集していない。Coordinator が統合時に整備する。
