//! gRPC adapter for Envoy's External Authorization v3 API, used by
//! AgentGateway to authorize external tool calls.

#[allow(clippy::all)]
mod pb {
    include!(concat!(env!("OUT_DIR"), "/_includes.rs"));
}

use pb::envoy::service::auth::v3::{
    CheckRequest, CheckResponse, DeniedHttpResponse,
    authorization_server::{Authorization, AuthorizationServer},
};
use pb::envoy::r#type::v3::{HttpStatus, StatusCode};
use pb::google::rpc::Status as RpcStatus;
use serde_json::Value;
use tonic::{Code, Request, Response, Status};

use crate::decision::{self, Action, Decision};

/// JSON-RPC method that runs an MCP tool.
const TOOLS_CALL: &str = "tools/call";

#[derive(Debug, Default)]
pub struct ExtAuthz;

#[tonic::async_trait]
impl Authorization for ExtAuthz {
    /// Denies MCP `tools/call` and allows every other request.
    async fn check(
        &self,
        request: Request<CheckRequest>,
    ) -> Result<Response<CheckResponse>, Status> {
        let (action, label) = classify(&request.into_inner());
        let decision = decision::evaluate(action);
        let verdict = match decision {
            Decision::Allow => "ALLOW",
            Decision::Deny { .. } => "DENY",
        };
        println!("extAuthz request received ({label}) -> {verdict}");
        Ok(Response::new(to_check_response(decision)))
    }
}

pub fn service() -> AuthorizationServer<ExtAuthz> {
    AuthorizationServer::new(ExtAuthz)
}

/// Works out from the forwarded HTTP body whether this request runs a tool.
///
/// Returns the action plus a short label for the log line. A body that is not
/// JSON cannot be classified and is treated as a tool call (fail closed).
fn classify(request: &CheckRequest) -> (Action, String) {
    let http = request
        .attributes
        .as_ref()
        .and_then(|a| a.request.as_ref())
        .and_then(|r| r.http.as_ref());
    let Some(http) = http else {
        return (Action::Other, "no http attributes".to_string());
    };

    let body = if http.raw_body.is_empty() {
        http.body.as_bytes()
    } else {
        &http.raw_body
    };
    if body.is_empty() {
        return (Action::Other, format!("{} {}", http.method, http.path));
    }

    let Ok(json) = serde_json::from_slice::<Value>(body) else {
        return (Action::ToolCall, "unparseable body".to_string());
    };
    // A JSON-RPC batch is a tool call if any of its messages is.
    let messages = match &json {
        Value::Array(batch) => batch.iter().collect(),
        single => vec![single],
    };
    let methods: Vec<&str> = messages
        .iter()
        .filter_map(|m| m.get("method").and_then(Value::as_str))
        .collect();

    let action = if methods.contains(&TOOLS_CALL) {
        Action::ToolCall
    } else {
        Action::Other
    };
    let label = if methods.is_empty() {
        "no json-rpc method".to_string()
    } else {
        methods.join(",")
    };
    (action, label)
}

/// Converts a [`Decision`] into the Envoy response Envoy/AgentGateway expects.
fn to_check_response(decision: Decision) -> CheckResponse {
    match decision {
        Decision::Allow => CheckResponse {
            status: Some(RpcStatus {
                code: Code::Ok as i32,
                message: String::new(),
            }),
            denied_response: None,
        },
        Decision::Deny { reason } => CheckResponse {
            status: Some(RpcStatus {
                code: Code::PermissionDenied as i32,
                message: reason.clone(),
            }),
            denied_response: Some(DeniedHttpResponse {
                status: Some(HttpStatus {
                    code: StatusCode::Forbidden as i32,
                }),
                body: reason,
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pb::envoy::service::auth::v3::AttributeContext;
    use pb::envoy::service::auth::v3::attribute_context::{HttpRequest, Request as AttrRequest};

    fn check_request(body: &str) -> CheckRequest {
        CheckRequest {
            attributes: Some(AttributeContext {
                request: Some(AttrRequest {
                    http: Some(HttpRequest {
                        method: "POST".to_string(),
                        path: "/mcp".to_string(),
                        body: body.to_string(),
                        raw_body: Vec::new(),
                    }),
                }),
            }),
        }
    }

    async fn code_for(body: &str) -> i32 {
        ExtAuthz
            .check(Request::new(check_request(body)))
            .await
            .expect("check response")
            .into_inner()
            .status
            .expect("rpc status")
            .code
    }

    #[test]
    fn deny_maps_to_permission_denied_and_403() {
        let response = to_check_response(decision::evaluate(Action::ToolCall));

        let status = response.status.expect("rpc status");
        assert_eq!(status.code, Code::PermissionDenied as i32);
        assert_eq!(status.message, decision::BLOCK_REASON);

        let denied = response.denied_response.expect("denied response");
        assert_eq!(
            denied.status.expect("http status").code,
            StatusCode::Forbidden as i32
        );
        assert_eq!(denied.body, decision::BLOCK_REASON);
    }

    #[tokio::test]
    async fn tools_call_is_denied() {
        let body = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_me"}}"#;
        assert_eq!(code_for(body).await, Code::PermissionDenied as i32);
    }

    #[tokio::test]
    async fn tools_call_inside_a_batch_is_denied() {
        let body = r#"[{"jsonrpc":"2.0","method":"notifications/initialized"},
                      {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{}}]"#;
        assert_eq!(code_for(body).await, Code::PermissionDenied as i32);
    }

    #[tokio::test]
    async fn handshake_and_listing_are_allowed() {
        for method in ["initialize", "notifications/initialized", "tools/list"] {
            let body = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}"}}"#);
            assert_eq!(code_for(&body).await, Code::Ok as i32, "{method}");
        }
    }

    #[tokio::test]
    async fn bodyless_requests_are_allowed() {
        assert_eq!(code_for("").await, Code::Ok as i32);
        assert_eq!(
            ExtAuthz
                .check(Request::new(CheckRequest::default()))
                .await
                .unwrap()
                .into_inner()
                .status
                .unwrap()
                .code,
            Code::Ok as i32
        );
    }

    #[tokio::test]
    async fn unparseable_body_is_denied() {
        assert_eq!(code_for("not json").await, Code::PermissionDenied as i32);
    }
}
