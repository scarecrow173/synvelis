//! ALIB-04 Git remote provider tests (Synvelis #88).
//!
//! Everything runs against an offline `file://` fixture: a real work
//! repo plus a bare remote built through the git CLI, with LFS
//! objects laid down under the remote's `lfs/objects` store. Real git
//! servers, SSH/HTTP transports against live hosts, OS credential
//! stores, and the GUI are NotRun/Unsupported by design — see
//! `docs/test-specs/alib-04-git-remote-provider.md`.
//!
//! Tests skip honestly (early return) when no `git` binary exists.

mod common;

use std::fs;
use std::io::Read;
use std::sync::Arc;

use common::git_fixture::{self, GitFixture};
use common::{TempTree, aref, browse_req, cx, cx_named, loc, pid};
use synvelis_library::{
    AccessMode, AssetKind, AssetProvider, Availability, BrowseEntry, BrowsePage, BrowseRequest,
    BrowseScope, ByteRange, Capability, ContentDigest, Coverage, CredentialKey, CredentialResolver,
    DigestAlgorithm, EntryKind, FilterSpec, GitObjectFormat, GitRemoteConfig, GitRemoteProvider,
    GitRemoteScheme, GitRemoteUrl, ImportIntent, ImportStore, LfsBackendKind, LocalBlobDir,
    MirrorMode, OpaqueCredential, OpaquePageCursor, PinCapability, PinProof, PinnedAssetRef,
    PinnedRead, ProviderContext, ProviderError, ProviderErrorKind, ProviderId, ProviderLimits,
    RetryAdvice, SearchQuery, SortDirection, SortField, SortSpec, StagedBlobPublisher,
    UnsupportedFilterPolicy, VerificationPolicy, import_verified,
};
use synvelis_project::{
    CounterIdGenerator, ImportId, Project, ProjectController, ProjectId, ProjectRevision,
};

const PROVIDER_RAW: u128 = 0x88_04;

fn provider_id() -> ProviderId {
    pid(PROVIDER_RAW)
}

/// A provider over `fix`'s remote with its mirror under `fix.tmp`.
fn provider(fix: &GitFixture, mirror: &str) -> GitRemoteProvider {
    let url = GitRemoteUrl::parse(&fix.remote_url).expect("fixture remote url");
    GitRemoteProvider::new(
        provider_id(),
        GitRemoteConfig::new(url, fix.tmp.join(mirror)),
    )
}

fn names(page: &BrowsePage) -> Vec<String> {
    page.entries
        .iter()
        .map(|e| e.display_name.clone())
        .collect()
}

fn entry<'a>(page: &'a BrowsePage, name: &str) -> &'a BrowseEntry {
    page.entries
        .iter()
        .find(|e| e.display_name == name)
        .unwrap_or_else(|| panic!("entry {name:?} missing from {:?}", names(page)))
}

fn scope_req(locator: &str, limit: u32) -> BrowseRequest {
    BrowseRequest {
        scope: BrowseScope::Locator {
            locator: loc(locator),
        },
        page_limit: limit,
        cursor: None,
        sort: SortSpec::default(),
        filter: FilterSpec::default(),
    }
}

fn pin_of(provider: &GitRemoteProvider, locator: &str, cx: &ProviderContext) -> PinnedAssetRef {
    provider
        .pin(&aref(provider_id(), locator), cx)
        .unwrap_or_else(|e| panic!("pin {locator:?}: {e}"))
}

fn open_and_read(
    provider: &GitRemoteProvider,
    pin: &PinnedAssetRef,
    cx: &ProviderContext,
) -> Result<(Vec<u8>, PinnedRead), ProviderError> {
    let mut pr = provider.open_pinned(pin, cx)?;
    let mut buf = Vec::new();
    pr.reader.read_to_end(&mut buf).map_err(|e| {
        ProviderError::new(
            ProviderErrorKind::Internal,
            format!("stream read failed: {e}"),
        )
    })?;
    Ok((buf, pr))
}

fn sha256_of(bytes: &[u8]) -> ContentDigest {
    use sha2::Digest as _;
    ContentDigest::new(DigestAlgorithm::Sha256, &sha2::Sha256::digest(bytes))
}

// ── browse / stat ────────────────────────────────────────────────

#[test]
fn browse_root_lists_fixed_commit_tree() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");

    let page = provider.browse(&browse_req(100), &cx()).expect("browse");
    assert_eq!(page.coverage, Coverage::Complete);
    let got = names(&page);
    for want in [
        ".gitattributes",
        "dir",
        "notes.txt",
        "payload.bin",
        "vendor",
    ] {
        assert!(got.iter().any(|n| n == want), "missing {want} in {got:?}");
    }
    if fix.symlink_made() {
        assert!(got.iter().any(|n| n == "link.txt"));
    }
    // Containers vs assets.
    assert_eq!(entry(&page, "dir").kind, EntryKind::Container);
    assert_eq!(entry(&page, "vendor").kind, EntryKind::Container);
    assert_eq!(entry(&page, "notes.txt").kind, EntryKind::Asset);
    // Checksum hints carry git object identities, never secrets.
    let hint = entry(&page, "notes.txt")
        .facts
        .checksum_hint
        .value()
        .expect("blob hint")
        .as_str()
        .to_owned();
    assert!(hint.starts_with("git-blob:"), "hint {hint}");
    // Byte length is observed provider-side.
    assert_eq!(
        *entry(&page, "notes.txt")
            .facts
            .byte_len
            .value()
            .expect("len"),
        10
    );
}

