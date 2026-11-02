//! Safe local import: local file → non-RT staging → verify → measured
//! OS durability → verified blob materialization → expected-revision
//! project mutation. Blob publication and the project commit are
//! separate receipts; an interrupted import leaves an explicit orphan
//! (manifest + blob), never a partial project mutation.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use synvelis_project::{
    AppliedImport, Asset, AssetId, Command, IdGenerator, ImportId, Operation, Project,
    ProjectController, Source, SourceId, SourceState,
};
use synvelis_recording::crc32::{Crc32, crc32};
use synvelis_recording::staging::{
    DirSyncCapability, detect_dir_sync, publish_atomic, sync_parent_dir,
};

use ruviform_media_core::{Fact, StreamKind};
use ruviform_media_io::{CodecFormat, CodecId, DecodeMode, Demuxer};
use ruviform_media_reader::SourceReader;

use crate::error::ImportError;

/// Parameters of one safe local import.
#[derive(Clone, Debug)]
pub struct ImportRequest {
    /// Local file to import — never itself a Source reference.
    pub path: PathBuf,
    /// Staging root (`<staging_root>/import-<id>/`).
    pub staging_root: PathBuf,
    /// Verified media root (`<media_root>/blobs/<digest>`).
    pub media_root: PathBuf,
    /// Resume token from a previous attempt — replays the manifest
    /// instead of reserving fresh IDs.
    pub resume: Option<ImportId>,
}

/// Facts measured during verification (never trusted declarations
/// alone — the frame count is decoded, not only probed).
#[derive(Clone, Debug)]
pub struct VerifiedMedia {
    /// Verified content digest (hex16).
    pub digest: String,
    /// Byte length of the staged copy.
    pub byte_len: u64,
    /// Decoded frame count.
    pub frames: u64,
    /// Canonical channel count.
    pub channels: u32,
    /// Canonical sample rate.
    pub sample_rate: u32,
    /// Container-declared frame count, when the container carried one.
    pub declared_frames: Option<u64>,
    /// Container family name.
    pub container: String,
    /// Codec name.
    pub codec: String,
    /// Measured OS durability guarantee
    /// (`staging`/`media` `DirSyncCapability` labels).
    pub durability: String,
}

/// How the verified blob landed in the media store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobMaterialization {
    /// The blob was written and published during this import.
    Created,
    /// An identical-digest blob already satisfied the store.
    Reused,
}

/// Receipt of the verified blob publication — kept separate from the
/// project commit receipt per the Issue #24 contract.
#[derive(Clone, Debug)]
pub struct BlobReceipt {
    /// Content digest naming the blob.
    pub digest: String,
    /// Absolute blob path.
    pub path: PathBuf,
    /// Blob byte length.
    pub byte_len: u64,
    /// Created or reused.
    pub materialized: BlobMaterialization,
}

/// Receipt of the expected-revision project mutation.
#[derive(Clone, Debug)]
pub struct ProjectReceipt {
    /// Revision returned by `ProjectController::apply`.
    pub revision: u64,
    /// Import identity (idempotency key).
    pub import_id: ImportId,
    /// Registered asset.
    pub asset_id: AssetId,
    /// Registered source.
    pub source_id: SourceId,
}

/// Result of [`import_local`].
#[derive(Clone, Debug)]
pub struct ImportOutcome {
    /// Measured verification facts.
    pub media: VerifiedMedia,
    /// Blob publication receipt.
    pub blob: BlobReceipt,
    /// Project mutation receipt.
    pub commit: ProjectReceipt,
    /// `true` when this import was already applied (manifest replay —
    /// nothing was registered twice).
    pub reused: bool,
}

/// The atomic project mutation of a verified import: registers the
/// asset, the committed source, and the applied-imports entry in a
/// single `Operation` — a partial apply is impossible (the controller
/// applies on a scratch clone).
#[derive(Clone, Debug)]
pub struct CommitImport {
    /// Idempotency identity reserved by the manifest.
    pub import_id: ImportId,
    /// Reserved asset identity.
    pub asset_id: AssetId,
    /// Reserved source identity.
    pub source_id: SourceId,
    /// Verified content digest.
    pub content_hash: String,
}

impl Operation for CommitImport {
    fn validate(&self, project: &Project) -> Result<(), synvelis_project::Error> {
        use synvelis_project::Error;
        if let Some(applied) = project.applied_imports.get(&self.import_id) {
            return Err(Error::Validation(
                if applied.content_hash == self.content_hash {
                    format!("import {} already applied", self.import_id)
                } else {
                    format!("import {} integrity conflict", self.import_id)
                },
            ));
        }
        if project.assets.contains_key(&self.asset_id) || project.contains_source(&self.source_id) {
            return Err(Error::Validation("import reserved-id collision".into()));
        }
        Ok(())
    }

