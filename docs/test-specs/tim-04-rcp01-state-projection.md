---
id: "TSPEC-Tim04Rcp01StateProjection01"
type: "test-spec"
status: "approved"
title: "TIM-04 RCP-01 persisted plugin-state projection test spec"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
  verifies:
    - "docs/specs/tim-04-rcp01-state-projection.md"
    - "docs/designs/tim-04-rcp01-state-projection.md"
---

# TIM-04 RCP-01 persisted plugin-state projection test spec

## Purpose

persisted TIM-04 plugin state が RCP-01 recipe resolution と RCP-02 artifact lookup eligibility へ fail-closed に投影されることを固定する。実 plugin Offline render や real ABI capability の Pass を主張しない。

## Candidate boundary

この test-spec が対象にするのは、persisted TIM-04 state を RCP-01/RCP-02 eligibility へ投影する contract である。実 plugin Offline render、real ABI permission capability、typed runtime process oracle、製品 registry/discovery は upstream capability または別 owner の設計が必要な候補外であり、ここでは Pass を主張しない。

## Rules

- R1: `Captured` + `Coordinated`/`Quiescent` は blob bytes の digest と長さを検証したうえで exact-eligible evidence となり、`RecipeResolution::Resolved` になる。closure の `PluginStateEvidence { digest, quality }` は manifest の値と一致する。
- R2: `Captured` + `BestEffort` は digest を捏造せず exact-ineligible となり、`PluginStateDegraded` の typed `Unknown` になる。closure に exact evidence を残さない。
- R3: `Unsupported` と `Failed` は digest を持たない outcome として typed `Unknown` になる。`Failed` kind は `UnknownCause::PluginStateDegraded.failure_kind` で `CaptureFailed`、`Timeout`、`PermissionDenied`、`Crashed`、`Corrupt`、`Unavailable`、`Missing` を区別できる。`Unsupported` と `Failed` も既存の typed `PluginStateDegraded` に写像し、下流の exhaustive match を壊す新しい `UnknownCause` variant は追加しない。
- R4: valid な manifest に対象 processor の record がない場合は `PluginStateUnknown` になる。projection error と混同しない。
- R5: malformed JSON、schema version mismatch、duplicate processor、manifest name/digest mismatch、missing/corrupt/unreadable blob は projection construction error となる。これらを成功、空 evidence exact、Quiescent/Coordinated evidence にしない。
- R6: store HEAD 経路と explicit manifest 経路の両方で projection を構築できる。HEAD absence と store read failure は fail-closed error である。
- R7: non-`Resolved` resolution を artifact `CacheLookup` へ渡した場合、verified artifact candidate が存在しても `LookupOutcome::Unknown` であり、`EligibleExact`/`EligiblePreview` に昇格しない。
- R8: `PluginStateEvidence { digest, quality }` の serialized shape、recipe schema version、canonicalization version、exact recipe key layout は変えない。
- R9: 状態操作は non-RT control path に限定する。RT callback から state save/restore/FS IO を呼ばない。

## Evidence schema

`evidence/sv128/tim04_rcp01_state_projection.jsonl` は JSONL とし、各行は `case`、`driver`、`verdict`、`detail`、`observed`、`environment` を持つ。`environment` は OS/arch、ruviform resolved rev、store backend、projection input source、manifest schema version を持つ。`observed` は fixture/projection input、processor outcome、digest、resolution verdict、lookup verdict、typed error、non-claim を持つ。`Pass`/`Fail`/`NotRun`/`Unsupported` を区別し、実 plugin Offline render Pass と書かない。

## Cases

`crates/synvelis-render/tests/tim04_rcp01_state_projection.rs` は projection matrix、recipe resolution、artifact lookup eligibility を headless に実走する。evidence row は deterministic fixture の実観測であり、real ABI、実 plugin、Offline render、GUI、3OS final acceptance の証拠ではない。
