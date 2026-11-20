# Variant diff explorer

The CB-09 Difference Explorer compares project variants: a view
lifecycle op, a persisted projection, and an action vocabulary
(select/audition/set-target/compare/adopt-preview/adopt) that gates
adoption on explicit endpoint pins plus an approval ticket
(synvelis#286).

## Sub-features

- `variant-visual` — `syn.variant-diff.visual` opens a Difference
  Explorer (`{target?}`) or closes it by `view_id`.
- `variant-view` — `syn.variant-diff.view` projects persisted
  view-local state + the deterministic `ViewSpec` for a `view_id`.
- `variant-action` — `syn.variant-diff.action` runs one CB-09 action
  (`select`/`audition`/`set-target`/`compare`/`adopt-preview`/
  `adopt`) — kind-gated per view.
- `variant-mount` — an opened explorer mounts through
  `mount_variant_diff_view` like the other product views.

## How to get to it (user POV)

- The Difference Explorer opens as its own view; compare pairs are
  pinned explicitly (A/B endpoints with freshness labels) before any
  adopt path exists.
- Audition plays a variant endpoint; adopt applies it — the approval
  ticket is the explicit consent step.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; live legs need an
  interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Op surface.** `mise run test synvelis-app --test
  cb09_variant_diff` proves the `syn.variant-diff.*` surface end to
  end: lifecycle kind-gating, persisted view-local state,
  explicit-pin compare, and the adopt/approval-ticket path.
- **Plan a lifecycle.** `syn-ctl drive --op syn.variant-diff.visual
  --params '{"action":"open"}' --dry-run` prints the plan naming the
  op — `open`/`close` and `action` mutate state, so live legs are
  scratch-only; the plan is the env-independent recipe.

## Gotchas

- `syn.variant-diff.view` needs a `view_id` from `visual open` — no
  explorer exists at baseline, so no op is a baseline live read.
- `adopt`/`adopt-preview` require explicit endpoint pins *and* the
  approval ticket — missing either is a named refusal, not a partial
  apply.
- `audition` produces sound — keep live audition legs out of shared
  environments; the lane covers it headlessly.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves live dispatch reached the app, not the
  diff — explorer semantics stay in `cb09_variant_diff`.