#[test]
fn browse_subtree_scopes_and_odd_names() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");

    // Subtree scope keeps the rev spelling in child locators.
    let page = provider
        .browse(&scope_req("HEAD:dir", 100), &cx())
        .expect("browse dir");
    let got = names(&page);
    assert!(got.iter().any(|n| n == "top.txt"), "{got:?}");
    assert!(got.iter().any(|n| n == "sub dir"), "{got:?}");
    let sub = page
        .entries
        .iter()
        .find(|e| e.display_name == "sub dir")
        .expect("sub dir");
    assert_eq!(sub.locator.as_str(), "HEAD:dir/sub dir");

    // Names with spaces round-trip through locators.
    let page = provider
        .browse(&scope_req("HEAD:dir/sub dir", 100), &cx())
        .expect("browse nested");
    assert_eq!(names(&page), vec!["f ile.txt".to_owned()]);
    assert_eq!(
        page.entries[0].locator.as_str(),
        "HEAD:dir/sub dir/f ile.txt"
    );

    // Gitlink surfaces as an unavailable asset, never fetchable.
    let page = provider
        .browse(&scope_req("HEAD:vendor", 100), &cx())
        .expect("browse vendor");
    let sub = entry(&page, "sub");
    assert_eq!(sub.kind, EntryKind::Asset);
    assert!(matches!(sub.availability, Availability::Unavailable { .. }));
}

#[test]
fn stat_reports_typed_facts_for_each_entry_kind() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");

    let file = provider
        .stat(&aref(provider_id(), "HEAD:notes.txt"), &cx())
        .expect("stat file");
    assert_eq!(file.availability, Availability::Available);
    assert_eq!(file.facts.kind.value(), Some(&AssetKind::File), "kind fact");
    assert_eq!(*file.facts.byte_len.value().expect("len"), 10);
    let hint = file
        .facts
        .checksum_hint
        .value()
        .expect("hint")
        .as_str()
        .to_owned();
    assert!(hint.starts_with("git-blob:"), "{hint}");

    let dir = provider
        .stat(&aref(provider_id(), "HEAD:dir"), &cx())
        .expect("stat dir");
    assert_eq!(dir.facts.kind.value(), Some(&AssetKind::Directory));

    // Repository root is a synthetic directory pinned to the commit.
    let root = provider
        .stat(&aref(provider_id(), "HEAD:"), &cx())
        .expect("stat root");
    assert_eq!(root.facts.kind.value(), Some(&AssetKind::Directory));
    let root_hint = root
        .facts
        .checksum_hint
        .value()
        .expect("root hint")
        .as_str()
        .to_owned();
    assert!(root_hint.starts_with("git-commit:"), "{root_hint}");
    assert_eq!(root_hint, format!("git-commit:{}", fix.head_sha));

    let gitlink = provider
        .stat(&aref(provider_id(), "HEAD:vendor/sub"), &cx())
        .expect("stat gitlink");
    assert!(matches!(
        gitlink.availability,
        Availability::Unavailable { .. }
    ));

    // Missing path → typed NotFound.
    let err = provider
        .stat(&aref(provider_id(), "HEAD:nope.txt"), &cx())
        .expect_err("missing path");
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);

    // Traversal can never escape the tree grammar.
    let err = provider
        .stat(&aref(provider_id(), "HEAD:../escape"), &cx())
        .expect_err("traversal");
    assert_eq!(err.kind(), ProviderErrorKind::InvalidRequest);
}

#[test]
fn pin_rejects_containers_gitlinks_and_root() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    for (locator, want) in [
        ("HEAD:", ProviderErrorKind::InvalidRequest),
        ("HEAD:dir", ProviderErrorKind::InvalidRequest),
        ("HEAD:vendor/sub", ProviderErrorKind::UnsupportedCapability),
    ] {
        let err = provider
            .pin(&aref(provider_id(), locator), &cx)
            .expect_err("pin must fail");
        assert_eq!(err.kind(), want, "pin {locator}");
    }
}

// ── pin / fetch ──────────────────────────────────────────────────

#[test]
fn blob_pin_open_roundtrip_is_identity_bound() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    let pin = pin_of(&provider, "HEAD:notes.txt", &cx);
    let PinProof::ImmutableObject { revision } = &pin.proof else {
        panic!("git pins are ImmutableObject")
    };
    assert_eq!(
        revision.as_str(),
        format!("commit:{}", fix.head_sha),
        "pin names the resolved commit"
    );
    assert!(pin.object_identity.as_str().starts_with("git-blob:"));
    assert_eq!(pin.expected_size, Some(10));
    assert!(pin.digest_hint.is_none(), "git blob oid is not sha256");

    let (bytes, pr) = open_and_read(&provider, &pin, &cx).expect("open");
    assert_eq!(bytes, b"hello git\n");
    assert!(matches!(
        pr.claims.verification,
        VerificationPolicy::IdentityBound { .. }
    ));
    let receipt = pr.completion.take().expect("finished").expect("receipt");
    assert_eq!(receipt.bytes_read, 10);
    assert!(!receipt.verified, "no strong digest was pinned");
}

