//! `synvelis-player` — the L1 player slice: safe local WAV import,
//! verified media resolution, timeline lowering onto the generic
//! Ruviform graph, and a deterministic headless playback driver.
//!
//! Layering: this crate owns the player-domain pipeline only. Project
//! mutations go through [`synvelis_project::ProjectController`] as
//! validated [`synvelis_project::Operation`]s; media verification,
//! decoding, prefetching and the audio runtime are Ruviform services.
//! No Project/Track/Clip type crosses the graph boundary — lowering
//! emits only stable node/port IDs plus plain audio formats.
//!
//! Pipeline (Issue #24 contract):
//!
//! ```text
//! local path → non-RT staging → size/hash/container/codec/frame/channel
//!   verify → measured OS durability → verified blob materialization →
//!   expected-revision project mutation → save/reload
//! ```

pub mod driver;
pub mod error;
pub mod fixture;
pub mod import;
pub mod lowering;
pub mod processor;
pub mod resolver;

pub use driver::{DriverState, HeadlessDriver, PlayReport, PumpPolicy};
pub use error::{ImportError, ResolveError};
pub use import::{BlobReceipt, ImportOutcome, ImportRequest, ProjectReceipt, VerifiedMedia};
pub use lowering::{ClipBinding, ClipWindow, EngineConfig, Lowered, SkippedClip};
pub use processor::{ClipPlayer, PlaybackStats};
pub use resolver::PreparedClip;
