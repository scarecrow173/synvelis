//! Recording session lifecycle rules.
//!
//! Legal transitions: `Created → Open → Closed → Finalized` only.
//! Registration commands are accepted exclusively in `Open`.
//! `MarkTakeVerified` is accepted in `Open` and `Closed` (media
//! verification is a separate stage from registration).

use synvelis_project::{Error, SessionState};

/// Returns `true` when `from -> to` is a legal session transition.
#[must_use]
pub fn legal_transition(from: SessionState, to: SessionState) -> bool {
    matches!(
        (from, to),
        (SessionState::Created, SessionState::Open)
            | (SessionState::Open, SessionState::Closed)
            | (SessionState::Closed, SessionState::Finalized)
    )
}

/// Returns `true` when registration commands (pass/stream/segment/take)
/// may be applied in `state`.
#[must_use]
pub fn accepts_registration(state: SessionState) -> bool {
    state == SessionState::Open
}

/// Returns `true` when `MarkTakeVerified` may be applied in `state`.
#[must_use]
pub fn accepts_take_verification(state: SessionState) -> bool {
    matches!(state, SessionState::Open | SessionState::Closed)
}

/// Rejects `from -> to` when not legal.
///
/// # Errors
/// [`Error::IllegalTransition`].
pub fn require_transition(from: SessionState, to: SessionState) -> Result<(), Error> {
    if legal_transition(from, to) {
        Ok(())
    } else {
        Err(Error::IllegalTransition(
            illegal_transition_message(from, to).into(),
        ))
    }
}

fn illegal_transition_message(from: SessionState, to: SessionState) -> &'static str {
    match (from, to) {
        (SessionState::Created, SessionState::Created) => "Created -> Created",
        (SessionState::Created, SessionState::Open) => "Created -> Open",
        (SessionState::Created, SessionState::Closed) => "Created -> Closed",
        (SessionState::Created, SessionState::Finalized) => "Created -> Finalized",
        (SessionState::Open, SessionState::Created) => "Open -> Created",
        (SessionState::Open, SessionState::Open) => "Open -> Open",
        (SessionState::Open, SessionState::Closed) => "Open -> Closed",
        (SessionState::Open, SessionState::Finalized) => "Open -> Finalized",
        (SessionState::Closed, SessionState::Created) => "Closed -> Created",
        (SessionState::Closed, SessionState::Open) => "Closed -> Open",
        (SessionState::Closed, SessionState::Closed) => "Closed -> Closed",
        (SessionState::Closed, SessionState::Finalized) => "Closed -> Finalized",
        (SessionState::Finalized, SessionState::Created) => "Finalized -> Created",
        (SessionState::Finalized, SessionState::Open) => "Finalized -> Open",
        (SessionState::Finalized, SessionState::Closed) => "Finalized -> Closed",
        (SessionState::Finalized, SessionState::Finalized) => "Finalized -> Finalized",
    }
}
