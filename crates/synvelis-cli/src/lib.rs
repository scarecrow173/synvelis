//! `synvelis-cli` — the headless, GUI-free command line entry point
//! (AUTX-03 / synvelis#262).
//!
//! This crate is a *thin adapter*: every operation flows through the
//! existing `synvelis-gateway` boundary — `HeadlessSessionHost` +
//! `LocalCliAdapter` for trust, `Gateway::handle` for parse → bind →
//! invoke, `CommitGate`/`DirectoryBundleStore`/`FilePolicyStore` for
//! the single-writer project and durable policy seams. It owns no
//! second registry, authorization engine, project writer, job
//! executor, or workflow run-state authority.
//!
//! Module map:
//!
//! - [`args`] — hand-rolled argument parser (no CLI framework).
//! - [`error`] — CLI-originated typed errors (`cli.*` codes).
//! - [`envelope`] — the versioned `CliEnvelope` stdout contract.
//! - [`exitcode`] — deterministic outcome/error → exit-code mapping.
//! - [`script`] — `--file` extension/kind/language/backend
//!   fail-closed classification (AUTX-CLI-FILE-01).
//! - [`host`] — gateway/session/project boot composition.
//! - [`commands`] — command implementations.

pub mod args;
pub mod commands;
pub mod envelope;
pub mod error;
pub mod exitcode;
pub mod host;
pub mod script;
