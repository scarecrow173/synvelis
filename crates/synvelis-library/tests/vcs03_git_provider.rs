//! VCS-03 (issue #94): Git read provider headless fixtures.
//!
//! Covers the issue's acceptance rows against real temporary local
//! git repositories: fixed Commit/Tree reads that survive branch/tag
//! movement, merge-aware history (all parents), index capture with
//! generation drift, partial stage / untracked / missing-LFS explicit
//! degradation (never silent Equal), repository+project identity
//! separation, SHA-1/SHA-256 object formats vs project `ContentDigest`,
//! cancellation, capabilities, and a read-only oracle proving
//! checkout/index/worktree/HEAD invariance.
//!
//! Real OS permission matrices, real remotes/auth, GC, and GUI are
//! out of scope — those lanes belong to issue #97 (`NotRun`).

mod common;

use std::fs;
use std::str::FromStr;

use common::git_fixture::{plain_path, sha256_hex};
use common::vcs_git_fixture::VcsGitFixture;
use synvelis_library::GitVcsProvider;
use synvelis_project::vcs::{
    HistoryQuery, ProjectPath, ProjectRelativePath, ProjectScope, RevisionSelector, VcsCancel,
    VcsError, VcsObjectFormat, VcsReadProvider,
};
use synvelis_project::{
    Command, ContentDigest, ContentStatus, CounterIdGenerator, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, SCHEMA_VERSION, SnapshotCancel, SnapshotEnvelope,
    SnapshotLocator, SnapshotResolver, SnapshotValidation, VcsBytesPort, VcsLocator,
};

// ---------------------------------------------------------------------------
// fixture helpers
// ---------------------------------------------------------------------------

const PROJECT_FILE: &str = "proj/main.json";

fn fixture() -> VcsGitFixture {
    VcsGitFixture::new().expect("git required for VCS-03 fixtures")
}

fn no_cancel() -> VcsCancel {
    VcsCancel::new()
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    let cmd = Command {
        expected_revision: c.live_revision(),
        op,
    };
    c.apply(cmd).expect("fixture op")
}

/// A project with one registered asset whose digest is `digest`.
fn controller_with_asset(digest: &str) -> ProjectController {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    );
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    apply(
        &mut c,
        ProjectOp::RegisterAsset {
            digest: digest.into(),
        },
    );
    c
}

fn controller_plain() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

/// The exact bytes `ProjectController::save` would publish (same
/// envelope construction as the VCS-01 fixtures).
fn envelope_bytes(c: &ProjectController) -> Vec<u8> {
    let envelope = SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        project_id: c.live().project_id,
        revision: c.live_revision(),
        project: c.live().clone(),
    };
    serde_json::to_vec(&envelope).expect("envelope serialize")
}

/// Writes a committed project + one content-verified asset blob.
/// Returns `(project_bytes, asset_digest, blob_relpath)`.
fn seed_project(fx: &VcsGitFixture, asset_content: &[u8]) -> (Vec<u8>, String, String) {
    let hex = sha256_hex(asset_content);
    let digest = format!("sha256:{hex}");
    let c = controller_with_asset(&digest);
    let bytes = envelope_bytes(&c);
    fx.write(PROJECT_FILE, &bytes);
    let blob_rel = format!("proj/blobs/sha256/{hex}");
    fx.write(&blob_rel, asset_content);
    (bytes, digest, blob_rel)
}

fn staged_locator(repo: &str) -> SnapshotLocator {
    SnapshotLocator::VcsStaged {
        repository_id: repo.into(),
        project_path: PROJECT_FILE.into(),
        index_generation: None,
    }
}

fn committed_locator(repo: &str, rev: &str) -> SnapshotLocator {
    SnapshotLocator::VcsCommitted {
        repository_id: repo.into(),
        fixed_revision: rev.into(),
        project_path: PROJECT_FILE.into(),
    }
}

