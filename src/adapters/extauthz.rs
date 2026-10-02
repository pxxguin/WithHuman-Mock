//! gRPC adapter for Envoy's External Authorization v3 API, used by
//! AgentGateway to authorize external tool calls.

#[allow(clippy::all)]
mod pb {
    include!(concat!(env!("OUT_DIR"), "/_includes.rs"));
}

use pb::envoy::r#type::v3::{HttpStatus, StatusCode};
use pb::envoy::service::auth::v3::{
    CheckRequest, CheckResponse, DeniedHttpResponse,
    authorization_server::{Authorization, AuthorizationServer},
};
use pb::google::rpc::Status as RpcStatus;
use tonic::{Code, Request, Response, Status};

use crate::decision::{self, Decision};

#[derive(Debug, Default)]
pub struct ExtAuthz;

#[tonic::async_trait]
impl Authorization for ExtAuthz {
    /// Answers every `Check` call without inspecting it.
    async fn check(
        &self,
        _request: Request<CheckRequest>,
    ) -> Result<Response<CheckResponse>, Status> {
        println!("extAuthz request received -> DENY");
        Ok(Response::new(to_check_response(decision::evaluate())))
    }
}

pub fn service() -> AuthorizationServer<ExtAuthz> {
    AuthorizationServer::new(ExtAuthz)
}

/// Converts a [`Decision`] into the Envoy response Envoy/AgentGateway expects.
fn to_check_response(decision: Decision) -> CheckResponse {
    match decision {
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

    #[test]
    fn deny_maps_to_permission_denied_and_403() {
        let response = to_check_response(decision::evaluate());

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
    async fn check_denies() {
        let response = ExtAuthz
            .check(Request::new(CheckRequest {}))
            .await
            .expect("check response");

        assert_eq!(
            response.into_inner().status.expect("rpc status").code,
            Code::PermissionDenied as i32
        );
    }
}
