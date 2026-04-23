//! Summary protection — runtime guard for automatic summary updates.
//!
//! `Chunk.edited_by_user` and `Chunk.auto_update_locked` are UX flags that the
//! auto-summarizer must respect. `apply_summary_update` encodes the decision:
//!
//! - `auto_update_locked = true`: never overwrite.
//! - `edited_by_user = true`: preserve by default, but allow when the caller
//!   opts in (`force_overwrite_user_edit`). Useful when the user resets via
//!   an explicit "regenerate" action.
//! - Otherwise: apply the new summary.
//!
//! The function returns an `SummaryUpdateOutcome` so callers can log or
//! surface why an update was skipped.

use crate::domain::{Chunk, SummaryMethod};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummaryUpdateOutcome {
    Applied,
    SkippedLocked,
    SkippedUserEdited,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SummaryUpdate {
    /// Force-override the user-edit guard. `auto_update_locked` is still
    /// respected regardless.
    pub force_overwrite_user_edit: bool,
}

impl SummaryUpdate {
    pub fn forced() -> Self {
        Self {
            force_overwrite_user_edit: true,
        }
    }
}

/// Apply a new summary to `chunk` respecting the protection flags. Returns
/// `Applied` when the write happened, `SkippedLocked` / `SkippedUserEdited`
/// otherwise.
pub fn apply_summary_update(
    chunk: &mut Chunk,
    new_summary: impl Into<String>,
    new_method: SummaryMethod,
    options: SummaryUpdate,
) -> SummaryUpdateOutcome {
    if chunk.auto_update_locked {
        return SummaryUpdateOutcome::SkippedLocked;
    }
    if chunk.edited_by_user && !options.force_overwrite_user_edit {
        return SummaryUpdateOutcome::SkippedUserEdited;
    }
    chunk.summary = new_summary.into();
    chunk.summary_method = new_method;
    if options.force_overwrite_user_edit {
        // Force-overwrite resets the user-edit flag so subsequent auto-updates
        // resume normally.
        chunk.edited_by_user = false;
    }
    SummaryUpdateOutcome::Applied
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Chunk {
        let mut c = Chunk::raw_leaf("x");
        c.summary_level = 1;
        c.summary = "old".into();
        c.summary_method = SummaryMethod::ExtractiveBM25;
        c
    }

    #[test]
    fn applies_when_unflagged() {
        let mut c = fresh();
        let outcome = apply_summary_update(
            &mut c,
            "new",
            SummaryMethod::LlmFull,
            SummaryUpdate::default(),
        );
        assert_eq!(outcome, SummaryUpdateOutcome::Applied);
        assert_eq!(c.summary, "new");
        assert_eq!(c.summary_method, SummaryMethod::LlmFull);
    }

    #[test]
    fn skips_when_locked() {
        let mut c = fresh();
        c.auto_update_locked = true;
        let outcome = apply_summary_update(
            &mut c,
            "new",
            SummaryMethod::LlmFull,
            SummaryUpdate::forced(),
        );
        assert_eq!(outcome, SummaryUpdateOutcome::SkippedLocked);
        assert_eq!(c.summary, "old");
    }

    #[test]
    fn skips_when_user_edited() {
        let mut c = fresh();
        c.edited_by_user = true;
        let outcome = apply_summary_update(
            &mut c,
            "new",
            SummaryMethod::LlmFull,
            SummaryUpdate::default(),
        );
        assert_eq!(outcome, SummaryUpdateOutcome::SkippedUserEdited);
        assert_eq!(c.summary, "old");
    }

    #[test]
    fn force_overwrites_user_edit() {
        let mut c = fresh();
        c.edited_by_user = true;
        let outcome = apply_summary_update(
            &mut c,
            "new",
            SummaryMethod::LlmFull,
            SummaryUpdate::forced(),
        );
        assert_eq!(outcome, SummaryUpdateOutcome::Applied);
        assert_eq!(c.summary, "new");
        assert!(!c.edited_by_user, "forced update should reset the flag");
    }

    #[test]
    fn lock_dominates_force() {
        let mut c = fresh();
        c.auto_update_locked = true;
        c.edited_by_user = true;
        let outcome = apply_summary_update(
            &mut c,
            "new",
            SummaryMethod::LlmFull,
            SummaryUpdate::forced(),
        );
        assert_eq!(outcome, SummaryUpdateOutcome::SkippedLocked);
    }
}
