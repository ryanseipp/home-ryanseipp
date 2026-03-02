use tonic::{Request, Response, Status};

use crate::proto::{
    GetJwksRequest, GetJwksResponse, LoginRequest, LoginResponse, SignUpRequest, SignUpResponse,
    UserInfoRequest, UserInfoResponse, identity_service_server::IdentityService,
};

/// Application state shared across all RPC handlers.
///
/// Will hold KeyStore, Kek, database pool, etc. as the service grows.
pub struct IdentityServiceImpl {}

impl IdentityServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}

#[tonic::async_trait]
impl IdentityService for IdentityServiceImpl {
    async fn sign_up(
        &self,
        _request: Request<SignUpRequest>,
    ) -> Result<Response<SignUpResponse>, Status> {
        Err(Status::unimplemented("sign_up not yet implemented"))
    }

    async fn login(
        &self,
        _request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        Err(Status::unimplemented("login not yet implemented"))
    }

    async fn user_info(
        &self,
        _request: Request<UserInfoRequest>,
    ) -> Result<Response<UserInfoResponse>, Status> {
        Err(Status::unimplemented("user_info not yet implemented"))
    }

    async fn get_jwks(
        &self,
        _request: Request<GetJwksRequest>,
    ) -> Result<Response<GetJwksResponse>, Status> {
        Err(Status::unimplemented("get_jwks not yet implemented"))
    }
}
