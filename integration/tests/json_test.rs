use axum::body::Bytes;
use axum::http::Request;
use axum::response::IntoResponse;
use eyre::Result;
use std::str;
use std::time::{Duration, UNIX_EPOCH};
use tower::service_fn;
use twurst_client::{TwirpHttpClient, TwirpRequestBody};
use twurst_integration::client::{Choice, Data};
use twurst_integration::proto::IntegrationServiceClient;
use twurst_server::TwirpError;

#[tokio::test]
async fn test_json_serialization() -> Result<()> {
    let expected_json = "{\"string\":\"test_simple_twirp_echo\",\"time\":\"1970-01-01T00:00:00Z\",\"nested\":{\"field0\":{\"enum\":\"X\"}},\"right\":1.2,\"duration\":\"1223.454355s\",\"any\":{\"@type\":\"type.googleapis.com/integration.Int\",\"value\":\"42\"},\"value\":\"foo\"}";
    let mut client = TwirpHttpClient::new(service_fn(
        move |request: Request<TwirpRequestBody>| async move {
            assert_eq!(
                str::from_utf8(&Bytes::from(request.into_body())).unwrap(),
                expected_json
            );
            Ok::<_, TwirpError>(TwirpError::unimplemented("").into_response())
        },
    ));
    client.use_json();
    let error = IntegrationServiceClient::new(client)
        .test(
            &Data {
                string: "test_simple_twirp_echo".into(),
                time: UNIX_EPOCH,
                choice: Choice::X,
                duration: Duration::from_micros(1223454355),
                any: 42,
                option: 1.2,
                value: "foo".into(),
            }
            .try_into()?,
        )
        .await
        .unwrap_err();
    assert_eq!(error, TwirpError::unimplemented(""));
    Ok(())
}