/// The repository-state oracle: index bytes hash, porcelain status,
/// HEAD, and the staged set — everything a read provider must leave
/// untouched.
fn repo_oracle(fx: &VcsGitFixture) -> String {
    let index = fx.repo.join(".git").join("index");
    let index_hash = fs::read(&index).map_or_else(|_| "no-index".into(), |b| sha256_hex(&b));
    let status = fx.git(&["status", "--porcelain=v1", "-z", "--no-renames"]);
    let head = fx.git_allow_fail(&["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]);
    let head = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    let staged = fx.git(&["ls-files", "--stage", "-z"]);
    format!("{index_hash}|{status}|{head}|{staged}")
}

/// Runs the full provider surface once (detection through byte
/// reads) — the workload the read-only oracle brackets.
fn exercise_provider(provider: &GitVcsProvider) {
    let cancel = no_cancel();
    let _ = provider.capabilities();
    let _ = provider.working_status(&ProjectScope::root(), &cancel);
    let _ = provider.list_history(&HistoryQuery::default(), &cancel);
    let fixed = provider
        .resolve_fixed_revision(&RevisionSelector::Head, &cancel)
        .expect("resolve head");
    let _ = provider.revision_relations(&fixed, &cancel);
    let _ = provider.read_revision_file(
        &fixed,
        &ProjectRelativePath::new(PROJECT_FILE).expect("path"),
        &cancel,
    );
    let _ = provider.resolve_staged_snapshot(&cancel);
    let _ = VcsBytesPort::index_generation(provider, &fixed.repository_id);
    let staged = VcsLocator::Staged {
        repository_id: fixed.repository_id.clone(),
        project_path: PROJECT_FILE.into(),
        index_generation: synvelis_project::IndexGeneration(String::new()),
    };
    let _ = VcsBytesPort::read_file(provider, &staged, PROJECT_FILE);
    let _ = VcsBytesPort::read_blob(provider, &staged, "sha256:00");
    let committed = VcsLocator::Committed {
        repository_id: fixed.repository_id.clone(),
        fixed_revision: fixed.commit_oid.clone(),
        project_path: PROJECT_FILE.into(),
    };
    let _ = VcsBytesPort::read_file(provider, &committed, PROJECT_FILE);
    let _ = VcsBytesPort::read_blob(provider, &committed, "sha256:00");
}

// ---------------------------------------------------------------------------
// identity, detection, capabilities
// ---------------------------------------------------------------------------

#[test]
fn detect_workspace_reports_identity_and_format() {
    let fx = fixture();
    fx.write("proj/main.json", b"{}");
    fx.write("notes.txt", b"n");
    fx.commit_all("c1");

    let info = <GitVcsProvider as VcsReadProvider>::detect_workspace(
        &ProjectPath::new(plain_path(&fx.repo).to_string_lossy().into_owned()).expect("path"),
    )
    .expect("detect");
    assert_eq!(info.project_root, "");
    assert_eq!(info.object_format, VcsObjectFormat::Sha1);
    assert!(info.head_commit.is_some());
    assert!(info.capabilities.history.is_supported());
    assert!(info.capabilities.staged_index.is_supported());
    assert!(info.capabilities.cancellation.is_supported());
    assert!(info.capabilities.resource_bounds.is_supported());

    // A provider rooted at the project subdir binds project_root="proj".
    let proj_dir = fx.repo.join("proj");
    let info_sub = <GitVcsProvider as VcsReadProvider>::detect_workspace(
        &ProjectPath::new(plain_path(&proj_dir).to_string_lossy().into_owned()).expect("path"),
    )
    .expect("detect subdir");
    assert_eq!(info_sub.project_root, "proj");
    assert_eq!(info_sub.repository_id, info.repository_id);

    // Non-repo path → NotRepository.
    let outside = fx.tmp.join("outside");
    fs::create_dir_all(&outside).expect("outside");
    let err = <GitVcsProvider as VcsReadProvider>::detect_workspace(
        &ProjectPath::new(plain_path(&outside).to_string_lossy().into_owned()).expect("path"),
    )
    .expect_err("not a repo");
    assert!(matches!(err, VcsError::NotRepository { .. }), "{err:?}");
}

#[test]
fn repository_identity_separates_repos_and_project_paths() {
    let fx_a = fixture();
    let fx_b = fixture();
    fx_a.write("proj/main.json", b"{}");
    fx_a.commit_all("a1");
    fx_b.write("proj/main.json", b"{}");
    fx_b.commit_all("b1");

    let pa = fx_a.provider();
    let pb = fx_b.provider();
    let repo_a = fx_a.repository_id();
    let repo_b = fx_b.repository_id();
    assert_ne!(repo_a, repo_b, "distinct object stores → distinct ids");

    let fixed_a = pa
        .resolve_fixed_revision(&RevisionSelector::Head, &no_cancel())
        .expect("resolve a");
    assert_eq!(fixed_a.repository_id, repo_a);
    // Repo B refuses repo A's fixed revision and locator.
    let err = pb
        .revision_relations(&fixed_a, &no_cancel())
        .expect_err("foreign binding");
    assert!(matches!(err, VcsError::ForeignBinding { .. }), "{err:?}");
    let err = pb
        .read_revision_file(
            &fixed_a,
            &ProjectRelativePath::new(PROJECT_FILE).expect("path"),
            &no_cancel(),
        )
        .expect_err("foreign binding read");
    assert!(matches!(err, VcsError::ForeignBinding { .. }), "{err:?}");

    // The byte port reports unknown repository_id as NotFound.
    let locator = VcsLocator::Committed {
        repository_id: repo_a.clone(),
        fixed_revision: fixed_a.commit_oid.clone(),
        project_path: PROJECT_FILE.into(),
    };
    let err =
        VcsBytesPort::read_file(&pb, &locator, PROJECT_FILE).expect_err("foreign repo locator");
    assert!(matches!(
        err,
        synvelis_project::VcsPortError::NotFound { .. }
    ));

    // Same repo, different project_root binding still reads the same
    // commit (project_root is scope metadata, not object identity).
    let p_root = fx_a.provider_at("");
    let fixed_root = p_root
        .resolve_fixed_revision(&RevisionSelector::Head, &no_cancel())
        .expect("resolve root");
    assert_eq!(fixed_root.commit_oid, fixed_a.commit_oid);
}

// ---------------------------------------------------------------------------
// fixed revisions, history, merge DAG
// ---------------------------------------------------------------------------

#[test]
fn linear_history_and_fixed_revision_survive_branch_move() {
    let fx = fixture();
    fx.write("proj/a.txt", b"one\n");
    let c1 = fx.commit_all("c1");
    fx.write("proj/a.txt", b"two\n");
    let c2 = fx.commit_all("c2");
    fx.write("proj/a.txt", b"three\n");
    let c3 = fx.commit_all("c3");

    let p = fx.provider();
    let cancel = no_cancel();

    // Selectors resolve once to the same fixed commit.
    let head = p
        .resolve_fixed_revision(&RevisionSelector::Head, &cancel)
        .unwrap();
    let main = p
        .resolve_fixed_revision(&RevisionSelector::Expr("main".into()), &cancel)
        .unwrap();
    let oid_c3 = p
        .resolve_fixed_revision(&RevisionSelector::Expr(c3.clone()), &cancel)
        .unwrap();
    assert_eq!(head.commit_oid, c3);
    assert_eq!(main.commit_oid, c3);
    assert_eq!(oid_c3.commit_oid, c3);
    assert_eq!(head.object_format, VcsObjectFormat::Sha1);
    assert_eq!(head.commit_oid.len(), 40);

    // Fixed reads return the pinned tree's bytes.
    let path = ProjectRelativePath::new("proj/a.txt").unwrap();
    assert_eq!(
        p.read_revision_file(&head, &path, &cancel).unwrap(),
        b"three\n"
    );
    let c1_rev = p
        .resolve_fixed_revision(&RevisionSelector::Expr("HEAD~2".into()), &cancel)
        .unwrap();
    assert_eq!(c1_rev.commit_oid, c1);
    assert_eq!(
        p.read_revision_file(&c1_rev, &path, &cancel).unwrap(),
        b"one\n"
    );

    // History: newest-first, single-parent chain, metadata present.
    let page = p
        .list_history(&HistoryQuery::default(), &cancel)
        .expect("history");
    let oids: Vec<&str> = page
        .nodes
        .iter()
        .map(|n| n.revision.commit_oid.as_str())
        .collect();
    assert_eq!(oids, [c3.as_str(), c2.as_str(), c1.as_str()]);
    // Linear chain: every node but the root has exactly one parent.
    assert_eq!(page.nodes[0].parents.len(), 1);
    assert_eq!(page.nodes[1].parents.len(), 1);
    assert_eq!(page.nodes[2].parents.len(), 0);
    assert!(page.nodes[0].message.as_deref().unwrap().contains("c3"));
    assert!(page.nodes[0].committed_at_unix.is_some());
    assert!(page.nodes[0].author.is_some());
}

#[test]
fn branch_and_tag_moves_do_not_change_fixed_reads() {
    let fx = fixture();
    fx.write("proj/a.txt", b"v1\n");
    let c1 = fx.commit_all("c1");
    fx.write("proj/a.txt", b"v2\n");
    fx.commit_all("c2");
    fx.write("proj/a.txt", b"v3\n");
    let c3 = fx.commit_all("c3");

    fx.git(&["tag", "v1", &c1]);
    fx.git(&["branch", "feat", &c1]);

    let p = fx.provider();
    let cancel = no_cancel();
    let fixed_c1 = p
        .resolve_fixed_revision(&RevisionSelector::Expr("v1".into()), &cancel)
        .unwrap();
    assert_eq!(fixed_c1.commit_oid, c1);

    // Force-move branch and tag.
    fx.git(&["branch", "-f", "feat", &c3]);
    fx.git(&["tag", "-f", "v1", &c3]);

    // Selectors now resolve to c3; the old fixed revision is unmoved.
    let feat = p
        .resolve_fixed_revision(&RevisionSelector::Expr("feat".into()), &cancel)
        .unwrap();
    let tag = p
        .resolve_fixed_revision(&RevisionSelector::Expr("v1".into()), &cancel)
        .unwrap();
    assert_eq!(feat.commit_oid, c3);
    assert_eq!(tag.commit_oid, c3);

    let path = ProjectRelativePath::new("proj/a.txt").unwrap();
    assert_eq!(
        p.read_revision_file(&fixed_c1, &path, &cancel).unwrap(),
        b"v1\n"
    );
    assert_eq!(
        p.read_revision_file(&feat, &path, &cancel).unwrap(),
        b"v3\n"
    );
}

#[test]
fn merge_commit_preserves_all_parents() {
    let fx = fixture();
    fx.write("proj/base.txt", b"base\n");
    let base = fx.commit_all("base");

    fx.git(&["checkout", "-b", "side", &base]);
    fx.write("proj/side.txt", b"side\n");
    let side = fx.commit_all("side");

    fx.git(&["checkout", "main"]);
    fx.write("proj/main.txt", b"main\n");
    let main_c = fx.commit_all("main work");

    // Merge side into main — two parents, no ff.
    fx.git(&["merge", "--no-ff", "-m", "merge side", "side"]);
    let merge = fx.head();

    let p = fx.provider();
    let cancel = no_cancel();
    let fixed = p
        .resolve_fixed_revision(&RevisionSelector::Expr(merge.clone()), &cancel)
        .unwrap();
    let rel = p.revision_relations(&fixed, &cancel).unwrap();
    let parent_oids: Vec<&str> = rel.parents.iter().map(|r| r.commit_oid.as_str()).collect();
    assert_eq!(parent_oids, [main_c.as_str(), side.as_str()]);

    // History node for the merge keeps both parents.
    let page = p
        .list_history(&HistoryQuery::default(), &cancel)
        .expect("history");
    let node = page
        .nodes
        .iter()
        .find(|n| n.revision.commit_oid == merge)
        .expect("merge node");
    assert_eq!(node.parents.len(), 2);
    assert!(
        node.parents
            .iter()
            .all(|r| r.object_format == VcsObjectFormat::Sha1)
    );

    // First-parent history still lists the merge's ancestors.
    let base_rel = p
        .revision_relations(
            &p.resolve_fixed_revision(&RevisionSelector::Expr(base.clone()), &cancel)
                .unwrap(),
            &cancel,
        )
        .unwrap();
    assert_eq!(base_rel.parents.len(), 0);
}

#[test]
fn rename_and_delete_are_visible_across_fixed_revisions() {
    let fx = fixture();
    fx.write("proj/old/name.txt", b"old\n");
    let c1 = fx.commit_all("add old");
    // Rename at the tree level: delete + add (object stores have no
    // rename records; similarity is a diff-time concern).
    fx.remove("proj/old/name.txt");
    fx.write("proj/new/name.txt", b"old\n");
    let c2 = fx.commit_all("rename");
    fx.remove("proj/new/name.txt");
    let c3 = fx.commit_all("delete");

    let p = fx.provider();
    let cancel = no_cancel();
    let rev = |oid: &str| {
        p.resolve_fixed_revision(&RevisionSelector::Expr(oid.to_owned()), &cancel)
            .unwrap()
    };
    let old_path = ProjectRelativePath::new("proj/old/name.txt").unwrap();
    let new_path = ProjectRelativePath::new("proj/new/name.txt").unwrap();

    // c1: old present, new absent. c2: swapped. c3: both absent.
    assert_eq!(
        p.read_revision_file(&rev(&c1), &old_path, &cancel).unwrap(),
        b"old\n"
    );
    assert!(matches!(
        p.read_revision_file(&rev(&c1), &new_path, &cancel),
        Err(VcsError::MissingProjectFile { .. })
    ));
    assert!(matches!(
        p.read_revision_file(&rev(&c2), &old_path, &cancel),
        Err(VcsError::MissingProjectFile { .. })
    ));
    assert_eq!(
        p.read_revision_file(&rev(&c2), &new_path, &cancel).unwrap(),
        b"old\n"
    );
    assert!(matches!(
        p.read_revision_file(&rev(&c3), &new_path, &cancel),
        Err(VcsError::MissingProjectFile { .. })
    ));
}

// ---------------------------------------------------------------------------
// index capture, staged/committed snapshot reads, degradation
// ---------------------------------------------------------------------------

#[test]
fn committed_snapshot_is_stable_across_branch_moves() {
    let fx = fixture();
    let (bytes, _digest, _blob) = seed_project(&fx, b"asset-bytes-v1");
    let c1 = fx.commit_all("project v1");
    fx.write(PROJECT_FILE, b"{\"tampered\":true}");
    let _c2 = fx.commit_all("tamper");

    let p = fx.provider();
    let repo = fx.repository_id();
    let resolver = SnapshotResolver::new();
    let cancel = SnapshotCancel::new();

    // Committed read at c1 ignores later commits entirely.
    let snap = resolver
        .resolve_vcs(&p, &committed_locator(&repo, &c1), &cancel)
        .expect("committed resolve");
    assert_eq!(snap.origin, synvelis_project::SnapshotOrigin::VcsCommitted);
    // The project file bytes are the seeded envelope — the projection
    // parses and exposes the registered track.
    assert_eq!(snap.projection.project().tracks.len(), 1);
    // Force-move main; the fixed commit is untouched.
    fx.git(&["reset", "--hard", &c1]);
    let snap2 = resolver
        .resolve_vcs(&p, &committed_locator(&repo, &c1), &cancel)
        .expect("committed resolve again");
    assert_eq!(snap.identity.digest, snap2.identity.digest);
    let _ = bytes;
}

#[test]
fn staged_snapshot_reads_index_and_detects_partial_stage() {
    let fx = fixture();
    let (_bytes, _digest, blob_rel) = seed_project(&fx, b"asset-bytes");
    fx.stage(PROJECT_FILE);
    // Deliberately do NOT stage the blob → partial stage.

    let p = fx.provider();
    let repo = fx.repository_id();
    let resolver = SnapshotResolver::new();

    let err = resolver
        .resolve_vcs(&p, &staged_locator(&repo), &SnapshotCancel::new())
        .expect_err("partial stage must fail explicitly");
    assert!(
        matches!(err, synvelis_project::SnapshotError::PartialStage { .. }),
        "{err:?}"
    );

    // Stage the blob too → clean staged capture.
    fx.stage(&blob_rel);
    let snap = resolver
        .resolve_vcs(&p, &staged_locator(&repo), &SnapshotCancel::new())
        .expect("full staged resolve");
    assert_eq!(snap.assets.entries[0].status, ContentStatus::Present);
    assert_eq!(
        snap.completeness,
        synvelis_project::SnapshotCompleteness::Complete
    );
    assert_eq!(snap.validation, SnapshotValidation::Valid);
}

#[test]
fn untracked_asset_is_missing_never_silent_equal() {
    let fx = fixture();
    let (_bytes, digest, blob_rel) = seed_project(&fx, b"untracked-asset");
    // Commit only the project file; the blob stays untracked.
    fx.stage(PROJECT_FILE);
    fx.git(&["commit", "-m", "project only"]);
    // Blob file exists in worktree but is untracked AND uncommitted.
    assert!(fx.repo.join(&blob_rel).exists());

    let p = fx.provider();
    let repo = fx.repository_id();
    let head = fx.head();
    let resolver = SnapshotResolver::new();
    let snap = resolver
        .resolve_vcs(&p, &committed_locator(&repo, &head), &SnapshotCancel::new())
        .expect("committed resolve with missing asset");
    let entry = snap
        .assets
        .entries
        .iter()
        .find(|e| e.digest == digest)
        .expect("asset entry");
    assert_eq!(entry.status, ContentStatus::Missing);
    assert_eq!(snap.validation, SnapshotValidation::Degraded);
    assert_eq!(
        snap.completeness,
        synvelis_project::SnapshotCompleteness::Partial
    );
}

#[test]
fn index_generation_tracks_staging_changes() {
    let fx = fixture();
    fx.write("proj/a.txt", b"a\n");
    fx.commit_all("c1");
    fx.write("proj/b.txt", b"b\n");

    let p = fx.provider();
    let repo = fx.repository_id();
    let g1 = VcsBytesPort::index_generation(&p, &repo).unwrap();
    let g1_again = VcsBytesPort::index_generation(&p, &repo).unwrap();
    assert_eq!(g1, g1_again, "stable when index unchanged");

    fx.stage("proj/b.txt");
    let g2 = VcsBytesPort::index_generation(&p, &repo).unwrap();
    assert_ne!(g1, g2, "staging bumps the generation");

    fx.git(&["reset", "-q", "HEAD", "--", "proj/b.txt"]);
    let g3 = VcsBytesPort::index_generation(&p, &repo).unwrap();
    assert_eq!(g1, g3, "unstaging restores the fingerprint");

    // Unknown repository → NotFound.
    let err = VcsBytesPort::index_generation(&p, "git:deadbeef").unwrap_err();
    assert!(matches!(
        err,
        synvelis_project::VcsPortError::NotFound { .. }
    ));
}

#[test]
fn staged_snapshot_and_working_status_report_index_state() {
    let fx = fixture();
    fx.write("proj/committed.txt", b"c\n");
    fx.commit_all("c1");
    fx.write("proj/committed.txt", b"modified\n"); // worktree edit, unstaged
    fx.write("proj/staged.txt", b"s\n");
    fx.stage("proj/staged.txt"); // staged add
    fx.write("proj/untracked.txt", b"u\n"); // untracked

    let p = fx.provider();
    let cancel = no_cancel();

    let staged = p.resolve_staged_snapshot(&cancel).unwrap();
    assert!(staged.complete);
    let staged_paths: Vec<&str> = staged.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(staged_paths.contains(&"proj/staged.txt"));
    assert!(!staged_paths.contains(&"proj/untracked.txt"));
    assert!(staged.index_generation.0.starts_with("lsfiles-sha256:"));

    let status = p
        .working_status(&ProjectScope::new("proj").unwrap(), &cancel)
        .unwrap();
    let by_path = |path: &str| {
        status
            .entries
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("missing status entry for {path}"))
    };
    assert_eq!(
        by_path("proj/committed.txt").worktree_change,
        Some(synvelis_project::vcs::WorkingChangeKind::Modified)
    );
    assert_eq!(
        by_path("proj/staged.txt").index_change,
        Some(synvelis_project::vcs::WorkingChangeKind::Added)
    );
    assert!(by_path("proj/untracked.txt").untracked);
}

#[test]
fn merge_conflict_marks_staged_snapshot_incomplete() {
    let fx = fixture();
    fx.write("proj/conflict.txt", b"base\n");
    let base = fx.commit_all("base");
    fx.git(&["checkout", "-b", "side", &base]);
    fx.write("proj/conflict.txt", b"side\n");
    fx.commit_all("side");
    fx.git(&["checkout", "main"]);
    fx.write("proj/conflict.txt", b"main\n");
    fx.commit_all("main");
    // Conflicting merge — leaves unmerged index stages.
    let _ = fx.git_allow_fail(&["merge", "side"]);

    let p = fx.provider();
    let staged = p.resolve_staged_snapshot(&no_cancel()).unwrap();
    assert!(!staged.complete, "conflicted index is not complete");
    assert!(
        staged
            .conflicted_paths
            .iter()
            .any(|p| p == "proj/conflict.txt")
    );
    assert!(staged.entries.iter().any(|e| e.stage != 0));
}

// ---------------------------------------------------------------------------
// LFS, submodules, object formats
// ---------------------------------------------------------------------------

#[test]
fn missing_lfs_object_is_degraded_never_pointer_bytes() {
    let fx = fixture();
    let content = b"real-lfs-payload".to_vec();
    let hex = sha256_hex(&content);
    let digest = format!("sha256:{hex}");

    // LFS-track the blob namespace and commit a pointer whose oid is
    // the content digest — but never lay down the LFS object.
    fx.write(
        ".gitattributes",
        b"proj/blobs/sha256/* filter=lfs diff=lfs merge=lfs -text\n",
    );
    let c = controller_with_asset(&digest);
    fx.write(PROJECT_FILE, &envelope_bytes(&c));
    let blob_rel = format!("proj/blobs/sha256/{hex}");
    fx.write(
        &blob_rel,
        VcsGitFixture::lfs_pointer(&hex, content.len()).as_bytes(),
    );
    let head = fx.commit_all("lfs pointer only");

    let p = fx.provider();
    let repo = fx.repository_id();
    let snap = SnapshotResolver::new()
        .resolve_vcs(&p, &committed_locator(&repo, &head), &SnapshotCancel::new())
        .expect("resolve");
    let entry = &snap.assets.entries[0];
    assert_eq!(entry.status, ContentStatus::Missing);
    assert_eq!(snap.validation, SnapshotValidation::Degraded);

    // Now materialize the LFS object locally → the same committed
    // revision reads real content (pointer text is never served).
    fx.write_lfs_object(&hex, &content);
    let snap2 = SnapshotResolver::new()
        .resolve_vcs(&p, &committed_locator(&repo, &head), &SnapshotCancel::new())
        .expect("resolve with lfs object");
    assert_eq!(snap2.assets.entries[0].status, ContentStatus::Present);
}

#[test]
fn gitlink_entries_are_reported_not_fetched() {
    let fx = fixture();
    fx.write("proj/file.txt", b"x\n");
    let base = fx.commit_all("base");
    // Gitlink (submodule) at proj/vendor pointing at base.
    fx.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{base},proj/vendor"),
    ]);
    fx.git(&["commit", "-m", "add gitlink"]);

    let p = fx.provider();
    let staged = p.resolve_staged_snapshot(&no_cancel()).unwrap();
    let gitlink = staged
        .entries
        .iter()
        .find(|e| e.path == "proj/vendor")
        .expect("gitlink entry");
    assert_eq!(gitlink.mode, synvelis_project::vcs::IndexEntryMode::Gitlink);

    // File reads through a gitlink path are Unknown, never bytes.
    let fixed = p
        .resolve_fixed_revision(&RevisionSelector::Head, &no_cancel())
        .unwrap();
    let err = p
        .read_revision_file(
            &fixed,
            &ProjectRelativePath::new("proj/vendor").unwrap(),
            &no_cancel(),
        )
        .expect_err("gitlink is not a file");
    assert!(matches!(err, VcsError::Unavailable { .. }), "{err:?}");
}

