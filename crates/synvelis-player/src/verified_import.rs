//! Consumer entry for the library verified-import pipeline
//! (synvelis#86 DoD(2)): local file → root-bounded provider pin →
//! [`synvelis_library::import_verified`] → published-blob decode
//! verification → `SetSourceState` advance to `Verified` — the
//! playback gate state.
//!
//! The library pipeline verifies *byte integrity* only and commits
//! sources at [`SourceState::Imported`]; media decode verification and
//! the state advance are this entry's responsibility, so a source the
//! player cannot decode is never reported as playable. Advancing
//! stops at `Verified` — the resolver gate — rather than `Committed`:
//! one extra command would change the undo granularity of the legacy
//! `import_local` path without changing playability.
//!
//! Everything here is blocking, non-RT worker code: file I/O,
//! hashing, demuxing and project mutation run on the caller's
//! non-RT thread, never on an audio thread.

use std::path::PathBuf;

use synvelis_library::{
    AssetProvider, ImportIntent, ImportStore, ImportTarget, LocalBlobDir, LocalFolderProvider,
    OpaqueLocator, ProviderContext, ProviderId, RemoteAssetRef, StagedBlobPublisher,
    import_verified,
};
use synvelis_project::{
    Command, ImportId, ProjectController, ProjectOp, ProjectRevision, SourceState,
};

use ruviform_media_core::{Fact, StreamKind};
use ruviform_media_io::{CodecId, DecodeMode, Demuxer};
use ruviform_media_reader::SourceReader;

use crate::blob::open_blob_verified;
use crate::error::VerifiedImportError;
use crate::import::VerifiedMedia;

/// Provider identity carried by the ephemeral per-import
/// [`LocalFolderProvider`] this entry binds to the file's parent
/// directory. The provider instance exists only for the call, but the
/// id travels inside the recorded pin and the durable manifest — a
/// constant keeps replay/resume pins identical. Exported so callers
/// driving `synvelis_library` job-recovery APIs (`inspect_imports`,
/// `rebase_import`, resume) can construct a provider that matches the
/// identity this entry recorded.
pub const VERIFIED_IMPORT_PROVIDER: ProviderId =
    ProviderId::new(0x7665_7269_6669_6564_696d_706f_7274_0001);

/// Parameters of one verified local-file import.
#[derive(Clone, Debug)]
pub struct VerifiedImportRequest {
    /// Local file to import — never itself a Source reference.
    pub path: PathBuf,
    /// Import job staging root (`<staging_root>/import-<id>/` —
    /// durable manifests enabling inspect/rebase/resume).
    pub staging_root: PathBuf,
    /// Verified media root (`<media_root>/blobs/sha256/<hex>`).
    pub media_root: PathBuf,
    /// Optional clip placement committed atomically with the import.
    pub target: Option<ImportTarget>,
    /// Import identity — `None` draws `draw_raw_id`; `Some` resumes or
    /// retries the durable manifest job with that identity.
    pub import_id: Option<ImportId>,
}

/// Result of [`import_verified_local`].
#[derive(Clone, Debug)]
pub struct VerifiedImportOutcome {
    /// Library pipeline receipt (blob publish + project commit +
    /// measured digest + reuse/interruption evidence).
    pub import: synvelis_library::ImportOutcome,
    /// Decode-verified media facts measured on the published blob.
    pub media: VerifiedMedia,
    /// Revision of the `SetSourceState`→`Verified` commit — `None`
    /// when the source was already at/above `Verified` (replay of an
    /// import whose state advance had already run).
    pub verified_revision: Option<ProjectRevision>,
}

