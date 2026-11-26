---
id: "PLAN-pcC4L1RealAbi01"
type: "plan"
status: "approved"
title: "PC-C4 L1 real ABI binding plan"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-l1-real-abi-binding.md"
    - "docs/designs/pc-c4-l1-real-abi-binding.md"
  implemented-by:
    - "docs/tasks/pc-c4-l1-real-abi-binding.md"
    - "docs/impl/pc-c4-l1-real-abi-binding.md"
  depends-on:
    - "docs/designs/pc-c4-l1-real-abi-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
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

# PC-C4 L1 real ABI binding plan

1. TIM-03 oracle と同じ runner/artifact resolution、fixture staging、SHA-256 provenance
   を test-local helper として用意する（TIM-03 file は編集しない）。
2. deterministic project fixture を構成し、VST3/CLAP `test_gain` を production seam で
   out-of-process registration する。
3. activation/binding/frame-base/source-map/capability pins を照合する。
4. fast control と最小 automation を順序立てて流し、control ledger の `Dispatched`
   frame projection を検証する。
5. paced playback PCM を block classification し、non-identity normalized/plain mapping
   を検証する。
6. clip 追加で generation replacement と retirement evidence を検証する。
7. `mise run test-pc-c4-l1`、TIM-03 非回帰、変更 crate の check/lint/test、fmt、RT、
   headless-deps、ruviform-rev lane を実行する。
8. evidence JSONL/verification note/docs chain を確定し、PR へ残件を明記する。

## 検証計画

正規コマンドは mise task。直接 cargo は補助証拠扱い。`docs/**/README.md` index は
Worker では更新せず Coordinator 統合時整備に残す。
