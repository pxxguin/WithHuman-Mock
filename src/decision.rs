//! Shared decision model used by every adapter.
//!
//! The PoC has no policy engine: [`evaluate`] always denies. Both the gRPC
//! extAuthz adapter and the HTTP hook adapter call it so the blocking
//! behaviour lives in exactly one place.

/// Reason attached to every denial.
pub const BLOCK_REASON: &str = "Blocked by WithHuman";

/// Outcome of evaluating a tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Deny { reason: String },
}

/// Evaluate a tool call.
///
/// Always returns [`Decision::Deny`] for now.
pub fn evaluate() -> Decision {
    Decision::Deny {
        reason: BLOCK_REASON.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_always_denies() {
        assert_eq!(
            evaluate(),
            Decision::Deny {
                reason: BLOCK_REASON.to_string()
            }
        );
    }
}