#[test]
fn sha256_object_format_is_distinct_from_content_digest() {
    let fx = VcsGitFixture::new_sha256().expect("git required");
    fx.write("proj/a.txt", b"a\n");
    fx.commit_all("c1");

    let p = fx.provider();
    assert_eq!(p.capabilities().object_formats.len(), 1);
    assert!(
        p.capabilities()
            .object_formats
            .contains(&VcsObjectFormat::Sha256)
    );

    let fixed = p
        .resolve_fixed_revision(&RevisionSelector::Head, &no_cancel())
        .unwrap();
    assert_eq!(fixed.object_format, VcsObjectFormat::Sha256);
    assert_eq!(fixed.commit_oid.len(), 64, "sha256 object ids are 64-hex");

    // Object ids are NOT project content digests: the commit oid has
    // no `sha256:` prefix and never parses as a ContentDigest.
    assert!(ContentDigest::from_str(&fixed.commit_oid).is_err());
    // And a project digest is never a valid git oid input either.
    let digest = ContentDigest::sha256(b"x").to_string();
    assert!(digest.starts_with("sha256:"));
    assert_ne!(fixed.commit_oid, digest);
}

// ---------------------------------------------------------------------------
// cancellation + read-only invariance
// ---------------------------------------------------------------------------

#[test]
fn cancelled_token_aborts_every_operation() {
    let fx = fixture();
    fx.write("proj/a.txt", b"a\n");
    fx.commit_all("c1");
    let p = fx.provider();

    let cancel = VcsCancel::new();
    cancel.cancel();

    for result in [
        p.working_status(&ProjectScope::root(), &cancel).map(|_| ()),
        p.list_history(&HistoryQuery::default(), &cancel)
            .map(|_| ()),
        p.resolve_fixed_revision(&RevisionSelector::Head, &cancel)
            .map(|_| ()),
        p.resolve_staged_snapshot(&cancel).map(|_| ()),
    ] {
        assert!(matches!(result, Err(VcsError::Cancelled)), "{result:?}");
    }

    // File reads on a fixed revision also abort (the tree lookup runs
    // through the cancelled context).
    let fixed = {
        // Resolve before... the token is already cancelled, so build
        // the revision struct through a fresh provider call path is
        // impossible — use relations on a manually minted value? No:
        // read_revision_file needs a valid binding; cancel fires
        // inside the first git call regardless.
        let fresh = VcsCancel::new();
        p.resolve_fixed_revision(&RevisionSelector::Head, &fresh)
            .unwrap()
    };
    let err = p
        .read_revision_file(
            &fixed,
            &ProjectRelativePath::new("proj/a.txt").unwrap(),
            &cancel,
        )
        .expect_err("cancelled read");
    assert!(matches!(err, VcsError::Cancelled), "{err:?}");
}