    fn apply(
        &self,
        project: &mut Project,
        _ids: &mut dyn IdGenerator,
    ) -> Result<(), synvelis_project::Error> {
        project.assets.insert(
            self.asset_id,
            Asset {
                id: self.asset_id,
                digest: self.content_hash.clone(),
            },
        );
        project.sources.insert(
            self.source_id,
            Source {
                id: self.source_id,
                asset_id: self.asset_id,
                content_digest: self.content_hash.clone(),
                state: SourceState::Committed,
            },
        );
        project.applied_imports.insert(
            self.import_id,
            AppliedImport {
                content_hash: self.content_hash.clone(),
                asset_id: self.asset_id,
                source_id: self.source_id,
            },
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum ImportStage {
    Staged,
    Verified,
    BlobPublished,
    Committed,
}

#[derive(Serialize, Deserialize)]
struct ImportManifest {
    import_id: ImportId,
    asset_id: AssetId,
    source_id: SourceId,
    source_path: String,
    digest: String,
    byte_len: u64,
    frames: u64,
    channels: u32,
    sample_rate: u32,
    stage: ImportStage,
}

fn manifest_path(stage_dir: &Path) -> PathBuf {
    stage_dir.join("import-manifest.json")
}

fn write_manifest(stage_dir: &Path, manifest: &ImportManifest) -> Result<(), ImportError> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|e| ImportError::StagingIo {
        detail: e.to_string(),
    })?;
    publish_atomic(&manifest_path(stage_dir), &bytes).map_err(|e| ImportError::StagingIo {
        detail: e.to_string(),
    })
}

/// Content digest: forward+reverse `crc32` fold, hex16 — the same
/// scheme as `synvelis-recording`'s commit payload hash.
#[must_use]
pub fn content_digest(bytes: &[u8]) -> String {
    let fwd = u64::from(crc32(bytes));
    let mut rev_bytes = bytes.to_vec();
    rev_bytes.reverse();
    let bwd = u64::from(crc32(&rev_bytes));
    format!("{fwd:08x}{bwd:08x}")
}

/// Streaming form of [`content_digest`] over one already-open handle:
/// the forward crc consumes sequential chunks, the backward crc
/// consumes chunks read tail→head with each chunk reversed — the same
/// result as `crc32(bytes.reversed())`, bounded memory. Digesting on
/// the same fd that is then handed to the demuxer closes the
/// verify-then-open gap: a replaced path cannot swap the bytes
/// between digest and decode. The cursor ends rewound to `0`.
///
/// # Errors
/// [`std::io::Error`] — read/seek failures.
pub fn content_digest_stream(file: &mut File) -> std::io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    const CHUNK: usize = 1 << 16;
    // usize fits u64 on every real target — the projection is exact.
    let chunk = u64::try_from(CHUNK).unwrap_or(u64::MAX);
    let len = file.seek(SeekFrom::End(0))?;
    file.seek(SeekFrom::Start(0))?;
    let mut buf = vec![0u8; CHUNK];
    let (mut fwd, mut bwd) = (Crc32::new(), Crc32::new());
    let mut pos = 0u64;
    while pos < len {
        let take = usize::try_from((len - pos).min(chunk)).unwrap_or(usize::MAX);
        file.read_exact(&mut buf[..take])?;
        fwd.update(&buf[..take]);
        pos += take as u64;
    }
    let mut end = len;
    while end > 0 {
        let take = usize::try_from(end.min(chunk)).unwrap_or(usize::MAX);
        let start = end - take as u64;
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut buf[..take])?;
        buf[..take].reverse();
        bwd.update(&buf[..take]);
        end = start;
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(format!(
        "{:08x}{:08x}",
        u64::from(fwd.finish()),
        u64::from(bwd.finish())
    ))
}

/// The canonical blob path for `digest` under `media_root`.
#[must_use]
pub fn blob_path(media_root: &Path, digest: &str) -> PathBuf {
    media_root.join("blobs").join(digest)
}

fn staging_dir(staging_root: &Path, import_id: ImportId) -> PathBuf {
    staging_root.join(format!("import-{:032x}", import_id.raw()))
}

