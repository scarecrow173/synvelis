# sv539 — VariantPorts::standard への ArrangementAdapter 登録 evidence

Issue: `scarecrow173/synvelis#539` / branch `bugfix/539-arrangement-adapter`
OS: Windows (mise managed toolchain)。cargo 直接実行は補助証拠、
canonical = mise。

## Lane 別 Pass/NotRun

| Lane | 判定 | 根拠 |
| --- | --- | --- |
| OS-L1 headless/fixture | **Pass** | 下記 mise 検証コマンド全 pass。`cb11_e2e_07_arrangement_scope_composite_adopt_undo_scope_isolation` が arrangement scope の gateway fork→composite edit→adopt を headless で完走 |
| GUI-L2 | **NotRun** | GUI surface は対象外（headless gateway 契約のみ） |
| EXT-L3 実 render/RT・実 plugin | **NotRun** | 実機/実 PCM/実 RT は外部 gate（Issue が明示的に対象外） |
| 実 CLI/MCP/GUI E2E | **NotRun** | `VariantPorts::standard` 登録は両 host へ自動伝播するが、host E2E は本 slice の検証範囲外 |
| MSRV lane | **NotRun** | ユーザー指定により実行しない |

## 実行した検証（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass |
| `mise run check synvelis-gateway` | Pass |
| `mise run lint synvelis-gateway` | Pass |
| `mise run test synvelis-gateway` | Pass（32 test binaries 全 pass・0 failed。`cb11_ai_vcs_off` 3 件 — grantless AI-off・VCS-off lifecycle・arrangement composite adopt を含む） |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI framework deps 無し） |
| `mise run check synvelis-project` | Pass |
| `mise run test synvelis-project` | Pass（全 suite・0 failed — adapter 契約非回帰） |
| `mise run check synvelis-workspace` | Pass |
| `mise run test synvelis-workspace` | Pass（43 test binaries 全 pass・0 failed — `GateDeps` 伝播非回帰） |
| `mise run check synvelis-cli` | Pass（`VariantPorts::standard` consumer の下流確認） |
| `mise run check mcp-poc` | Pass（同上） |
| `mise exec -- prek run --all-files` | Pass |

## 補助証拠（canonical = mise）

- `cargo test --locked -p synvelis-gateway cb11_e2e_07`:
  3 passed / 0 failed（対象 test の早期確認用に実行）。
- `cargo check --locked --all-targets -p synvelis-gateway`:
  初回は `rev_before.0` の型不一致で fail → `rev_before + 1`
  に修正後 pass（`ProjectRevision` は `u64` newtype ではなく
  `revision` フィールドが `u64`）。

## 変更サマリ

- `crates/synvelis-gateway/src/variant.rs`:
  `ArrangementAdapter` import 追加・adapters map へ
  `VariantScopeKind::Arrangement` 登録・陳腐化 doc comment
  4 箇所更新。`adapter_for` の fail-closed 機構は維持。
- `crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs`:
  旧 fail-closed pin を
  `cb11_e2e_07_arrangement_scope_composite_adopt_undo_scope_isolation`
  へ positive flip。mute+move composite・ApproveToApply
  approval gate・単一 CAS=1 undo・scope 外 bitwise 不変を assert。
- gap 記録解消: `evidence/sv288/coverage.md`・
  `docs/impl/ir|exp|test-specs/cb-11-cross-gate.md`。
- doc chain: `docs/*/issue-539-arrangement-adapter.md`（slug 固定）。

## gap 解消の対応関係

`evidence/sv288/coverage.md`「判明した gap」・
`docs/impl/exp/cb-11-cross-gate.md` E8 が記録していた
「`VariantPorts::standard` が `ArrangementAdapter` を未配線」
は本 slice で解消。fail-closed `unsupported` の pin fixture は
positive 検証へ flip 済み。