#[test]
fn provider_is_read_only_across_the_whole_surface() {
    let fx = fixture();
    let (_bytes, digest, blob_rel) = seed_project(&fx, b"ro-bytes");
    fx.stage(PROJECT_FILE);
    fx.stage(&blob_rel);
    fx.commit_all("project");
    // Leave some worktree dirt so status has something to report.
    fx.write("proj/dirty.txt", b"dirty\n");

    let p = fx.provider();
    // Warm-up oracle: the fixture's own `git status` may refresh index
    // stat info once; the provider never writes, so a second oracle is
    // the stable baseline.
    let _ = repo_oracle(&fx);
    let before = repo_oracle(&fx);
    exercise_provider(&p);
    // Resolver paths too (staged + committed).
    let repo = fx.repository_id();
    let resolver = SnapshotResolver::new();
    let _ = resolver.resolve_vcs(&p, &staged_locator(&repo), &SnapshotCancel::new());
    let head = fx.head();
    let _ = resolver.resolve_vcs(&p, &committed_locator(&repo, &head), &SnapshotCancel::new());
    let _ = digest;

    assert_eq!(before, repo_oracle(&fx), "repo state must be invariant");
}

#[test]
fn provider_does_not_touch_project_session_or_undo() {
    let fx = fixture();
    fx.write("proj/main.json", &envelope_bytes(&controller_plain()));
    fx.commit_all("c1");

    // A live controller stands in for the DAW session; provider calls
    // must not observe or mutate it.
    let mut c = controller_plain();
    apply(
        &mut c,
        ProjectOp::AddTrack {
            name: "live".into(),
        },
    );
    let rev_before = c.live_revision();
    let tracks_before = c.live().tracks.len();

    let p = fx.provider();
    exercise_provider(&p);

    assert_eq!(c.live_revision(), rev_before);
    assert_eq!(c.live().tracks.len(), tracks_before);
}

