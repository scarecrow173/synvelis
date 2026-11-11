---
id: "IMPL-H6s5wJEMrwXZ_wViwW0y8"
type: "impl"
status: "completed"
title: "synvelis#335 unborn HEAD 誤分類修正 — rev-parse --quiet 除去 slice"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/335"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/vcs-03-unborn-head-classification.md"
      - type: "file"
        path: "docs/specs/vcs-03-unborn-head-classification.md"
      - type: "file"
        path: "docs/designs/vcs-03-unborn-head-classification-design.md"
      - type: "file"
        path: "docs/plans/vcs-03-unborn-head-classification-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-03-unborn-head-classification-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-03-unborn-head-classification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/git/vcs.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/vcs03_git_provider.rs"
      - type: "file"
        path: "docs/designs/vcs-03-git-read-provider-design.md"
      - type: "file"
        path: "docs/test-specs/vcs-03-git-provider-fixtures.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-03-unborn-head-classification-task.md"
    - "docs/plans/vcs-03-unborn-head-classification-plan.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/vcs-03-unborn-head-classification-task.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-03-unborn-head-classification.md"
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
    - "docs/plans/vcs-03-unborn-head-classification-plan.md"
    - "docs/test-specs/vcs-03-unborn-head-classification.md"
    - "docs/impl/ir/vcs-03-git-read-provider.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/vcs-03-unborn-head-classification-task.md"
    - "crates/synvelis-library/src/git/vcs.rs"
    - "crates/synvelis-library/src/git/objects.rs"
    - "crates/synvelis-library/src/git/mirror.rs"
    - "crates/synvelis-library/tests/vcs03_git_provider.rs"
    - "crates/synvelis-library/tests/common/vcs_git_fixture.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# synvelis#335 unborn HEAD 誤分類修正 — rev-parse --quiet 除去 slice

## Summary

`GitVcsProvider` の `rev-parse --verify` 呼出（`resolve_oid`・
`head_oid`・`probe_workspace` の 3 箇所）から `--quiet` を除去した。
unborn HEAD・解決不能 revision は `--quiet` なしでは exit 128 +
`fatal: Needed a single revision`（stderr）で失敗し、既存の
`classify_kind` が `NotFound` に分類する。これにより `head_oid`
は `Ok(None)`、`resolve_oid` は `VcsError::RevisionGone` を返し、
VCS-03 design の unborn 意味論（`RevisionSelector::Head` →
`RevisionGone`、`head_commit: Option<String>` の `None` が正規
表現）が回復した。共有 `classify_kind`・`GitRunner`・mirror
provider・`synvelis-project` API は一切変更していない。

## Context

