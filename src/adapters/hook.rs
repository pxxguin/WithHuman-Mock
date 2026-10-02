//! HTTP adapter for agent hooks such as Claude Code `PreToolUse`.
//!
//! The response shape is deliberately kept in this module so it can later be
//! swapped for an agent-specific format without touching the decision model.

use axum::{Json, Router, routing::post};
use serde::Serialize;
use serde_json::Value;

use crate::decision::{self, Action, Decision};

/// Internal response model for `POST /v1/hooks/pre-tool`.
#[derive(Debug, Serialize)]
pub struct PreToolResponse {
    pub decision: &'static str,
    pub reason: String,
}

impl From<Decision> for PreToolResponse {
    fn from(decision: Decision) -> Self {
        match decision {
            Decision::Allow => Self {
                decision: "ALLOW",
                reason: String::new(),
            },
            Decision::Deny { reason } => Self {
                decision: "DENY",
                reason,
            },
        }
    }
}

pub fn router() -> Router {
    Router::new().route("/v1/hooks/pre-tool", post(pre_tool))
}

/// Accepts any JSON tool call and denies it: every `PreToolUse` is a tool call.
async fn pre_tool(Json(_tool_call): Json<Value>) -> Json<PreToolResponse> {
    println!("PreToolUse request received -> DENY");
    Json(decision::evaluate(Action::ToolCall).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn pre_tool_denies_any_payload() {
        let request = Request::builder()
            .method("POST")
            .uri("/v1/hooks/pre-tool")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"tool_name":"Bash","anything":[1,2,3]}"#))
            .unwrap();

        let response = router().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["decision"], "DENY");
        assert_eq!(body["reason"], decision::BLOCK_REASON);
    }
}
