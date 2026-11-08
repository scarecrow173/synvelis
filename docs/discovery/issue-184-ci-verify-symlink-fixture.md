---
id: "DISC-034U2L1N2kWQrSPjNRT7Vf"
type: "discovery"
status: "resolved"
title: "Issue #184 CI verify/evidence 失敗 — 非 canonical temp 綴りと symlink fixture"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/184"
    - "https://github.com/scarecrow173/synvelis/actions/runs/42431461645"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/issue-148-evidence-fail-open-gap.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-library/tests/common/mod.rs"
    - "crates/synvelis-library/tests/local.rs"
    - ".github/workflows/ci.yml"
  defers: []
  deferred-by: []
---

# Issue #184 CI verify/evidence 失敗 — 非 canonical temp 綴りと symlink fixture

## Summary

CI run `42431461645`（sha `8c11e311`）で `verify-macos`・
`verify (windows-latest)` の `mise run test` が `synvelis-library`
の symlink 系テスト 4 件で失敗し、fail-closed の `evidence` /
`ci-summary` が連鎖失敗した。原因は ruviform `ruviform-io-file` の
canonical root 契約に対し、テスト fixture が
`std::env::temp_dir()` の非 canonical 綴りをそのままルートに
使っていたこと。fixture 側で `fs::canonicalize` するのが正しい
修正境界であり、production の containment 動作は仕様通りだった
（confirmed）。

## Exploration Goal

Issue #184: `verify`/`evidence` failure の原因特定と修正方針の決定。
`evidence`/`ci-summary` が独立した失敗か、親 lane の連鎖かを切り分け、
synvelis 側・ruviform 側・CI 環境・flaky のどこに root cause があるかを
特定する。

## Key Issues and Assumptions

- `evidence`/`ci-summary` は fail-closed gate であり、親 job 失敗の
  連鎖で落ちる設計（confirmed: 両 job とも prerequisite failure を
  検出して失敗）。
- `ruviform-io-file` の `RootedDir` resolver は absolute link target
  を canonical root の component 列と照合し、不一致を `EscapesRoot`
  として拒否する（confirmed: pin revision `79d1db0` の
  `src/rooted/resolve.rs` で確認）。
- `std::env::temp_dir()` は canonical でない綴りを返しうる
  （macOS `/var`→`/private/var`、Windows 8.3 名・junction）（confirmed:
  ローカル junction 再現で同一 4 件が再現・修正後に解消）。
- ruviform の挙動は文書化済み契約であり synvelis 側 workaround で
  隠すべきではない（前提: ユーザー指定①）。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| fixture で temp root を canonicalize | 契約の正しい境界を修正、全テストに効く、依存追加なし | なし | yes |
| 個別テストで symlink target を正規化 | 影響範囲が狭い | 各所で重複、fixture の綴り不整合が残る | no |
| ruviform 側で alias 正規化を緩和 | 呼び出し側が綴りを気にしなくて済む | containment の厳密性を弱める恐れ、ruviform 変更が必要 | no |
| CI で TEMP/TMP を canonical 化 | 本番コード無変更 | CI 環境依存の対症療法、ローカル再現性なし | no |

## Tentative Conclusions and Hypotheses

- confirmed: `TempTree::new` で `create_dir_all` 後に
  `fs::canonicalize` したパスを保持する修正で、非 canonical TEMP
  環境の再現テストが全件 pass した。
- confirmed: `evidence` / `ci-summary` の失敗は独立した defect ではなく
  fail-closed gate の正しい連鎖失敗である。
- hypothesis: ruviform 側に「非 canonical 綴りの absolute link target
  をどう扱うべきか」の設計論点は残るが、本 Issue の範囲外
  （synvelis 側で canonical fixture を供給するのが契約適合）。

## Open Questions

- ruviform `ruviform-io-file` の resolver が非 canonical 綴りの
  absolute target を将来的に受理すべきか — ruviform Issue として
  別途検討（本タスクでは起票判断は Coordinator/ユーザーに委ねる）。

## Promotion Candidates

- [x] spec-doc needed: `docs/specs/issue-184-ci-verify-symlink-fixture.md`
- [ ] adr-doc needed: なし（既存 ADR の範囲内の fixture 修正）

## Sources and Relations

- https://github.com/scarecrow173/synvelis/issues/184
- https://github.com/scarecrow173/synvelis/actions/runs/42431461645
- ruviform pin: `Cargo.lock` rev `79d1db0a7ba82324f05a1a5ae934dbfa6f19d59d`
  `crates/ruviform-io-file/src/rooted/resolve.rs`

## Evidence

- 2026-11-08: CI ログで `verify-macos` / `verify (windows-latest)` の
  `Test workspace (mise run test)` step が `synvelis-library` の
  `dangling_symlink_is_missing_not_fatal`・
  `stat_reports_link_kind_not_target_kind`・
  `symlink_into_root_is_browsable_and_marked`・
  `symlink_loop_does_not_recurse_forever` の 4 件で失敗。
  `stat` は `PermissionDenied` / "path escapes the capability root"、
  他 3 件は symlink が `Container` でなく `Asset`/missing 扱い。
- 2026-11-08: ローカル Windows で junction による非 canonical TEMP
  （`TEMP`/`TMP` を alias 経由に設定）で同一 4 件を再現。
- 2026-11-08: `TempTree::new` に `fs::canonicalize` を追加後、同一
  非 canonical TEMP 環境で `cargo test -p synvelis-library --test local`
  が 34 passed / 0 failed。`mise run test` workspace 全件も pass。
