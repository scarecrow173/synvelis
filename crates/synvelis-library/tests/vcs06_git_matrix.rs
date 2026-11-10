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
    HistoryQuery, IndexEntryMode, ProjectRelativePath, RevisionSelector, VcsCancel, VcsReadProvider,
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