#[test]
fn history_pagination_is_bounded_and_cursor_bound() {
    let fx = fixture();
    for i in 0..5 {
        fx.write("proj/n.txt", format!("n{i}\n").as_bytes());
        fx.commit_all(&format!("c{i}"));
    }
    let p = fx.provider();
    let cancel = no_cancel();

    let page1 = p
        .list_history(
            &synvelis_project::vcs::HistoryQuery {
                limit: 2,
                ..Default::default()
            },
            &cancel,
        )
        .unwrap();
    assert_eq!(page1.nodes.len(), 2);
    let cursor = page1.next_cursor.clone().expect("more pages");

    let page2 = p
        .list_history(
            &synvelis_project::vcs::HistoryQuery {
                limit: 2,
                cursor: Some(cursor.clone()),
                ..Default::default()
            },
            &cancel,
        )
        .unwrap();
    assert_eq!(page2.nodes.len(), 2);
    // No overlap between pages.
    let ids1: std::collections::BTreeSet<_> = page1
        .nodes
        .iter()
        .map(|n| n.revision.commit_oid.clone())
        .collect();
    assert!(
        page2
            .nodes
            .iter()
            .all(|n| !ids1.contains(&n.revision.commit_oid))
    );

    // A cursor replayed against a different start is rejected.
    let other = p
        .resolve_fixed_revision(&RevisionSelector::Expr("HEAD~4".into()), &cancel)
        .unwrap();
    let err = p
        .list_history(
            &synvelis_project::vcs::HistoryQuery {
                start: Some(RevisionSelector::Expr(other.commit_oid.clone())),
                limit: 2,
                cursor: Some(cursor),
                ..Default::default()
            },
            &cancel,
        )
        .expect_err("foreign cursor");
    assert!(matches!(err, VcsError::InvalidRequest { .. }), "{err:?}");
}