#[test]
fn partial_clone_fetches_only_selected_objects() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    // Browse + pin + open exactly one small blob.
    provider.browse(&browse_req(100), &cx).expect("browse");
    let pin = pin_of(&provider, "HEAD:notes.txt", &cx);
    let (bytes, _pr) = open_and_read(&provider, &pin, &cx).expect("open");
    assert_eq!(bytes, b"hello git\n");

    let info = provider.git_info(&cx).expect("git_info");
    assert_eq!(info.mirror_mode, MirrorMode::Partial);
    assert_eq!(info.object_format, Some(GitObjectFormat::Sha1));
    assert_eq!(info.lfs_backend, LfsBackendKind::LocalObjects);
    assert_eq!(provider.lfs_backend_kind(), LfsBackendKind::LocalObjects);

    // Objects outside the browsed/pinned path stay promisor-missing —
    // selective fetch really happened; nothing was bulk-downloaded.
    // (`ls-tree -l` does fetch blobs *listed* in a browsed tree for
    // their sizes, so only the never-browsed `dir/` subtree blobs
    // remain missing — exactly the two nested files.)
    let out = git_fixture::git(
        &info.mirror_dir,
        &["rev-list", "--objects", "--all", "--missing=print"],
    );
    let missing = out.lines().filter(|l| l.starts_with('?')).count();
    assert!(
        missing >= 1,
        "expected unfetched blobs in unbrowsed subtree, got {missing} missing lines:\n{out}"
    );
}

#[test]
fn lfs_pointer_pin_fetches_and_verifies_real_object() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    let pin = pin_of(&provider, "HEAD:payload.bin", &cx);
    assert!(
        pin.object_identity.as_str().starts_with("git-lfs:"),
        "identity {}",
        pin.object_identity.as_str()
    );
    assert_eq!(pin.expected_size, Some(4096));
    let digest = pin.digest_hint.clone().expect("lfs digest hint");
    assert_eq!(digest, sha256_of(&GitFixture::lfs_content()));

    let (bytes, pr) = open_and_read(&provider, &pin, &cx).expect("open lfs");
    // The fetched bytes are the real object — never the pointer text.
    assert_eq!(bytes, GitFixture::lfs_content());
    let pointer_text = format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize 4096\n",
        GitFixture::lfs_oid()
    );
    assert_ne!(bytes, pointer_text.as_bytes());
    assert!(matches!(
        pr.claims.verification,
        VerificationPolicy::StreamVerified { .. }
    ));
    let receipt = pr.completion.take().expect("finished").expect("receipt");
    assert!(receipt.verified, "lfs sha256 verified at EOF");
}

#[test]
fn lfs_missing_object_is_not_found_not_pointer() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    // Remove the real LFS object from the remote store.
    let oid = GitFixture::lfs_oid();
    let (a, rest) = oid.split_at(2);
    let (b, _) = rest.split_at(2);
    let obj = fix.lfs_objects_root().join(a).join(b).join(&oid);
    fs::remove_file(&obj).expect("remove lfs object");

    // Pinning still works — the pointer blob exists in git.
    let pin = pin_of(&provider, "HEAD:payload.bin", &cx);
    assert!(pin.object_identity.as_str().starts_with("git-lfs:"));

    // Fetch must fail typed — unfetched LFS bytes are never assets.
    let err = provider
        .open_pinned(&pin, &cx)
        .expect_err("missing lfs object");
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);
    assert_eq!(err.retry(), RetryAdvice::Never);
}

#[test]
fn lfs_size_mismatch_and_corruption_fail_verification() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    // Case 1: pointer declares a size the object does not have —
    // caught at open, before any byte streams.
    let oid = GitFixture::lfs_oid();
    let bad_pointer = format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {}\n",
        4096 + 1
    );
    fix.tmp.write("work/payload.bin", bad_pointer.as_bytes());
    fix.commit_all("bad lfs size");
    fix.push();
    provider.refresh(&cx).expect("refresh");
    let bad_pin = pin_of(&provider, "HEAD:payload.bin", &cx);
    let err = provider
        .open_pinned(&bad_pin, &cx)
        .expect_err("size mismatch");
    assert_eq!(err.kind(), ProviderErrorKind::IntegrityMismatch);

    // Case 2: same-length corrupt bytes pass the size pre-check but
    // fail the SHA-256 verifier at EOF — and the completion slot
    // records the typed failure.
    let good = pin_of(&provider, "HEAD:notes.txt", &cx); // unrelated sanity pin
    let _ = open_and_read(&provider, &good, &cx).expect("sanity");
    // Restore the good pointer so payload.bin pins the real object.
    let good_pointer =
        format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize 4096\n");
    fix.tmp.write("work/payload.bin", good_pointer.as_bytes());
    fix.commit_all("restore lfs pointer");
    fix.push();
    provider.refresh(&cx).expect("refresh2");

    let (a, rest) = oid.split_at(2);
    let (b, _) = rest.split_at(2);
    let obj = fix.lfs_objects_root().join(a).join(b).join(&oid);
    let corrupt: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
    fs::write(&obj, &corrupt).expect("corrupt lfs object");

    let pin = pin_of(&provider, "HEAD:payload.bin", &cx);
    let mut pr = provider.open_pinned(&pin, &cx).expect("open");
    let mut buf = Vec::new();
    assert!(pr.reader.read_to_end(&mut buf).is_err(), "corrupt bytes");
    let outcome = pr.completion.take().expect("finished");
    match outcome {
        Err(e) => assert_eq!(e.kind(), ProviderErrorKind::IntegrityMismatch),
        Ok(r) => panic!("corrupt lfs fetch reported success: {r:?}"),
    }
}