`docs/tasks/vcs-03-unborn-head-classification-task.md` /
`docs/plans/vcs-03-unborn-head-classification-plan.md` /
`docs/specs/vcs-03-unborn-head-classification.md` /
`docs/designs/vcs-03-unborn-head-classification-design.md` に従う。
Issue #335: commit 0 件 repo で `git rev-parse --verify --quiet
HEAD^{commit}` が exit 1 + 空 stderr を返し、`classify_kind` が
`TransientIo` に誤分類 → `VcsError::Unavailable` となり、
`syn.vcs.visual refresh` が service_error・VCS History 表示が
Failed になる欠陥。発見元は Issue #95（PR #334 の production
`set_vcs_backend` 配線）で「既知制限」として記録されていたもの。

## Diagnosis（因果の特定経過）

1. `GitRunner::run`（`objects.rs`）は非 0 exit を
   `classify_failure(stderr, code)` に渡す。`classify_kind` は
   stderr のパターン群を順に検査し、全て外れた場合 exit code
   フォールバック（`Some(128)`・その他 → `TransientIo`、
   負値 → `Cancelled`）へ落ちる。
2. `rev-parse --verify --quiet` は失敗時に stderr を出さず
   exit 1 で返る仕様のため、stderr が空 → 全パターン不合致 →
   `TransientIo`。`map_provider_error` は `TransientIo` を
   `VcsError::Unavailable` に写像する。
3. `head_oid` は `ProviderErrorKind::NotFound` のみを `Ok(None)`
   に写像する設計だったが、`--quiet` 経路では `NotFound` に
   到達不能 — 分類器が意味を確定できない silent failure を
   呼出側が作っていたのが根本原因。
4. scratch repo での実測: `--quiet` なしの `rev-parse --verify`
   は unborn HEAD・未知 oid・範囲外 peel の全てで exit 128 +
   stderr `fatal: Needed a single revision` を返し、これは
   `classify_kind` の `NotFound` パターンに合致する。
   `mirror.rs`（ALIB-04）は既に `--quiet` なしで同じ idiom を
   使っており、`tests/git.rs` がその stderr 由来 `NotFound`
   分類を assertion で固定している — **正しい分類経路は既に
   確立済みで、vcs.rs 側だけが `--quiet` でそれを潰していた**。

破られた不変条件と所有境界: 「revision が解決不能」は git の
失敗診断として stderr に現れ、意味確定は `classify_kind` が
担う。`--quiet` はその診断を抑止する呼出側の選択であり、
意味が確定する経路で使うべきでなかった。修正は呼出層（vcs.rs）
で行い、共有分類器の意味を変えない — silent exit-1 を
`NotFound` に倒す classifier 変更は mirror provider を含む全
呼出の blast radius を持つため採用しない。

## Implementation

### Changes

- `crates/synvelis-library/src/git/vcs.rs`
  - `resolve_oid`: rev-parse argv から `"--quiet"` を除去し、
    doc comment に「`--quiet` は silent exit 1 で `TransientIo`
    に誤分類される」契約を明記。
  - `head_oid`: 同様に `"--quiet"` を除去。`NotFound` →
    `Ok(None)` の写像は不変。unborn HEAD は
    `fatal: Needed a single revision` → `NotFound` → `Ok(None)`。
  - `probe_workspace` の head probe: `"--quiet"` を除去
    （`.ok()` が全失敗を `None` に畳む informational 経路のため
    semantic 不変。同一 idiom 統一のための整合）。
- `crates/synvelis-library/tests/vcs03_git_provider.rs`
  - `unborn_head_reports_typed_absence_not_unavailable` を追加
    （計 20 test）。0-commit repo（`fixture()` = `git init` +
    config のみ）で detect/status/resolve/history/staged の
    観測面を固定。
- `docs/designs/vcs-03-git-read-provider-design.md` — resolve 節の
  `rev-parse --verify --quiet` 記述を `--verify`（stderr 駆動
  `NotFound` 分類）へ整合、`updated` 更新。
- `docs/test-specs/vcs-03-git-provider-fixtures.md` — test 数
  19→20、VCS-03-UNBORN feature 節を追加、`updated` 更新。
- 文書チェーン: `docs/specs|designs|plans|tasks|test-specs|impl/ir/
  vcs-03-unborn-head-classification*`。

### Decisions During Implementation

- **`--quiet` 除去（案 a）を採用**: call-site で exit-1+空
  stderr を検出する案（b）は、`GitRunner::run` が失敗時に
  `ProviderError`（kind+sanitized detail）のみを返し exit
  code・生 stderr を露出しないため、runner API の拡張なしには
  実装不能。意味確定は git 診断 + `classify_kind` に委ねる方が
  責務境界として一貫する。
- **共有 `classify_kind` は不変**: Issue の境界条件どおり最後の
  手段とし、今回は不要と判断。`tests/git.rs` ALIB-04 系は
  `mise run test` で全件 pass を実測（blast radius ゼロの証拠）。
- **`probe_workspace` も `--quiet` 除去**: `.ok()` がエラーを
  全て捨てるため挙動差はゼロだが、3 箇所で同一 idiom を保つ
  方が将来のコピー元として安全。

## Related Experiments

### Adopted

- scratch repo での `rev-parse --verify` `--quiet` 有無の実測 —
  unborn/未知 oid/範囲外 peel の 3 系で exit 128 + `fatal:
  Needed a single revision`（stderr）を確認し、`NotFound`
  パターン合致を実証。

### Rejected

- `classify_kind` で silent exit-1 を `NotFound` に分類 —
  mirror provider（ALIB-04）の stderr 由来分類契約に波及する
  blast radius を持ち、Issue の境界条件に反する。
- `GitRunner` への exit code/stderr 生値の露出 API 追加 —
  runner の失敗表現を広げるより、`--quiet` を外して既存の
  診断経路を使う方が小さく正しい。

## Validation

ホスト: Windows（ローカル、rustc 1.99.0 / MSRV 1.89.0）。cargo 系は
`mise run` 経由、`cargo test -p synvelis-library --test
vcs03_git_provider` の直接実行は補助証拠（canonical = mise）。

- `mise run fmt-check` — exit 0。
- `mise run check` — exit 0。
- `mise run lint` — exit 0（clippy -D warnings）。
- `mise run test` — exit 0（workspace 全 pass。`git.rs` ALIB-04
  系を含む。`vcs03_git_provider` は 20/20 で
  `unborn_head_reports_typed_absence_not_unavailable` 含む）。
- `mise run test-rt` — exit 0。
- `mise run check-app-native` — exit 0。
- `mise run lint-app-native` — exit 0。
- `mise run test-msrv` — exit 0（rust 1.89.0 で workspace test）。
- `mise run build` — exit 0。
- 補助証拠（pre-fix fail 証明）: 修正前コードで回帰 test を実行
  → `working_status` が `Unavailable { detail: "TransientIo:
  git command failed" }` で決定的に失敗（Issue #335 の観測
  signature と一致）→ 修正後 20/20 pass。

未実行: PR CI の verify lane 実測（PR CI で観測する。外部 gate —
実 remote/provider・実 OS 耐久・GUI・RT 性能 — は NotRun であり
Pass 扱いしない）。

## Risks

- git の将来の診断文変更で `classify_kind` の `NotFound`
  パターンを外れる可能性は残る。unborn/未知 ref/範囲外 peel の
  3 系は `Needed a single revision` に収束することを実測済みで、
  回帰 test が同経路を固定する。
- `<expr>` が別種の fatal（例: `expected commit type`）を出す
  場合も exit 128 + stderr 非空であり、分類は従来どおり
  `TransientIo` フォールバック — `--quiet` 時代と同じで回帰
  しない。

## Follow-ups

- `probe_workspace` の head probe が unborn 以外の失敗を
  `head_commit: None` に畳む既存 semantics の再検討は別 slice
  候補（本 slice の範囲外、spec Deferred Design Concerns 記録
  どおり）。
- PR CI の verify lane 実測を観測し、unborn HEAD 経路の実機
  挙動を確認（マージ判断は Coordinator）。
