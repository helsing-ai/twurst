use eyre::Result;
use std::time::{Duration, SystemTime};
use tower::ServiceBuilder;
use tower_http::auth::AddAuthorizationLayer;
use twurst_client::TwirpHttpClient;
use twurst_integration::client::{Choice, Data, IntegrationClient};
use twurst_integration::proto::{IntegrationService, IntegrationServiceClient};
use twurst_integration::server;
use twurst_integration::server::IntegrationServiceServicer;
use twurst_server::TwirpError;

#[tokio::test]
async fn test_simple_twirp_echo_protobuf() -> Result<()> {
    let server = server::serve_twirp().await?;
    let data = example_data();
    let client = IntegrationClient::new(server.url(), false);
    let response = client.test(data.clone()).await?;
    assert_eq!(response, data);
    Ok(())
}

#[tokio::test]
async fn test_simple_twirp_echo_json() -> Result<()> {
    let server = server::serve_twirp().await?;
    let data = example_data();
    let client = IntegrationClient::new(server.url(), true);
    let response = client.test(data.clone()).await?;
    assert_eq!(response, data);
    Ok(())
}

#[tokio::test]
async fn test_without_networking() -> Result<()> {
    let data = example_data();
    let client = IntegrationServiceClient::new(TwirpHttpClient::new(
        ServiceBuilder::new()
            .layer(AddAuthorizationLayer::bearer("password"))
            .service(IntegrationServiceServicer {}.into_router()),
    ));
    let response = Data::try_from(client.test(&data.clone().try_into()?).await?)?;
    assert_eq!(response, data);
    Ok(())
}

#[tokio::test]
async fn test_no_auth_header() -> Result<()> {
    let data = example_data();
    let client = IntegrationServiceClient::new(TwirpHttpClient::new(
        IntegrationServiceServicer {}.into_router(),
    ));
    let error = client.test(&data.clone().try_into()?).await.unwrap_err();
    assert_eq!(
        error,
        TwirpError::unauthenticated("Authorization header is required")
    );
    Ok(())
}

#[tokio::test]
async fn test_wrong_password() -> Result<()> {
    let data = example_data();
    let client = IntegrationServiceClient::new(TwirpHttpClient::new(
        ServiceBuilder::new()
            .layer(AddAuthorizationLayer::bearer("foo"))
            .service(IntegrationServiceServicer {}.into_router()),
    ));
    let error = client.test(&data.clone().try_into()?).await.unwrap_err();
    assert_eq!(error, TwirpError::unauthenticated("Invalid password"));
    Ok(())
}

fn example_data() -> Data {
    Data {
        string: "test_simple_twirp_echo".to_string(),
        time: SystemTime::now(),
        choice: Choice::X,
        duration: Duration::from_micros(1223454355),
        any: 123,
        option: 34.6,
        value: "foo".to_string(),
    }
}