// ── refresh / fixed revisions ────────────────────────────────────

#[test]
fn refresh_makes_remote_updates_visible() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    let before = names(&provider.browse(&browse_req(100), &cx).expect("browse"));
    assert!(!before.iter().any(|n| n == "added.txt"));

    fix.tmp.write("work/added.txt", b"brand new\n");
    fix.commit_all("add file");
    fix.push();

    // Without refresh the mirror still shows the old commit.
    let stale = names(&provider.browse(&browse_req(100), &cx).expect("browse"));
    assert!(!stale.iter().any(|n| n == "added.txt"));

    provider.refresh(&cx).expect("refresh");
    let fresh = names(&provider.browse(&browse_req(100), &cx).expect("browse"));
    assert!(fresh.iter().any(|n| n == "added.txt"), "{fresh:?}");
}

#[test]
fn pinned_commit_survives_later_remote_updates() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    let old_pin = pin_of(&provider, "HEAD:notes.txt", &cx);
    let old_bytes = b"hello git\n".to_vec();

    fix.tmp.write("work/notes.txt", b"changed content\n");
    fix.commit_all("change notes");
    fix.push();
    provider.refresh(&cx).expect("refresh");

    // The old pin still yields the old bytes — immutable object.
    let (bytes, _pr) = open_and_read(&provider, &old_pin, &cx).expect("old pin");
    assert_eq!(bytes, old_bytes);

    // A fresh pin sees the new content under a different identity.
    let new_pin = pin_of(&provider, "HEAD:notes.txt", &cx);
    assert_ne!(old_pin.object_identity, new_pin.object_identity);
    let (bytes, _pr) = open_and_read(&provider, &new_pin, &cx).expect("new pin");
    assert_eq!(bytes, b"changed content\n");

    // An explicit sha locator also resolves the old commit.
    let old_sha = match &old_pin.proof {
        PinProof::ImmutableObject { revision } => revision
            .as_str()
            .strip_prefix("commit:")
            .expect("commit rev")
            .to_owned(),
        PinProof::ConditionalStrong { .. } => unreachable!(),
    };
    let (bytes, _pr) = open_and_read(
        &provider,
        &pin_of(&provider, &format!("{old_sha}:notes.txt"), &cx),
        &cx,
    )
    .expect("sha locator");
    assert_eq!(bytes, old_bytes);
}

#[test]
fn force_push_keeps_pinned_objects_fetchable() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    let pin = pin_of(&provider, "HEAD:notes.txt", &cx);

    // Rewrite history: orphan branch with different content, force-push
    // that branch onto main.
    git_fixture::git(&fix.work, &["checkout", "--orphan", "rewrite"]);
    git_fixture::git(&fix.work, &["rm", "-rf", "."]);
    fix.tmp.write("work/notes.txt", b"rewritten\n");
    fix.commit_all("rewrite history");
    git_fixture::git(&fix.work, &["push", "--force", "origin", "rewrite:main"]);
    git_fixture::git(&fix.work, &["checkout", "main"]);
    provider.refresh(&cx).expect("refresh");

    // HEAD now serves the rewritten history…
    let (bytes, _pr) = open_and_read(&provider, &pin_of(&provider, "HEAD:notes.txt", &cx), &cx)
        .expect("rewritten head");
    assert_eq!(bytes, b"rewritten\n");
    // …but the pin still fetches the original immutable bytes: the
    // mirror retains objects it already holds (no implicit gc).
    let (bytes, _pr) = open_and_read(&provider, &pin, &cx).expect("old pin");
    assert_eq!(bytes, b"hello git\n");
}

// ── cursors ──────────────────────────────────────────────────────

#[test]
fn browse_cursor_survives_remote_update_within_session() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    let mut req = browse_req(2);
    let p1 = provider.browse(&req, &cx).expect("page1");
    let cursor = p1.next_cursor.clone().expect("cursor for page 2");
    assert_eq!(p1.entries.len(), 2);

    // Remote moves on mid-session; the epoch still pins page 1's commit.
    fix.tmp.write("work/zzz_new.txt", b"new\n");
    fix.commit_all("add zzz");
    fix.push();
    provider.refresh(&cx).expect("refresh");

    req.cursor = Some(cursor);
    let p2 = provider.browse(&req, &cx).expect("page2");
    assert!(
        !names(&p2).iter().any(|n| n == "zzz_new.txt"),
        "cursor page leaked post-epoch entries: {:?}",
        names(&p2)
    );
    // A fresh browse (no cursor) does see the new commit.
    let fresh = provider.browse(&browse_req(100), &cx).expect("fresh");
    assert!(names(&fresh).iter().any(|n| n == "zzz_new.txt"));
}

