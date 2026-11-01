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
        Err(Error::IllegalTransition(format!("{from:?} -> {to:?}")))
    }
}
