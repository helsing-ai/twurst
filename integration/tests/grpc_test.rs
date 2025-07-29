use crate::proto::integration_service_client::IntegrationServiceClient;
use crate::proto::test_nested::NestedMessage;
use crate::proto::{TestNested, TestRequest, TestResponse, test_request, test_response};
use eyre::Result;
use prost_types::Value;
use std::time::{Duration, UNIX_EPOCH};
use tokio_stream::StreamExt;
use tonic::{Code, Request};
use twurst_integration::server;

mod proto {
    tonic::include_proto!("tonic/integration");
}

#[tokio::test]
async fn test_simple_grpc_echo() -> Result<()> {
    let server = server::serve_grpc().await?;
    let mut client = IntegrationServiceClient::connect(server.url().to_string()).await?;
    let mut request = Request::new(dummy_request());
    request
        .metadata_mut()
        .insert("authorization", "Bearer password".parse()?);
    let response = client.test(request).await?.into_inner();
    assert_eq!(response, dummy_response());
    Ok(())
}

#[tokio::test]
async fn test_no_authorization_header() -> Result<()> {
    let server = server::serve_grpc().await?;
    let mut client = IntegrationServiceClient::connect(server.url().to_string()).await?;
    let status = client.test(TestRequest::default()).await.unwrap_err();
    assert_eq!(status.code(), Code::Unauthenticated);
    assert_eq!(status.message(), "Authorization header is required");
    Ok(())
}

#[tokio::test]
async fn test_server_streaming_grpc_echo() -> Result<()> {
    let server = server::serve_grpc().await?;
    let mut client = IntegrationServiceClient::connect(server.url().to_string()).await?;
    let mut request = Request::new(dummy_request());
    request
        .metadata_mut()
        .insert("authorization", "Bearer password".parse()?);
    let response = client
        .test_server_stream(request)
        .await?
        .into_inner()
        .collect::<Vec<_>>()
        .await;
    assert_eq!(response[0].clone()?, dummy_response());
    assert_eq!(response[1].clone().unwrap_err().code(), Code::NotFound);
    Ok(())
}

#[tokio::test]
async fn test_client_streaming_grpc_echo() -> Result<()> {
    let server = server::serve_grpc().await?;
    let mut client = IntegrationServiceClient::connect(server.url().to_string()).await?;
    let mut request = Request::new(tokio_stream::once(dummy_request()));
    request
        .metadata_mut()
        .insert("authorization", "Bearer password".parse()?);
    let response = client.test_client_stream(request).await?.into_inner();
    assert_eq!(response, dummy_response());
    Ok(())
}

#[tokio::test]
async fn test_streaming_grpc_echo() -> Result<()> {
    let server = server::serve_grpc().await?;
    let mut client = IntegrationServiceClient::connect(server.url().to_string()).await?;
    let mut request = Request::new(tokio_stream::once(dummy_request()));
    request
        .metadata_mut()
        .insert("authorization", "Bearer password".parse()?);
    let response = client
        .test_stream(request)
        .await?
        .into_inner()
        .collect::<Vec<_>>()
        .await;
    assert_eq!(response[0].clone()?, dummy_response());
    Ok(())
}

fn dummy_request() -> TestRequest {
    TestRequest {
        string: "test_simple_grpc_echo".to_string(),
        time: Some(UNIX_EPOCH.into()),
        nested: Some(TestNested {
            field0: Some(NestedMessage { r#enum: 1 }),
        }),
        duration: Some(Duration::from_micros(1223454355).try_into().unwrap()),
        any: None,
        option: Some(test_request::Option::Right(1.2)),
        value: Some(Value::from("foo".to_string())),
    }
}

fn dummy_response() -> TestResponse {
    TestResponse {
        string: "test_simple_grpc_echo".to_string(),
        time: Some(UNIX_EPOCH.into()),
        nested: Some(TestNested {
            field0: Some(NestedMessage { r#enum: 1 }),
        }),
        duration: Some(Duration::from_micros(1223454355).try_into().unwrap()),
        any: None,
        option: Some(test_response::Option::Right(1.2)),
        value: Some(Value::from("foo".to_string())),
    }
}