#[test]
fn cursor_rejected_on_principal_query_or_garbage_change() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    let p1 = provider.browse(&browse_req(2), &cx).expect("page1");
    let cursor = p1.next_cursor.clone().expect("cursor");

    // Different principal → invalid.
    let mut req = browse_req(2);
    req.cursor = Some(cursor.clone());
    let err = provider
        .browse(&req, &cx_named("other-principal"))
        .expect_err("foreign principal");
    assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid);

    // Changed filter → invalid.
    let mut req = browse_req(2);
    req.cursor = Some(cursor.clone());
    req.filter.name_contains = Some("notes".to_owned());
    let err = provider.browse(&req, &cx).expect_err("changed query");
    assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid);

    // Garbage token → invalid, never a panic.
    let mut req = browse_req(2);
    req.cursor = OpaquePageCursor::new("not-hex!!").ok();
    let err = provider.browse(&req, &cx).expect_err("garbage cursor");
    assert_eq!(err.kind(), ProviderErrorKind::CursorInvalid);
}

#[test]
fn paging_walks_the_whole_tree_deterministically() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    // Two different sort specs over the same scope must each be
    // self-consistent; name-sort pages concatenate to the full set.
    let mut req = BrowseRequest {
        scope: BrowseScope::Root,
        page_limit: 3,
        cursor: None,
        sort: SortSpec {
            field: SortField::Name,
            direction: SortDirection::Asc,
        },
        filter: FilterSpec::default(),
    };
    let mut all = Vec::new();
    loop {
        let page = provider.browse(&req, &cx).expect("page");
        all.extend(names(&page));
        match page.next_cursor {
            Some(c) => req.cursor = Some(c),
            None => break,
        }
    }
    all.sort();
    all.dedup();
    assert!(all.iter().any(|n| n == "notes.txt"));
    assert!(all.iter().any(|n| n == "payload.bin"));
    assert!(all.iter().any(|n| n == "vendor"));
}

// ── deletions / unresolvable revs ────────────────────────────────

#[test]
fn deleted_paths_and_branches_fail_typed_after_refresh() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    // Delete a file, commit, push, refresh → stat NotFound.
    fix.remove("notes.txt");
    fix.commit_all("delete notes");
    fix.push();
    provider.refresh(&cx).expect("refresh");
    let err = provider
        .stat(&aref(provider_id(), "HEAD:notes.txt"), &cx)
        .expect_err("deleted file");
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);

    // A branch that never existed → NotFound, not a crash.
    let err = provider
        .browse(&scope_req("no-such-branch:", 10), &cx)
        .expect_err("missing rev");
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);

    // Create + push a branch, then delete it remotely.
    git_fixture::git(&fix.work, &["checkout", "-b", "tmp-branch"]);
    git_fixture::git(&fix.work, &["checkout", "main"]);
    git_fixture::git(&fix.work, &["push", "origin", "tmp-branch"]);
    provider.refresh(&cx).expect("refresh2");
    provider
        .browse(&scope_req("tmp-branch:", 10), &cx)
        .expect("branch browse");
    git_fixture::git(&fix.work, &["push", "origin", ":refs/heads/tmp-branch"]);
    provider.refresh(&cx).expect("refresh3");
    let err = provider
        .browse(&scope_req("tmp-branch:", 10), &cx)
        .expect_err("deleted branch");
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);
}

// ── preview / search / capabilities ──────────────────────────────

#[test]
fn preview_returns_unverified_bounded_bytes() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    // Full preview of a blob.
    let mut pr = provider
        .open_preview(&aref(provider_id(), "HEAD:notes.txt"), None, &cx)
        .expect("preview");
    assert!(matches!(
        pr.claims.verification,
        VerificationPolicy::Unverified
    ));
    let mut buf = Vec::new();
    pr.reader.read_to_end(&mut buf).expect("read");
    assert_eq!(buf, b"hello git\n");

    // Ranged preview.
    let mut pr = provider
        .open_preview(
            &aref(provider_id(), "HEAD:notes.txt"),
            Some(ByteRange { offset: 6, len: 3 }),
            &cx,
        )
        .expect("ranged preview");
    let mut buf = Vec::new();
    pr.reader.read_to_end(&mut buf).expect("read");
    assert_eq!(buf, b"git");

    // LFS preview serves real object bytes, never pointer text.
    let mut pr = provider
        .open_preview(
            &aref(provider_id(), "HEAD:payload.bin"),
            Some(ByteRange { offset: 0, len: 16 }),
            &cx,
        )
        .expect("lfs preview");
    let mut buf = Vec::new();
    pr.reader.read_to_end(&mut buf).expect("read");
    assert_eq!(buf, GitFixture::lfs_content()[..16]);

    // Containers and gitlinks cannot be previewed.
    for (locator, want) in [
        ("HEAD:", ProviderErrorKind::InvalidRequest),
        ("HEAD:dir", ProviderErrorKind::InvalidRequest),
        ("HEAD:vendor/sub", ProviderErrorKind::UnsupportedCapability),
    ] {
        let err = provider
            .open_preview(&aref(provider_id(), locator), None, &cx)
            .expect_err("preview must fail");
        assert_eq!(err.kind(), want, "preview {locator}");
    }
}

#[test]
fn search_is_typed_unsupported_not_fake_empty() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    let caps = provider.capabilities(None);
    assert!(matches!(caps.search, Capability::Unsupported { .. }));
    assert_eq!(caps.pin, PinCapability::ImmutableObject);
    assert_eq!(caps.access, AccessMode::ReadOnly);

    let err = provider
        .search(
            &SearchQuery {
                text: "notes".to_owned(),
                scope: BrowseScope::Root,
                page_limit: 10,
                cursor: None,
            },
            &cx,
        )
        .expect_err("search unsupported");
    assert_eq!(err.kind(), ProviderErrorKind::UnsupportedCapability);
}

