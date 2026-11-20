# Asset library browser

The library surface is the asset catalog: registered providers with
capability tags, a detached browser view, and typed browse/search/
preview/import actions — `syn.library.*` over a `LibraryBackend`
bound to the workspace (ALIB-05/ALIB-UX-H).

## Sub-features

- `library-providers` — `syn.library.providers` lists provider
  descriptors with capability tags; independent of any view and an
  empty list when no backend is bound (never an error).
- `library-visual` — `syn.library.visual` `open|close` spawns the
  detached `AssetLibrary` view and returns its `view_id`.
- `library-view` — `syn.library.view` projects providers + typed
  view state + the deterministic `ViewSpec` for a `view_id`.
- `library-action` — `syn.library.action` runs one `LibraryAction`:
  `provider.select`, `browse.*`, `search.*`, `entry.*`, `preview.*`,
  `import.*`, `drag.*`.

## How to get to it (user POV)

- The library browser opens as a detached view surface; the provider
  catalog populates the selector before any browse state exists.
- Entries browse → search → preview → import through the typed
  action vocabulary.

## Driving it with syn-ctl and the test lanes

Preconditions:

- `mise run build-app-uitest` + `syn-ctl` build; live legs need an
  interactive desktop session.
- `mise run test synvelis-app` lanes run headless on any host.

- **Op surface.** `mise run test synvelis-app --test library_browser`
  proves the `syn.library.*` acceptance surface end to end (ALIB-05 /
  ALIB-UX-H).
- **Live catalog.** `syn-ctl drive --op syn.library.providers`
  injects the read-only provider-catalog op through the seam —
  dispatch journaled; an unbound backend answers an empty list
  honestly.
- **View lifecycle.** `syn-ctl drive --op syn.library.visual
  --params '{"action":"open"}' --dry-run` prints the plan — `open` is
  a mutation (it spawns a detached view), so the live leg is
  scratch-only and the semantic proof stays in `library_browser`.

## Gotchas

- `syn.library.providers` never fails on an unbound backend — an
  empty provider list is the honest result, not a fault.
- `visual open`/`close` and `action` mutate view and library state —
  keep live legs to `providers`; `view` needs a `view_id` from an
  `open` first.
- `import.*`/`drag.*` actions touch project state — scratch projects
  only when driven live.
- The receipt journal is metadata-only (seq/direction/kind/bytes):
  `syn-ctl drive --op` proves live dispatch reached the app, not the
  catalog contents — library semantics stay in the lane.
