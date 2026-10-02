//! Shared decision model used by every adapter.
//!
//! The PoC has no policy engine: [`evaluate`] denies every tool call and lets
//! everything else through. Both the gRPC extAuthz adapter and the HTTP hook
//! adapter call it so the blocking behaviour lives in exactly one place.

/// Reason attached to every denial.
pub const BLOCK_REASON: &str = "Blocked by WithHuman";

/// What the agent is trying to do, as far as the adapter can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// A tool is about to run: a `PreToolUse` hook or an MCP `tools/call`.
    ToolCall,
    /// Anything else, such as MCP `initialize` or `tools/list`.
    Other,
}

/// Outcome of evaluating an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny { reason: String },
}

/// Evaluate an action.
///
/// Tool calls are always denied; everything else is allowed so an MCP client
/// can still connect and list tools.
pub fn evaluate(action: Action) -> Decision {
    match action {
        Action::ToolCall => Decision::Deny {
            reason: BLOCK_REASON.to_string(),
        },
        Action::Other => Decision::Allow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_calls_are_denied() {
        assert_eq!(
            evaluate(Action::ToolCall),
            Decision::Deny {
                reason: BLOCK_REASON.to_string()
            }
        );
    }

    #[test]
    fn other_actions_are_allowed() {
        assert_eq!(evaluate(Action::Other), Decision::Allow);
    }
}
