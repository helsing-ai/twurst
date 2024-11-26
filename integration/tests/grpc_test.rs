use crate::proto::integration_service_client::IntegrationServiceClient;
use crate::proto::test_nested::NestedMessage;
use crate::proto::{test_request, test_response, TestNested, TestRequest, TestResponse};
use eyre::Result;
use prost_types::Value;
use std::time::{Duration, UNIX_EPOCH};
use tonic::{Code, Request};
use twurst_integration::server;

mod proto {
    tonic::include_proto!("tonic/integration");
}

#[tokio::test]
async fn test_simple_grpc_echo() -> Result<()> {
    let server = server::serve_grpc().await?;
    let mut client = IntegrationServiceClient::connect(server.url().to_string()).await?;
    let mut request = Request::new(TestRequest {
        string: "test_simple_grpc_echo".to_string(),
        time: Some(UNIX_EPOCH.into()),
        nested: Some(TestNested {
            field0: Some(NestedMessage { r#enum: 1 }),
        }),
        duration: Some(Duration::from_micros(1223454355).try_into()?),
        any: None,
        option: Some(test_request::Option::Right(1.2)),
        value: Some(Value::from("foo".to_string())),
    });
    request
        .metadata_mut()
        .insert("authorization", "Bearer password".parse()?);
    let response = client.test(request).await?.into_inner();
    assert_eq!(
        response,
        TestResponse {
            string: "test_simple_grpc_echo".to_string(),
            time: Some(UNIX_EPOCH.into()),
            nested: Some(TestNested {
                field0: Some(NestedMessage { r#enum: 1 }),
            }),
            duration: Some(Duration::from_micros(1223454355).try_into()?),
            any: None,
            option: Some(test_response::Option::Right(1.2)),
            value: Some(Value::from("foo".to_string())),
        }
    );
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