/// Runs the verified consumer import for a local file.
///
/// Order of effects: provider pin → `import_verified` (durable
/// staged/verified/published/committed job) → published-blob
/// decode verification → `SetSourceState` advance to `Verified` at
/// the live `expected_revision`. The save decision stays with the
/// caller — `CommittedLive` is never reported as `SavedRevision`.
///
/// The source-file pin binds a per-call [`LocalFolderProvider`] to
/// the file's parent directory — the capability root is the
/// containing directory, the locator is the bare file name, so the
/// provider can never resolve outside it.
///
/// # Errors
/// [`VerifiedImportError`] — typed per stage: provider/pin, library
/// pipeline (stale revision, integrity conflict, publish, job state,
/// interruption), blob resolution, media decode, or the state-advance
/// commit. No variant leaves a partial project mutation — the library
/// commit is atomic and the state advance is a single `apply`.
pub fn import_verified_local(
    controller: &mut ProjectController,
    request: &VerifiedImportRequest,
    cx: &ProviderContext,
) -> Result<VerifiedImportOutcome, VerifiedImportError> {
    // ---- pin: canonical path → root-bounded provider pin ------------
    let canonical = request
        .path
        .canonicalize()
        .map_err(|e| VerifiedImportError::Io {
            detail: format!("canonicalize {}: {e}", request.path.display()),
        })?;
    let root = canonical.parent().ok_or_else(|| VerifiedImportError::Io {
        detail: format!("{} has no parent directory", canonical.display()),
    })?;
    let name = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| VerifiedImportError::Io {
            detail: format!("{} is not a UTF-8 file name", canonical.display()),
        })?;
    let provider = LocalFolderProvider::new(VERIFIED_IMPORT_PROVIDER, root)
        .map_err(VerifiedImportError::Provider)?;
    let asset = RemoteAssetRef {
        provider: VERIFIED_IMPORT_PROVIDER,
        locator: OpaqueLocator::new(name).map_err(VerifiedImportError::Provider)?,
    };
    let pin = provider
        .pin(&asset, cx)
        .map_err(VerifiedImportError::Provider)?;

    // ---- pipeline: import_verified (staged→verified→published→commit)
    let import_id = request
        .import_id
        .unwrap_or_else(|| ImportId::new(controller.draw_raw_id()));
    let intent = ImportIntent {
        import_id,
        expected_project_revision: controller.live_revision(),
        source: pin,
        target: request.target,
    };
    let store = ImportStore::new(&request.staging_root, &request.media_root);
    let blobs = LocalBlobDir::new(store.blob_dir());
    let outcome = import_verified(
        &provider,
        &intent,
        &store,
        &blobs,
        &StagedBlobPublisher,
        controller,
        cx,
    )
    .map_err(VerifiedImportError::Import)?;

    // ---- media verification: decode-verify the published blob -------
    // Runs through the same digest-kind blob layer the resolver uses,
    // so a source whose bytes cannot decode is never advanced.
    let digest = outcome.content_digest.to_string();
    let media = verify_blob_media(&request.media_root, &digest, &outcome)?;

    // ---- state advance: Imported → Verified --------------------------
    // `SetSourceState` is strictly one-stage-forward; a replay whose
    // source is already at/above `Verified` skips the op instead of
    // re-committing (mid-entry-crash resume still advances).
    let source_id = outcome.commit.source_id;
    let state = controller.live().sources.get(&source_id).map(|s| s.state);
    let verified_revision = if state == Some(SourceState::Imported) {
        Some(
            controller
                .apply(Command {
                    expected_revision: controller.live_revision(),
                    op: ProjectOp::SetSourceState {
                        source_id,
                        state: SourceState::Verified,
                    },
                })
                .map_err(|source| VerifiedImportError::CommitRejected { source })?,
        )
    } else {
        None
    };

    Ok(VerifiedImportOutcome {
        import: outcome,
        media,
        verified_revision,
    })
}

/// Decode-verifies the published blob through the shared digest-kind
/// resolution and measures the media facts a player needs — mirrors
/// `resolver`'s audio/PCM gate plus `import_local`'s full decode, so
/// `Verified` is only ever applied to bytes the player can actually
/// play.
fn verify_blob_media(
    media_root: &std::path::Path,
    digest: &str,
    outcome: &synvelis_library::ImportOutcome,
) -> Result<VerifiedMedia, VerifiedImportError> {
    let blob = open_blob_verified(media_root, digest).map_err(VerifiedImportError::Resolve)?;
    let demuxer = ruviform_io_symphonia::open_demuxer(blob.file).map_err(|e| {
        VerifiedImportError::Corrupt {
            detail: e.to_string(),
        }
    })?;
    let stream = demuxer
        .streams()
        .iter()
        .find(|s| s.stream.kind == StreamKind::Audio)
        .ok_or_else(|| VerifiedImportError::UnsupportedCodec {
            detail: "no audio stream".into(),
        })?
        .clone();
    if stream.codec.codec != CodecId::Pcm {
        return Err(VerifiedImportError::UnsupportedCodec {
            detail: format!("codec {:?} is not PCM", stream.codec.codec),
        });
    }
    let mut reader = SourceReader::open(Box::new(demuxer), stream.stream.id, DecodeMode::Canonical)
        .map_err(|e| VerifiedImportError::Corrupt {
            detail: e.to_string(),
        })?;
    let canonical = reader.metadata().canonical;
    let declared = match reader.metadata().frame_count {
        Fact::Known(frames) => Some(frames),
        _ => None,
    };
    let channels = canonical.layout.channels() as usize;
    let mut bufs: Vec<Vec<f32>> = vec![vec![0.0; 4096]; channels];
    let mut frames = 0_u64;
    loop {
        let mut views: Vec<&mut [f32]> = bufs.iter_mut().map(Vec::as_mut_slice).collect();
        let out =
            reader
                .read_planar(&mut views, 4096)
                .map_err(|e| VerifiedImportError::Corrupt {
                    detail: e.to_string(),
                })?;
        frames += out.frames as u64;
        if out.eof || out.frames == 0 {
            break;
        }
    }
    Ok(VerifiedMedia {
        digest: digest.to_owned(),
        byte_len: outcome.byte_len,
        frames,
        channels: canonical.layout.channels(),
        sample_rate: canonical.sample_rate.get(),
        declared_frames: declared,
        container: reader.metadata().container.format.name().to_owned(),
        codec: "pcm".into(),
        // Honest publish evidence, never the requested level —
        // DirectorySync::Unsupported (Windows) is recorded, not hidden.
        durability: format!(
            "{} dirsync={:?}",
            outcome.blob.durability, outcome.blob.directory_sync
        ),
    })
}