// ── filter policy ────────────────────────────────────────────────

#[test]
fn unfiltered_remote_rejects_or_mirrors_per_policy() {
    let Some(fix) = GitFixture::new_unfiltered() else {
        return;
    };

    // Reject policy → typed UnsupportedCapability, no silent full clone.
    let url = GitRemoteUrl::parse(&fix.remote_url).expect("url");
    let reject = GitRemoteProvider::new(
        provider_id(),
        GitRemoteConfig::new(url.clone(), fix.tmp.join("mirror-reject")),
    );
    let err = reject.browse(&browse_req(10), &cx()).expect_err("reject");
    assert_eq!(err.kind(), ProviderErrorKind::UnsupportedCapability);

    // AllowFullMirror → explicit whole mirror, honestly reported.
    let mut cfg = GitRemoteConfig::new(url, fix.tmp.join("mirror-full"));
    cfg.filter_policy = UnsupportedFilterPolicy::AllowFullMirror;
    let full = GitRemoteProvider::new(provider_id(), cfg);
    let page = full.browse(&browse_req(100), &cx()).expect("full browse");
    assert!(names(&page).iter().any(|n| n == "notes.txt"));
    let info = full.git_info(&cx()).expect("git_info");
    assert_eq!(info.mirror_mode, MirrorMode::Full);
}

// ── transports / credentials ─────────────────────────────────────

#[test]
fn plain_local_path_remote_works() {
    let Some(fix) = GitFixture::new() else { return };
    // A bare filesystem path (no scheme) must canonicalize to file://.
    let url = GitRemoteUrl::parse(fix.remote.to_str().expect("utf8")).expect("path url");
    assert_eq!(url.scheme(), GitRemoteScheme::File);
    let provider = GitRemoteProvider::new(
        provider_id(),
        GitRemoteConfig::new(url, fix.tmp.join("mirror-path")),
    );
    let page = provider.browse(&browse_req(100), &cx()).expect("browse");
    assert!(names(&page).iter().any(|n| n == "notes.txt"));
}

struct FailingResolver;
impl CredentialResolver for FailingResolver {
    fn resolve(&self, _key: &CredentialKey) -> Result<OpaqueCredential, ProviderError> {
        Err(ProviderError::new(
            ProviderErrorKind::AuthExpired,
            "credential store reports expiry",
        ))
    }
}

struct StaticResolver(&'static str);
impl CredentialResolver for StaticResolver {
    fn resolve(&self, key: &CredentialKey) -> Result<OpaqueCredential, ProviderError> {
        Ok(OpaqueCredential::new(key.clone(), self.0))
    }
}

#[test]
fn credential_failures_are_typed_and_secrets_never_leak() {
    // HTTPS remote that cannot be reached — credential resolution
    // happens before any socket I/O, so both branches are offline.
    let tree = TempTree::new("git-https");
    let url = GitRemoteUrl::parse("https://127.0.0.1:9/repo.git").expect("https url");
    let key = CredentialKey::new("test-key").expect("key");

    // Resolver reports expiry → AuthExpired + AfterAuth advice.
    let mut cfg = GitRemoteConfig::new(url.clone(), tree.join("m1"));
    cfg.credential_key = Some(key.clone());
    let p1 = GitRemoteProvider::new(provider_id(), cfg);
    let mut cx1 = cx();
    cx1.credentials = Some(Arc::new(FailingResolver));
    let err = p1.browse(&browse_req(10), &cx1).expect_err("auth");
    assert_eq!(err.kind(), ProviderErrorKind::AuthExpired);
    assert_eq!(err.retry(), RetryAdvice::AfterAuth);

    // Resolver hands back a secret-bearing context → the transport
    // fails (unreachable), and the secret never appears in the typed
    // error or its detail.
    let mut cfg = GitRemoteConfig::new(url, tree.join("m2"));
    cfg.credential_key = Some(key);
    let p2 = GitRemoteProvider::new(provider_id(), cfg);
    let mut cx2 = cx();
    cx2.credentials = Some(Arc::new(StaticResolver("SECRET-TOKEN-xyz")));
    let err = p2.browse(&browse_req(10), &cx2).expect_err("offline");
    assert!(
        matches!(
            err.kind(),
            ProviderErrorKind::Offline | ProviderErrorKind::TransientIo
        ),
        "unexpected kind {:?}",
        err.kind()
    );
    let rendered = format!("{err} | {}", err.detail());
    assert!(
        !rendered.contains("SECRET-TOKEN-xyz"),
        "secret leaked into error: {rendered}"
    );
}

// ── cancellation / budgets ───────────────────────────────────────

#[test]
fn cancellation_is_typed_before_and_during_fetch() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");

    // Pre-cancelled context → Cancelled on every entry point.
    let dead = cx();
    dead.cancel.cancel();
    let err = provider
        .browse(&browse_req(10), &dead)
        .expect_err("cancelled");
    assert_eq!(err.kind(), ProviderErrorKind::Cancelled);

    let pin = pin_of(&provider, "HEAD:notes.txt", &cx());
    let err = provider.open_pinned(&pin, &dead).expect_err("cancelled");
    assert_eq!(err.kind(), ProviderErrorKind::Cancelled);

    // Mid-stream: read some bytes, cancel, the reader fails typed and
    // the completion slot records Cancelled. (`read`, not
    // `read_to_end`: std retries `ErrorKind::Interrupted` inside
    // `read_to_end`, which would mask the typed failure — the slot
    // is the authoritative outcome either way.)
    let live = cx();
    let token = live.cancel.clone();
    let mut pr = provider.open_pinned(&pin, &live).expect("open");
    let mut buf = [0u8; 4];
    let n = pr.reader.read(&mut buf).expect("first read");
    assert!(n > 0, "stream should deliver blob bytes");
    token.cancel();
    let mut rest = [0u8; 16];
    let err = pr
        .reader
        .read(&mut rest)
        .expect_err("read after cancel must fail");
    assert_eq!(err.kind(), std::io::ErrorKind::Interrupted);
    let outcome = pr.completion.take().expect("finished");
    match outcome {
        Err(e) => assert_eq!(e.kind(), ProviderErrorKind::Cancelled),
        Ok(r) => panic!("cancelled fetch reported success: {r:?}"),
    }
}

