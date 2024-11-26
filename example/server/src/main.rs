use crate::proto::{ExampleService, TestRequest, TestResponse};
use axum::http::HeaderMap;
use axum::Router;
use std::error::Error;
use tokio::join;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use twurst_server::TwirpError;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/example.rs"));
}

pub struct ExampleServiceServicer {}

impl ExampleService for ExampleServiceServicer {
    async fn test(
        &self,
        request: TestRequest,
        _headers: HeaderMap, // We have access to the headers because we customize the build in build.rs
    ) -> Result<TestResponse, TwirpError> {
        Ok(TestResponse {
            string: request.string,
            time: request.time,
        })
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    // We expose Twirp with CORS enabled
    let twirp_service = axum::serve(
        TcpListener::bind("localhost:8080").await?,
        Router::new()
            .nest("/twirp", ExampleServiceServicer {}.into_router())
            .layer(
                CorsLayer::new()
                    .allow_methods(Any)
                    .allow_origin(Any)
                    .allow_headers(Any),
            ),
    );

    // We expose also gRPC for gRPC-only clients on another port
    let grpc_service = axum::serve(
        TcpListener::bind("localhost:8081").await?,
        ExampleServiceServicer {}.into_grpc_router(),
    );

    let (twirp_result, grpc_result) = join!(twirp_service, grpc_service);
    twirp_result?;
    grpc_result?;
    Ok(())
}