fn read_manifest(stage_dir: &Path) -> Result<ImportManifest, ImportError> {
    let bytes = fs::read(manifest_path(stage_dir)).map_err(|e| ImportError::StagingIo {
        detail: e.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|e| ImportError::Corrupt {
        detail: format!("manifest: {e}"),
    })
}

/// Runs the safe local import against `controller`.
///
/// Order of effects: staged copy → verify → manifest → blob publish →
/// single atomic `CommitImport`. The caller owns `save` afterwards so
/// the Saved Snapshot receipt stays a separate event.
///
/// # Errors
/// [`ImportError`] — typed per stage; `Orphaned` reports a published
/// blob whose commit failed. No variant leaves a partial project
/// mutation.
#[allow(clippy::too_many_lines)] // linear pipeline; stages must stay ordered
pub fn import_local(
    controller: &mut ProjectController,
    request: &ImportRequest,
) -> Result<ImportOutcome, ImportError> {
    // ---- manifest: resume or reserve ----------------------------------
    let (mut manifest, stage_dir) = if let Some(import_id) = request.resume {
        let dir = staging_dir(&request.staging_root, import_id);
        (
            read_manifest(&dir).map_err(|_| ImportError::UnknownImport { import_id })?,
            dir,
        )
    } else {
        let import_id = ImportId::new(controller.draw_raw_id());
        let manifest = ImportManifest {
            import_id,
            asset_id: AssetId::new(controller.draw_raw_id()),
            source_id: SourceId::new(controller.draw_raw_id()),
            source_path: request.path.display().to_string(),
            digest: String::new(),
            byte_len: 0,
            frames: 0,
            channels: 0,
            sample_rate: 0,
            stage: ImportStage::Staged,
        };
        let dir = staging_dir(&request.staging_root, import_id);
        fs::create_dir_all(&dir).map_err(|e| ImportError::StagingIo {
            detail: e.to_string(),
        })?;
        write_manifest(&dir, &manifest)?;
        (manifest, dir)
    };

    // Idempotent replay: the registry is the authority.
    if let Some(applied) = controller.live().applied_imports.get(&manifest.import_id) {
        if applied.content_hash != manifest.digest {
            return Err(ImportError::Conflict {
                import_id: manifest.import_id,
            });
        }
        return Ok(ImportOutcome {
            media: VerifiedMedia {
                digest: manifest.digest.clone(),
                byte_len: manifest.byte_len,
                frames: manifest.frames,
                channels: manifest.channels,
                sample_rate: manifest.sample_rate,
                declared_frames: None,
                container: "wav".into(),
                codec: "pcm".into(),
                durability: "replayed".into(),
            },
            blob: BlobReceipt {
                digest: manifest.digest.clone(),
                path: blob_path(&request.media_root, &manifest.digest),
                byte_len: manifest.byte_len,
                materialized: BlobMaterialization::Reused,
            },
            commit: ProjectReceipt {
                revision: controller.live_revision().0,
                import_id: manifest.import_id,
                asset_id: applied.asset_id,
                source_id: applied.source_id,
            },
            reused: true,
        });
    }

    // ---- stage: copy local file → non-RT staging ----------------------
    let staged = stage_dir.join("source.bin");
    if !staged.exists() || manifest.digest.is_empty() {
        let bytes = fs::read(&request.path).map_err(|e| ImportError::StagingIo {
            detail: e.to_string(),
        })?;
        {
            let mut out = File::create(&staged).map_err(|e| ImportError::StagingIo {
                detail: e.to_string(),
            })?;
            out.write_all(&bytes).map_err(|e| ImportError::StagingIo {
                detail: e.to_string(),
            })?;
            out.sync_all().map_err(|e| ImportError::StagingIo {
                detail: e.to_string(),
            })?;
        }
        manifest.digest = content_digest(&bytes);
        manifest.byte_len = bytes.len() as u64;
        write_manifest(&stage_dir, &manifest)?;
    }
    let staged_bytes = fs::read(&staged).map_err(|e| ImportError::StagingIo {
        detail: e.to_string(),
    })?;
    if content_digest(&staged_bytes) != manifest.digest {
        return Err(ImportError::Corrupt {
            detail: "staged copy digest drifted".into(),
        });
    }

    // ---- stage: verify (container/codec/frame/channel) ----------------
    let media = if manifest.stage == ImportStage::Staged {
        let demuxer = ruviform_io_symphonia::open_demuxer(File::open(&staged).map_err(|e| {
            ImportError::StagingIo {
                detail: e.to_string(),
            }
        })?)
        .map_err(|e| ImportError::Corrupt {
            detail: e.to_string(),
        })?;
        let stream = demuxer
            .streams()
            .iter()
            .find(|s| s.stream.kind == StreamKind::Audio)
            .ok_or_else(|| ImportError::UnsupportedCodec {
                detail: "no audio stream".into(),
            })?
            .clone();
        if stream.codec.codec != CodecId::Pcm {
            return Err(ImportError::UnsupportedCodec {
                detail: format!("codec {:?} is not PCM", stream.codec.codec),
            });
        }
        if stream.facts.decodable != Fact::known(()) {
            return Err(ImportError::UnsupportedCodec {
                detail: format!("decodable {:?}", stream.facts.decodable),
            });
        }
        let stream_id = stream.stream.id;
        let mut reader = SourceReader::open(Box::new(demuxer), stream_id, DecodeMode::Canonical)
            .map_err(|e| ImportError::Corrupt {
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
            let out = reader
                .read_planar(&mut views, 4096)
                .map_err(|e| ImportError::Corrupt {
                    detail: e.to_string(),
                })?;
            frames += out.frames as u64;
            if out.eof {
                break;
            }
        }
        let CodecFormat::Audio(_) = stream.codec.format else {
            return Err(ImportError::UnsupportedCodec {
                detail: "decoded format is not audio".into(),
            });
        };
        let media = VerifiedMedia {
            digest: manifest.digest.clone(),
            byte_len: manifest.byte_len,
            frames,
            channels: canonical.layout.channels(),
            sample_rate: canonical.sample_rate.get(),
            declared_frames: declared,
            container: demuxer_container_label(&reader),
            codec: "pcm".into(),
            durability: String::new(),
        };
        manifest.frames = frames;
        manifest.channels = media.channels;
        manifest.sample_rate = media.sample_rate;
        manifest.stage = ImportStage::Verified;
        write_manifest(&stage_dir, &manifest)?;
        media
    } else {
        VerifiedMedia {
            digest: manifest.digest.clone(),
            byte_len: manifest.byte_len,
            frames: manifest.frames,
            channels: manifest.channels,
            sample_rate: manifest.sample_rate,
            declared_frames: None,
            container: "wav".into(),
            codec: "pcm".into(),
            durability: String::new(),
        }
    };

    // ---- stage: measured OS durability --------------------------------
    let mut media = media;
    media.durability = format!(
        "staging={:?} media={:?}",
        detect_dir_sync(&stage_dir),
        detect_dir_sync(&request.media_root)
    );

    // ---- stage: verified blob materialization -------------------------
    let blob = blob_path(&request.media_root, &manifest.digest);
    if manifest.stage == ImportStage::Verified {
        fs::create_dir_all(blob.parent().unwrap_or(&request.media_root)).map_err(|e| {
            ImportError::StagingIo {
                detail: e.to_string(),
            }
        })?;
    }
    let materialized = if blob.exists() {
        let existing = fs::read(&blob).map_err(|e| ImportError::StagingIo {
            detail: e.to_string(),
        })?;
        if content_digest(&existing) != manifest.digest {
            return Err(ImportError::Corrupt {
                detail: format!("existing blob {} digest mismatch", manifest.digest),
            });
        }
        BlobMaterialization::Reused
    } else {
        publish_atomic(&blob, &staged_bytes).map_err(|e| ImportError::StagingIo {
            detail: e.to_string(),
        })?;
        // Directory sync only where the measured capability allows it —
        // on Windows a dir cannot be `File::open`ed for sync, so the
        // recorded durability receipt reflects the real guarantee.
        let dir_synced = blob
            .parent()
            .is_some_and(|parent| detect_dir_sync(parent) == DirSyncCapability::Supported);
        if dir_synced {
            sync_parent_dir(&blob).map_err(|e| ImportError::StagingIo {
                detail: e.to_string(),
            })?;
        }
        BlobMaterialization::Created
    };
    if manifest.stage < ImportStage::BlobPublished {
        manifest.stage = ImportStage::BlobPublished;
        write_manifest(&stage_dir, &manifest)?;
    }

    // ---- stage: expected-revision project mutation --------------------
    let op = CommitImport {
        import_id: manifest.import_id,
        asset_id: manifest.asset_id,
        source_id: manifest.source_id,
        content_hash: manifest.digest.clone(),
    };
    let revision = controller
        .apply(Command {
            expected_revision: controller.live_revision(),
            op,
        })
        .map_err(|e| ImportError::Orphaned {
            digest: manifest.digest.clone(),
            detail: e.to_string(),
        })?;
    manifest.stage = ImportStage::Committed;
    write_manifest(&stage_dir, &manifest)?;

    Ok(ImportOutcome {
        media,
        blob: BlobReceipt {
            digest: manifest.digest,
            path: blob,
            byte_len: manifest.byte_len,
            materialized,
        },
        commit: ProjectReceipt {
            revision: revision.0,
            import_id: manifest.import_id,
            asset_id: manifest.asset_id,
            source_id: manifest.source_id,
        },
        reused: false,
    })
}

fn demuxer_container_label(reader: &SourceReader) -> String {
    reader.metadata().container.format.name().to_owned()
}
