//! VCS-06 (issue #97): git-provider fixture rows not covered by
//! `vcs03_git_provider.rs` — symlink index entries, credential
//! non-disclosure, and history-page clamping.
//!
//! Real local temp repositories via the `git` CLI (same fixture as
//! VCS-03). Real remotes/auth, GC, OS permission matrices, and GUI
//! stay `NotRun` (see `evidence/reports/vcs-06-fixture-matrix-ledger.md`).

mod common;

use std::fmt::Write as _;

use common::vcs_git_fixture::VcsGitFixture;
use synvelis_library::{GitVcsConfig, GitVcsProvider};
use synvelis_project::vcs::{
    HistoryQuery, IndexEntryMode, ProjectRelativePath, ProjectScope, RevisionSelector, VcsCancel,
    VcsReadProvider, WorkingChangeKind,
};

fn fixture() -> VcsGitFixture {
    VcsGitFixture::new().expect("git required for VCS-06 git fixtures")
}

fn no_cancel() -> VcsCancel {
    VcsCancel::new()
}

// ── VCS-GIT-05: symlink entries ───────────────────────────────────

#[test]
fn symlink_index_entry_keeps_link_target_bytes() {
    let fx = fixture();
    fx.write("proj/real-target.txt", b"payload\n");
    fx.commit_all("base");

    // Stage a symlink purely through the index (mode 120000) — no
    // filesystem symlink privilege needed on Windows.
    fx.write(".linktmp", b"proj/real-target.txt");
    let oid = fx.git(&["hash-object", "-w", ".linktmp"]);
    let oid = oid.trim();
    fx.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("120000,{oid},proj/link"),
    ]);
    fx.remove(".linktmp");

    let p = fx.provider();
    let staged = p.resolve_staged_snapshot(&no_cancel()).unwrap();
    let link = staged
        .entries
        .iter()
        .find(|e| e.path == "proj/link")
        .expect("symlink index entry");
    assert_eq!(link.mode, IndexEntryMode::Symlink);

    // Commit the index verbatim (no `add -A`, which would drop the
    // index-only entry) and read the blob at the fixed revision:
    // the bytes are the link *target text*, never followed.
    fx.git(&["commit", "-m", "add symlink"]);
    let fixed = p
        .resolve_fixed_revision(&RevisionSelector::Head, &no_cancel())
        .unwrap();
    let bytes = p
        .read_revision_file(
            &fixed,
            &ProjectRelativePath::new("proj/link").unwrap(),
            &no_cancel(),
        )
        .unwrap();
    assert_eq!(bytes, b"proj/real-target.txt");
}

// ── VCS-GIT-06: credential non-disclosure ─────────────────────────

#[test]
fn remote_credentials_never_surface_in_provider_output() {
    let fx = fixture();
    fx.write("proj/a.txt", b"a\n");
    fx.commit_all("c1");
    // A remote URL carrying userinfo — the provider is local-read
    // only and must never echo it through any returned value/error.
    fx.git(&[
        "remote",
        "add",
        "origin",
        "https://user:s3cr3t-t0ken@example.com/org/repo.git",
    ]);

    let p = fx.provider();
    let cancel = no_cancel();
    let mut surface = String::new();
    let _ = write!(surface, "{:?}", p.capabilities());
    if let Ok(info) = <GitVcsProvider as VcsReadProvider>::detect_workspace(
        &synvelis_project::vcs::ProjectPath::new(
            common::git_fixture::plain_path(fx.path())
                .to_string_lossy()
                .into_owned(),
        )
        .unwrap(),
    ) {
        let _ = write!(surface, "{info:?}");
    }
    if let Ok(status) = p.working_status(&synvelis_project::vcs::ProjectScope::root(), &cancel) {
        let _ = write!(surface, "{status:?}");
    }
    if let Ok(page) = p.list_history(&HistoryQuery::default(), &cancel) {
        let _ = write!(surface, "{page:?}");
    }
    if let Ok(snap) = p.resolve_staged_snapshot(&cancel) {
        let _ = write!(surface, "{snap:?}");
    }
    if let Ok(rev) = p.resolve_fixed_revision(&RevisionSelector::Head, &cancel) {
        let _ = write!(surface, "{rev:?}");
    }
    assert!(
        !surface.contains("s3cr3t-t0ken"),
        "credential material leaked into provider surface"
    );
}

// ── VCS-GIT-06: history page bounds ───────────────────────────────