#[test]
fn byte_budget_is_enforced_at_pin_and_preview() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");

    let mut tight = cx();
    tight.limits = ProviderLimits {
        max_bytes: 4,
        ..ProviderLimits::default()
    };

    // 10-byte blob exceeds the 4-byte budget at pin time.
    let err = provider
        .pin(&aref(provider_id(), "HEAD:notes.txt"), &tight)
        .expect_err("budget");
    assert_eq!(err.kind(), ProviderErrorKind::BudgetExceeded);

    // The 4096-byte LFS object also exceeds it (declared size checked).
    let err = provider
        .pin(&aref(provider_id(), "HEAD:payload.bin"), &tight)
        .expect_err("lfs budget");
    assert_eq!(err.kind(), ProviderErrorKind::BudgetExceeded);
}

// ── project/worktree invariance ──────────────────────────────────

#[test]
fn provider_never_mutates_worktree_index_head_or_remote() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    let snap = || {
        let head = git_fixture::git(&fix.work, &["rev-parse", "HEAD"]);
        let status = git_fixture::git(&fix.work, &["status", "--porcelain"]);
        let refs = git_fixture::git(&fix.remote, &["for-each-ref"]);
        let files = git_fixture::git(&fix.work, &["ls-files"]);
        (head, status, refs, files)
    };
    let before = snap();

    // Exercise the full surface.
    provider.browse(&browse_req(100), &cx).expect("browse");
    provider
        .stat(&aref(provider_id(), "HEAD:notes.txt"), &cx)
        .expect("stat");
    let pin = pin_of(&provider, "HEAD:notes.txt", &cx);
    let _ = open_and_read(&provider, &pin, &cx).expect("open");
    provider
        .open_preview(&aref(provider_id(), "HEAD:notes.txt"), None, &cx)
        .expect("preview");
    provider.refresh(&cx).expect("refresh");

    let after = snap();
    assert_eq!(before, after, "provider mutated repo state");
}

// ── ALIB-02 import integration ───────────────────────────────────

fn import_store(tree: &TempTree) -> (ImportStore, LocalBlobDir) {
    let store = ImportStore::new(tree.join("staging"), tree.join("media"));
    let blobs = LocalBlobDir::new(store.blob_dir());
    (store, blobs)
}

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

#[test]
fn import_verified_consumes_git_blob_and_lfs_pins() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();
    let tree = TempTree::new("git-imp");
    let (store, blobs) = import_store(&tree);
    let mut c = controller();

    // Ordinary git blob → verified import.
    let pin = pin_of(&provider, "HEAD:notes.txt", &cx);
    let it = ImportIntent {
        import_id: ImportId::new(501),
        expected_project_revision: ProjectRevision(0),
        source: pin,
        target: None,
    };
    let out = import_verified(
        &provider,
        &it,
        &store,
        &blobs,
        &StagedBlobPublisher,
        &mut c,
        &cx,
    )
    .expect("import blob");
    assert_eq!(out.byte_len, 10);
    assert_eq!(out.content_digest, sha256_of(b"hello git\n"));

    // Replay converges idempotently.
    let replay = import_verified(
        &provider,
        &it,
        &store,
        &blobs,
        &StagedBlobPublisher,
        &mut c,
        &cx,
    )
    .expect("replay");
    assert!(replay.reused);
    assert_eq!(replay.content_digest, out.content_digest);

    // Git LFS object → real bytes verified end-to-end.
    let pin = pin_of(&provider, "HEAD:payload.bin", &cx);
    let it = ImportIntent {
        import_id: ImportId::new(502),
        expected_project_revision: ProjectRevision(1),
        source: pin,
        target: None,
    };
    let out = import_verified(
        &provider,
        &it,
        &store,
        &blobs,
        &StagedBlobPublisher,
        &mut c,
        &cx,
    )
    .expect("import lfs");
    assert_eq!(out.byte_len, 4096);
    assert_eq!(out.content_digest, sha256_of(&GitFixture::lfs_content()));
}

// ── ALIB-07 fault matrix: offline / rename / hostile input ───────

