---
id: "TASK-034RBFXktNsZRlOztedppD"
type: "task"
status: "done"
title: "SYN-OBS-01 task5 logging テスト新設と mise 検証"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/syn-obs-01-host-logging-source-catalog.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-obs-01-host-logging-source-catalog.md"
    - "docs/tasks/syn-obs-01-task2-owner-catalog-app-info.md"
    - "docs/tasks/syn-obs-01-task3-tracing-events.md"
    - "docs/tasks/syn-obs-01-task4-bin-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-obs-01-logging-source-catalog.md"
  references: []
  defers: []
  deferred-by: []
---

## Work

- `crates/synvelis-app/tests/logging.rs` 新設:
  - `resolve` 優先順位: CLI>env>既定、OFF 既定、`--log` 裸=info、
    `--log-file` 有効化含意、format 未知値エラー、`AL_LOG` 非読取。
  - `to_logging_config`: OFF→None、ON→sinks 構成(stderr Required/
    file BestEffort・rotation/max_files/queue_capacity)。
  - `install` OFF→`InstallState::Off`・health 無し。
  - global install → 2度目 install が `AlreadyInstalled`(global 系
    検証はこの1テストに集約し、他テストは scoped dispatch)。
  - イベント配信: `build_logging`+`with_default` scoped で
    `tracing::info!` が file sink(JSONL)へ届く(tempdir)。
  - `syn.app.info` の logging セクション形状(state/sinks/degraded)。
  - `ObservabilityOwner::sources()` が canonical descriptor/scope/
    reader を持ち、increment が reader 経由で読める。
- `tests/service_ops.rs` に `syn.app.info` logging セクションの
  形状・OFF 既定を追加検証。
- `cargo tree` 系で `synvelis-recording` 等他 crate の normal 依存木に
  `tracing` が無いことを確認(手動検証、結果を impl record に記録)。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/syn-obs-01-host-logging-source-catalog.md`

## Dependencies

- `docs/tasks/syn-obs-01-task2-owner-catalog-app-info.md`
- `docs/tasks/syn-obs-01-task3-tracing-events.md`
- `docs/tasks/syn-obs-01-task4-bin-wiring.md`

## Affected Files

- `crates/synvelis-app/tests/logging.rs`(新規)、
  `crates/synvelis-app/tests/service_ops.rs`

## Done When

- [ ] 新規テスト全 pass。
- [ ] `mise run check`/`lint`/`test`/`test-rt`/`check-headless-deps` pass。

## Verification

- [ ] `cargo test -p synvelis-app` pass
- [ ] `mise run test` / `mise run test-rt` pass
