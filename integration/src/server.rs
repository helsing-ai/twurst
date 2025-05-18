use crate::proto::{test_request, test_response, IntegrationService, TestRequest, TestResponse};
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::Router;
use eyre::Result;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::pin::pin;
use tokio::net::TcpListener;
use tokio::task::{spawn, JoinHandle};
use tokio_stream::{Stream, StreamExt};
use tower_http::cors::{Any, CorsLayer};
use twurst_server::TwirpError;

pub struct IntegrationServiceServicer {}

impl IntegrationService for IntegrationServiceServicer {
    async fn test(
        &self,
        request: TestRequest,
        ExtractBearerToken(bearer_token): ExtractBearerToken,
    ) -> Result<TestResponse, TwirpError> {
        if bearer_token != "password" {
            return Err(TwirpError::unauthenticated("Invalid password"));
        }
        Ok(response_from_request(request))
    }

    async fn test_server_stream(
        &self,
        request: TestRequest,
        ExtractBearerToken(bearer_token): ExtractBearerToken,
    ) -> Result<Box<dyn Stream<Item = Result<TestResponse, TwirpError>> + Send>, TwirpError> {
        if bearer_token != "password" {
            return Err(TwirpError::unauthenticated("Invalid password"));
        }
        Ok(Box::new(tokio_stream::iter([
            Ok(response_from_request(request)),
            Err(TwirpError::not_found("foo")),
        ])))
    }

    async fn test_client_stream(
        &self,
        request: impl Stream<Item = Result<TestRequest, TwirpError>> + Send,
        ExtractBearerToken(bearer_token): ExtractBearerToken,
    ) -> Result<TestResponse, TwirpError> {
        if bearer_token != "password" {
            return Err(TwirpError::unauthenticated("Invalid password"));
        }
        let request = pin!(request)
            .next()
            .await
            .ok_or_else(|| TwirpError::not_found("No request"))??;
        Ok(response_from_request(request))
    }

    async fn test_stream(
        &self,
        request: impl Stream<Item = Result<TestRequest, TwirpError>> + Send + 'static,
        ExtractBearerToken(bearer_token): ExtractBearerToken,
    ) -> Result<Box<dyn Stream<Item = Result<TestResponse, TwirpError>> + Send>, TwirpError> {
        if bearer_token != "password" {
            return Err(TwirpError::unauthenticated("Invalid password"));
        }
        Ok(Box::new(
            request.map(|request| Ok(response_from_request(request?))),
        ))
    }
}

fn response_from_request(request: TestRequest) -> TestResponse {
    TestResponse {
        string: request.string,
        time: request.time,
        nested: request.nested,
        duration: request.duration,
        any: request.any,
        value: request.value,
        option: request.option.map(|o| match o {
            test_request::Option::Left(l) => test_response::Option::Left(l),
            test_request::Option::Right(r) => test_response::Option::Right(r),
        }),
    }
}

pub struct Server {
    url: String,
    task: JoinHandle<()>,
}

impl Server {
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn serve_twirp() -> Result<Server> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).await?;
    let url = format!("http://{}/twirp", listener.local_addr()?);
    let task = spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .nest("/twirp", IntegrationServiceServicer {}.into_router())
                .layer(
                    CorsLayer::new()
                        .allow_methods(Any)
                        .allow_origin(Any)
                        .allow_headers(Any),
                ),
        )
        .await
        .unwrap();
    });
    Ok(Server { url, task })
}

pub async fn serve_grpc() -> Result<Server> {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).await?;
    let url = format!("http://{}", listener.local_addr()?);
    let task = spawn(async move {
        axum::serve(listener, IntegrationServiceServicer {}.into_grpc_router())
            .await
            .unwrap();
    });
    Ok(Server { url, task })
}

pub struct ExtractBearerToken(pub String);

impl<S> FromRequestParts<S> for ExtractBearerToken
where
    S: Send + Sync,
{
    type Rejection = TwirpError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Some(authorization) = parts.headers.get(AUTHORIZATION) else {
            return Err(TwirpError::unauthenticated(
                "Authorization header is required",
            ));
        };
        let Some(token) = authorization
            .to_str()
            .map_err(|_| TwirpError::malformed("Authorization header must be valid UTF-8"))?
            .strip_prefix("Bearer ")
        else {
            return Err(TwirpError::malformed(
                "The authorization header must start with `Bearer `",
            ));
        };
        Ok(Self(token.into()))
    }
}