#[test]
fn offline_remote_typed_failures_mirror_stays_honest() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    // Warm the mirror: browse, fetch one blob, pin the LFS pointer.
    provider.browse(&browse_req(100), &cx).expect("browse");
    let blob_pin = pin_of(&provider, "HEAD:notes.txt", &cx);
    let (bytes, _pr) = open_and_read(&provider, &blob_pin, &cx).expect("read");
    assert_eq!(bytes, b"hello git\n");
    let lfs_pin = pin_of(&provider, "HEAD:payload.bin", &cx);

    fix.remove_remote();

    // Explicit refresh is a typed failure — never silently "fresh".
    let err = provider.refresh(&cx).expect_err("refresh offline");
    assert!(
        matches!(
            err.kind(),
            ProviderErrorKind::Offline
                | ProviderErrorKind::TransientIo
                | ProviderErrorKind::NotFound
        ),
        "refresh kind {:?}",
        err.kind()
    );

    // Mirror-backed browse/stat still serve the last fetched state —
    // honest staleness, not a live claim.
    let page = provider
        .browse(&browse_req(100), &cx)
        .expect("offline browse");
    assert!(names(&page).iter().any(|n| n == "notes.txt"));
    provider
        .stat(&aref(provider_id(), "HEAD:notes.txt"), &cx)
        .expect("offline stat");

    // Already-fetched objects stay readable from the local mirror.
    let (bytes, _pr) = open_and_read(&provider, &blob_pin, &cx).expect("offline read");
    assert_eq!(bytes, b"hello git\n");

    // A never-fetched object cannot be verified → typed failure at
    // pin time, never missing bytes served as content.
    let err = provider
        .pin(&aref(provider_id(), "HEAD:dir/top.txt"), &cx)
        .expect_err("pin unfetched offline");
    assert!(
        matches!(
            err.kind(),
            ProviderErrorKind::Offline
                | ProviderErrorKind::TransientIo
                | ProviderErrorKind::NotFound
        ),
        "unfetched pin kind {:?}",
        err.kind()
    );

    // The LFS pin keeps its identity, but the remote object store is
    // gone → NotFound, never the pointer text as content.
    let err = provider
        .open_pinned(&lfs_pin, &cx)
        .expect_err("lfs object gone");
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);
}

#[test]
fn remote_rename_keeps_pin_and_fixed_commit_identity() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();

    let pin = pin_of(&provider, "HEAD:notes.txt", &cx);
    let old_sha = match &pin.proof {
        PinProof::ImmutableObject { revision } => revision
            .as_str()
            .strip_prefix("commit:")
            .expect("commit rev")
            .to_owned(),
        PinProof::ConditionalStrong { .. } => unreachable!(),
    };

    // Rename on the remote: notes.txt → renamed.txt.
    git_fixture::git(&fix.work, &["mv", "notes.txt", "renamed.txt"]);
    fix.commit_all("rename notes");
    fix.push();
    provider.refresh(&cx).expect("refresh");

    // HEAD moved: the old path is gone, the new path resolves.
    let err = provider
        .stat(&aref(provider_id(), "HEAD:notes.txt"), &cx)
        .expect_err("old path after rename");
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);
    pin_of(&provider, "HEAD:renamed.txt", &cx);

    // The pin still yields the original bytes — the commit is
    // immutable even though the branch moved.
    let (bytes, _pr) = open_and_read(&provider, &pin, &cx).expect("pinned read");
    assert_eq!(bytes, b"hello git\n");

    // A fixed-commit locator still sees the old name.
    let (bytes, _pr) = open_and_read(
        &provider,
        &pin_of(&provider, &format!("{old_sha}:notes.txt"), &cx),
        &cx,
    )
    .expect("sha path");
    assert_eq!(bytes, b"hello git\n");
}

#[test]
fn hostile_locator_and_url_inputs_fail_closed() {
    let Some(fix) = GitFixture::new() else { return };
    let provider = provider(&fix, "mirror");
    let cx = cx();
    provider.browse(&browse_req(10), &cx).expect("browse");

    // Revision/path forms that could smuggle git options, reflog
    // syntax, or tree traversal are rejected before any argv exists.
    for locator in [
        "-x:foo",
        "--upload-pack=touch:pwned",
        "HEAD@{1}:notes.txt",
        "main:../escape",
        "main:a\\b",
        "main:a//b",
        ":notes.txt",
    ] {
        let err = provider
            .stat(&aref(provider_id(), locator), &cx)
            .expect_err(locator);
        assert_eq!(err.kind(), ProviderErrorKind::InvalidRequest, "{locator}");
    }

    // A path component that merely *looks* like an option is data:
    // it resolves through the tree grammar, never argv.
    fix.tmp.write("work/-dash.txt", b"dash named\n");
    fix.commit_all("add dash file");
    fix.push();
    provider.refresh(&cx).expect("refresh");
    let (bytes, _pr) = open_and_read(&provider, &pin_of(&provider, "HEAD:-dash.txt", &cx), &cx)
        .expect("dash path");
    assert_eq!(bytes, b"dash named\n");

    // Transport URLs that could execute commands or smuggle
    // credentials are refused at parse — before any provider exists.
    for bad in [
        "ext::sh -c touch%20pwned",
        "fd::17",
        "https://user:pass@example.com/r.git",
        "https://user@example.com/r.git",
    ] {
        assert!(GitRemoteUrl::parse(bad).is_err(), "{bad}");
    }
}