#[test]
fn history_limit_is_clamped_and_zero_rejected() {
    let fx = fixture();
    for i in 0..3 {
        fx.write("proj/f.txt", format!("v{i}\n").as_bytes());
        fx.commit_all(&format!("c{i}"));
    }

    // A provider configured with a tiny page cap must clamp a huge
    // requested limit instead of streaming unbounded history.
    let p = GitVcsProvider::open(&GitVcsConfig {
        root: common::git_fixture::plain_path(fx.path()),
        project_root: Some("proj".into()),
        blob_root: None,
        max_blob_bytes: 64 * 1024 * 1024,
        max_history_page: 2,
    })
    .expect("provider");
    let page = p
        .list_history(
            &HistoryQuery {
                limit: usize::MAX,
                ..HistoryQuery::default()
            },
            &no_cancel(),
        )
        .unwrap();
    assert!(page.nodes.len() <= 2, "page must be clamped: {page:?}");
    assert!(page.next_cursor.is_some(), "3 commits with cap 2 must page");

    // Zero limit is a caller error, never an empty-page success.
    let err = p
        .list_history(
            &HistoryQuery {
                limit: 0,
                ..HistoryQuery::default()
            },
            &no_cancel(),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        synvelis_project::vcs::VcsError::InvalidRequest { .. }
    ));
}

// ── VCS-GIT-REAL-01: packed object/ref read path ─────────────────
//
// `git gc`/`repack`/`pack-refs` move refs into `packed-refs` and
// objects into packfiles. The provider reads through git plumbing,
// so the fixed-revision read path must be indifferent to the
// loose-vs-packed storage layout — this row proves it on a real
// repo rather than assuming it.

#[test]
fn gc_pack_refs_and_repack_keep_fixed_revision_reads() {
    let fx = fixture();
    fx.write("proj/a.txt", b"v1\n");
    let c1 = fx.commit_all("c1");
    fx.git(&["checkout", "-b", "side", &c1]);
    fx.write("proj/b.txt", b"side\n");
    fx.commit_all("side-c");
    fx.git(&["checkout", "main"]);
    fx.write("proj/a.txt", b"v2\n");
    fx.commit_all("c2");
    fx.git(&["tag", "v1", &c1]);

    let p = fx.provider();
    let cancel = no_cancel();
    let pinned = p
        .resolve_fixed_revision(&RevisionSelector::Head, &cancel)
        .unwrap();

    // Force the packed layout: refs into packed-refs, objects into
    // a single pack, loose objects pruned.
    fx.git(&["pack-refs", "--all"]);
    fx.git(&["repack", "-ad"]);
    fx.git(&["gc", "--prune=now"]);

    // Honest precondition: the packed layout is really in effect —
    // packed-refs exists and the loose branch ref is gone.
    assert!(fx.git_common().join("packed-refs").exists());
    assert!(!fx.git_common().join("refs/heads/main").exists());

    // Every read surface still resolves the same fixed revisions.
    assert_eq!(pinned.commit_oid, fx.head());
    let bytes = p
        .read_revision_file(
            &pinned,
            &ProjectRelativePath::new("proj/a.txt").unwrap(),
            &cancel,
        )
        .unwrap();
    assert_eq!(bytes, b"v2\n");

    // Explicit oid, tag, and branch expressions resolve post-pack.
    for expr in [
        c1.clone(),
        "v1".to_owned(),
        "side".to_owned(),
        "main".to_owned(),
    ] {
        let rev = p
            .resolve_fixed_revision(&RevisionSelector::Expr(expr.clone()), &cancel)
            .unwrap_or_else(|e| panic!("expr {expr} must resolve post-gc: {e}"));
        assert!(rev.oid_matches_format(&rev.commit_oid));
    }
    let c1_rev = p
        .resolve_fixed_revision(&RevisionSelector::Expr(c1.clone()), &cancel)
        .unwrap();
    assert_eq!(c1_rev.commit_oid, c1);

    // History pages through packed commits; the side branch keeps
    // its own parent chain.
    let page = p.list_history(&HistoryQuery::default(), &cancel).unwrap();
    let oids: Vec<&str> = page
        .nodes
        .iter()
        .map(|n| n.revision.commit_oid.as_str())
        .collect();
    assert!(
        oids.contains(&c1.as_str()),
        "c1 missing from history: {oids:?}"
    );

    // The index read path is unaffected by object packing.
    let staged = p.resolve_staged_snapshot(&cancel).unwrap();
    assert!(staged.complete);
    assert!(staged.entries.iter().any(|e| e.path == "proj/a.txt"));
}

