# Evidence — synvelis#86 DoD(1): real v5 snapshot compatibility migration

Residual slice: prove that snapshot bytes written by the *actual* v5
build decode through the current ordered `STEPS` ladder, keep their
legacy digests verbatim, save under the new schema, reload, and
preserve the original snapshot and sibling blob bytes read-only.

Prior state (Issue #86 close review, 2026-11-10): the v10→v11 digest
step and the read-only `snapshot/` resolver existed, but every v5 test
(`tests/alib02_digest.rs`) built a **synthetic** v5 envelope by deleting
post-v5 keys from current project JSON. Whether genuine v5 wire bytes —
legacy `{"scope": ...}` control-owner tags, absent `routing`,
CRC-fold digests produced by the real v5 writer — survive the ladder
was unproven ("deferred stub" in the close verdict). This slice adds the
real fixture and the end-to-end test; no production-code change was
required (see "Measured findings").

## Fixture provenance

| Item | Value |
| --- | --- |
| Producer commit | `a401e5c` (`SCHEMA_VERSION = 5`, last v5-era checkout) |
| Generator | `emit_v5_fixture.rs` (this directory) — built in a detached worktree of that commit as `crates/synvelis-recording/examples/`; **not** part of the shipped tree |
| Command | `cargo run -p synvelis-recording --example emit_v5_fixture` inside the detached worktree |
| Output | `alib02-v5-legacy.json` (6,328 bytes as emitted; **6,329 as committed** — the repo `end-of-file-fixer` hook appends one `\n`; the JSON document is identical) + `alib02-v5-legacy.blob` (9,600 bytes, byte-exact — NUL bytes make git treat it as binary) |
| Envelope | `schema_version: 5`, `revision: 22`, project_id `project-…0086` |
| Legacy digest | `5fea10b959fe82a9` — forward+reverse CRC32 fold of the blob, produced by the v5-era `synvelis_recording::crc32` |
| Committed fixture | `crates/synvelis-project/tests/fixtures/alib02-v5-legacy.{json,blob}` |

The fixture exercises the legacy surface: `{"scope": …}` control-owner
encoding, no `routing`/`monitoring`/`plugin_instances`/`smf_documents`
keys, asset + source + one applied-import receipt carrying the CRC
digest, a recording session/pass/stream/segment/take, tempo and
signature events, and a clip windowed over `source_region 0..384000`.

## Measured findings (what the code actually does)

1. **Decode** — `ProjectController::load` deserializes the real v5
   body unchanged: `ControlOwner` accepts the legacy `scope` tag via
   serde alias; absent post-v5 keys default through `#[serde(default)]`.
2. **Migration** — the ordered `STEPS` ladder runs v5→v6
   (`migrate_v5_routing` projects one strip per track + `Main`
   connections into the master bus), v6→v7 (scope tags → typed
   `ControlOwner` + derived `AutomationTimeScope`), v7→v8 (bootstrap
   master-fader target `MASTER_FADER_RAW`), v9→v10 (per-strip params),
   v10→v11 (`validate_digests`), v11→v12 (monitoring default),
   v12→v13 (plugin-instance backfill). Nothing v5 never had — sends,
   processors, extra buses, monitoring decls, plugin instances — is
   fabricated.
3. **Digest rule (measured)** — `AssetDigest::parse` is infallible;
   `kind()` classifies `sha256:<64hex>` → `Sha256`, exactly 16
   lowercase hex → `Crc32Fold`, anything else → `Opaque`.
   `is_content_addressed()` is true only for `Sha256`, so legacy
   digests are never promoted to verified identity. Empty strings are
   rejected by `validate_digests` at load/commit. The CRC digest is
   preserved **verbatim** on `Asset.digest`, `Source.content_digest`,
   and `AppliedImport.content_hash`; the receipt's `asset_id`/
   `source_id` still resolve to live entities.
4. **Read-only original** — a migrated load bumps the live revision
   (`22 → 23`) while `SavedState` retains the source HEAD, so `save`
   writes a **new** snapshot name. The v5 file bytes, the flat
   `blobs/<crc>` payload, a coexisting `blobs/sha256/<hex>` blob, and
   sibling `plugin-state/` opaque bytes are all verified unchanged
   after the round-trip (`DirectoryBundleStore`).
5. **Deterministic projection** — `SnapshotResolver::resolve_saved`
   on identical v5 bytes returns `schema_version 5 → projection
   version 13` with `PartialEq`-equal `Project`, equal asset manifest,
   `SnapshotValidation::Valid`, and `SnapshotCompleteness::Complete`.
6. **Degradation vocabulary (existing, exercised)** —
   `BlobRead::Absent` → `ContentStatus::Missing` + `Degraded` +
   `Partial`; no probe → `Unknown` + `Degraded`; `schema_version`
   outside the ladder → `SnapshotError::UnsupportedSchema` /
   `Error::UnsupportedSchema` (reject, not degrade); truncated bytes →
   `Error::Deserialize`; a v5 body smuggling a non-default
   `RoutingModel` → `Error::Validation` (foreign-state reject);
   duplicate CRC digests on distinct assets → both preserved,
   `Crc32Fold`, never content-addressed.

## Test that carries the proof

`crates/synvelis-project/tests/alib02_v5_real.rs` — 8 tests,
`cargo test -p synvelis-project --test alib02_v5_real`: **8 passed**
(local dev run, 2026-11; canonical `mise run test` result is recorded
in the PR body).

## Blob-namespace coexistence (from `synvelis-library`, read-only here)

`blob_relpath` maps `sha256:<64hex>` → `blobs/sha256/<lowerhex>` and
filename-safe legacy digests → flat `blobs/<digest>`. A 16-hex CRC
fold can never collide with the `sha256/` subtree, so the two
namespaces coexist without rewriting — the flat blob in the fixture
bundle is preserved alongside a `sha256/` sibling.
