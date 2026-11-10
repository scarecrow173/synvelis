---
id: "SPEC-vcs06-fixture-fault-matrix"
type: "spec"
status: "approved"
title: "VCS-06 fixture-level fault matrix + evidence ledger"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "SPEC-vcs01-immutable-snapshot-resolver"
    - "SPEC-vcs02-semantic-diff-changeset"
    - "SPEC-vcs03-git-read-provider"
    - "SPEC-vcs05-apply-restore-headless"
  supersedes: []
  superseded_by: []
---

# VCS-06 fixture-level fault matrix + evidence ledger (synvelis#97)

## Intent

統合受入証跡: VCS-01..VCS-05 の fixture/headless 層を固定 Case ID
（`VCS-SNAP-01..06` / `VCS-DIFF-01..08` / `VCS-GIT-01..06` /
`VCS-UI-01..05` / `VCS-APPLY-01..06` / `VCS-RESTORE-01..06` /
`VCS-COMPAT-01..04`）で Expected/Observed 独立記録し、
`evidence/reports/vcs-06-fixture-matrix-ledger.md` に verdict を残す。

## Rules

- 実測のみ `Pass`/`Fail`。headless/persisted 層の成功を実機
  GUI/OS/provider/plugin/RT/consumer の Pass に換算しない
  （`Pass*` で区別）。
- 未計測レーンは `NotRun`、真に不可能な能力は `Unsupported`、
  未実装は `Blocked`/`NotRun` と理由付きで記録する。
- `src/` は変更しない。既存 pub API 経由の fixture/test と
  docs/evidence のみ。
- VCS-UI-01..05・実 GUI(#114)・consumer(#219/#294)・CI(#83)・
  他 OS・VCS Writer/3-way Merge は本 slice の対象外。

## Acceptance

- 既存 vcs01/02/03/05 fixture の未マッピング行を `vcs06_*.rs` で補完。
- 全 Case ID の verdict を ledger に行形式で記録。
- `mise run fmt-check/check/lint/test/test-rt` を canonical 検証として実行。