// ── VCS-GIT-REAL-02: fixed revision gone after prune ─────────────
//
// A pinned commit whose objects are genuinely collected must fail
// with the typed `RevisionGone` — never a silent re-resolve, never
// an empty read. Reflogs are expired first so `gc --prune=now`
// really removes the object (the honest precondition is asserted
// through `cat-file`).

#[test]
fn pruned_commit_is_revision_gone_not_silent() {
    let fx = fixture();
    fx.write("proj/a.txt", b"v1\n");
    fx.commit_all("c1");
    fx.write("proj/a.txt", b"doomed\n");
    let doomed = fx.commit_all("doomed");
    fx.git(&["reset", "--hard", "HEAD~1"]);
    fx.git(&["reflog", "expire", "--expire-unreachable=now", "--all"]);
    fx.git(&["gc", "--prune=now"]);

    // The object is affirmatively absent before the provider runs.
    let probe = fx.git_allow_fail(&["cat-file", "-e", &doomed]);
    assert!(!probe.status.success(), "doomed object must be pruned");

    let p = fx.provider();
    let cancel = no_cancel();
    let err = p
        .resolve_fixed_revision(&RevisionSelector::Expr(doomed.clone()), &cancel)
        .unwrap_err();
    assert!(
        matches!(err, synvelis_project::vcs::VcsError::RevisionGone { .. }),
        "pruned commit must be RevisionGone: {err:?}"
    );

    // HEAD and reachable history are unaffected by the prune.
    let head = p
        .resolve_fixed_revision(&RevisionSelector::Head, &cancel)
        .unwrap();
    assert_eq!(head.commit_oid, fx.head());
}

// ── VCS-GIT-REAL-03: file:// remote + force-push + fetch ─────────
//
// A `file://` remote exercises the fetch/remote-tracking read path
// without network or auth — it is *not* the authenticated-real-
// remote row (that stays `NotRun`). A second local clone force-
// pushes divergent history; after `git fetch` the remote-tracking
// ref moves while a previously pinned commit stays byte-identical.

#[test]
fn file_remote_force_push_fetch_preserves_pinned_revision() {
    let fx = fixture();
    fx.write("proj/a.txt", b"v1\n");
    fx.commit_all("c1");

    // Bare file:// remote + initial push.
    let remote = fx.tmp.join("remote.git");
    let remote_arg = common::git_fixture::plain_path(&remote)
        .to_string_lossy()
        .into_owned();
    common::git_fixture::git(&fx.repo, &["init", "--bare", &remote_arg]);
    // Bare `init` HEADs at the platform default branch; point it at
    // `main` so clones check out the pushed history.
    common::git_fixture::git(&remote, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    let url = common::git_fixture::path_to_file_url(&remote);
    fx.git(&["remote", "add", "origin", &url]);
    fx.git(&["push", "origin", "main"]);

    // Pin before the remote moves.
    let p = fx.provider();
    let cancel = no_cancel();
    let pinned = p
        .resolve_fixed_revision(&RevisionSelector::Head, &cancel)
        .unwrap();
    let pinned_bytes = p
        .read_revision_file(
            &pinned,
            &ProjectRelativePath::new("proj/a.txt").unwrap(),
            &cancel,
        )
        .unwrap();
    assert_eq!(pinned_bytes, b"v1\n");

    // A second clone force-pushes divergent history to the remote.
    let other = fx.tmp.join("other");
    let other_arg = common::git_fixture::plain_path(&other)
        .to_string_lossy()
        .into_owned();
    common::git_fixture::git(&fx.repo, &["clone", &url, &other_arg]);
    common::git_fixture::git(&other, &["config", "user.email", "other@test"]);
    common::git_fixture::git(&other, &["config", "user.name", "other"]);
    common::git_fixture::git(&other, &["config", "commit.gpgsign", "false"]);
    std::fs::write(other.join("proj/a.txt"), b"remote-v2\n").unwrap();
    common::git_fixture::git(&other, &["add", "-A"]);
    common::git_fixture::git(&other, &["commit", "-m", "remote c2"]);
    let remote_c2 = common::git_fixture::git(&other, &["rev-parse", "HEAD"]);
    common::git_fixture::git(&other, &["push", "--force", "origin", "main"]);

    // Fetch in the fixture repo: remote-tracking ref moves, local
    // HEAD/objects untouched by the read provider's perspective.
    fx.git(&["fetch", "origin"]);

    // The pinned revision is unchanged — same oid, same bytes.
    let still = p
        .resolve_fixed_revision(&RevisionSelector::Expr(pinned.commit_oid.clone()), &cancel)
        .unwrap();
    assert_eq!(still.commit_oid, pinned.commit_oid);
    let still_bytes = p
        .read_revision_file(
            &still,
            &ProjectRelativePath::new("proj/a.txt").unwrap(),
            &cancel,
        )
        .unwrap();
    assert_eq!(still_bytes, pinned_bytes);

    // The remote-tracking ref honestly reports the moved remote —
    // fetch worked and `origin/main` now names the force-pushed tip.
    let remote_tip = p
        .resolve_fixed_revision(&RevisionSelector::Expr("origin/main".to_owned()), &cancel)
        .unwrap();
    assert_eq!(remote_tip.commit_oid, remote_c2);
    assert_ne!(remote_tip.commit_oid, pinned.commit_oid);

    // Local HEAD did not move: the provider still pins c1.
    let head = p
        .resolve_fixed_revision(&RevisionSelector::Head, &cancel)
        .unwrap();
    assert_eq!(head.commit_oid, pinned.commit_oid);
}

// ── VCS-GIT-REAL-04: linked worktree index scope ─────────────────
//
// `git worktree add` shares the object store/refs but gives each
// linked worktree its own index and HEAD. Staging inside the linked
// worktree must be invisible to a provider bound to the main
// worktree — and visible (with its own index generation) to a
// provider bound to the linked root.

#[test]
fn linked_worktree_index_scope_is_isolated() {
    let fx = fixture();
    fx.write("proj/a.txt", b"base\n");
    fx.commit_all("base");

    // Linked worktree at a detached HEAD — shares `.git` objects,
    // owns `.git/worktrees/<name>/index`.
    let linked = fx.tmp.join("linked-wt");
    let linked_arg = common::git_fixture::plain_path(&linked)
        .to_string_lossy()
        .into_owned();
    fx.git(&["worktree", "add", "--detach", &linked_arg]);
    assert!(linked.join("proj/a.txt").exists());

    // Stage a modification ONLY in the linked worktree's index.
    std::fs::write(linked.join("proj/a.txt"), b"linked-staged\n").unwrap();
    common::git_fixture::git(&linked, &["add", "proj/a.txt"]);

    // Main-worktree provider: the linked index is a different
    // index — no staged entry, no status row, clean status.
    let main = fx.provider();
    let cancel = no_cancel();
    let main_staged = main.resolve_staged_snapshot(&cancel).unwrap();
    let main_entry = main_staged
        .entries
        .iter()
        .find(|e| e.path == "proj/a.txt")
        .expect("main index entry");
    let main_status = main.working_status(&ProjectScope::root(), &cancel).unwrap();
    assert!(
        main_status
            .entries
            .iter()
            .all(|e| e.index_change.is_none() && e.worktree_change.is_none() && !e.untracked),
        "linked staging leaked into main status: {:?}",
        main_status.entries
    );

    // Linked-worktree provider: same repository identity (shared
    // object store) but its own index shows the staged change.
    let linked_p = GitVcsProvider::open(&GitVcsConfig {
        root: common::git_fixture::plain_path(&linked),
        project_root: Some("proj".into()),
        blob_root: None,
        max_blob_bytes: 64 * 1024 * 1024,
        max_history_page: 100,
    })
    .expect("linked provider");
    let linked_status = linked_p
        .working_status(&ProjectScope::root(), &cancel)
        .unwrap();
    let linked_row = linked_status
        .entries
        .iter()
        .find(|e| e.path == "proj/a.txt")
        .expect("linked status row");
    assert_eq!(
        linked_row.index_change,
        Some(WorkingChangeKind::Modified),
        "linked index must report the staged edit"
    );

    let linked_staged = linked_p.resolve_staged_snapshot(&cancel).unwrap();
    let linked_entry = linked_staged
        .entries
        .iter()
        .find(|e| e.path == "proj/a.txt")
        .expect("linked index entry");
    assert_ne!(
        linked_entry.object_id, main_entry.object_id,
        "linked staged blob must differ from main's HEAD blob"
    );

    // Both worktrees pin the same commit — the object store is
    // genuinely shared while the indexes stay separate.
    let main_head = main
        .resolve_fixed_revision(&RevisionSelector::Head, &cancel)
        .unwrap();
    let linked_head = linked_p
        .resolve_fixed_revision(&RevisionSelector::Head, &cancel)
        .unwrap();
    assert_eq!(main_head.commit_oid, linked_head.commit_oid);
    assert_eq!(main_head.repository_id, linked_head.repository_id);
}
