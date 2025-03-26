#![doc = include_str!("../README.md")]
#![doc(
    test(attr(deny(warnings))),
    html_favicon_url = "https://raw.githubusercontent.com/helsing-ai/twurst/main/docs/img/twurst.png",
    html_logo_url = "https://raw.githubusercontent.com/helsing-ai/twurst/main/docs/img/twurst.png"
)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use http::header::CONTENT_TYPE;
use http::{HeaderValue, Method, Request, Response, StatusCode};
use http_body::{Body, Frame, SizeHint};
use http_body_util::BodyExt;
use prost_reflect::bytes::{Buf, Bytes, BytesMut};
use prost_reflect::{DynamicMessage, ReflectMessage};
use serde::Serialize;
use std::convert::Infallible;
use std::error::Error;
use std::future::poll_fn;
#[cfg(feature = "reqwest-012")]
use std::future::Future;
use std::mem::take;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower_service::Service;
pub use twurst_error::{TwirpError, TwirpErrorCode};

const APPLICATION_JSON: HeaderValue = HeaderValue::from_static("application/json");
const APPLICATION_PROTOBUF: HeaderValue = HeaderValue::from_static("application/protobuf");

/// Underlying client used by autogenerated clients to handle networking.
///
/// Can be constructed with [`TwirpHttpClient::new_using_reqwest_012`] to use [`reqwest 0.12`](reqwest_012)
/// or from a regular [`tower::Service`](Service) using [`TwirpHttpClient::new_with_base`]
/// or [`TwirpHttpClient::new`] if relative URLs are fine.
///
/// URL grammar for twirp service is `URL ::= Base-URL [ Prefix ] "/" [ Package "." ] Service "/" Method`.
/// The `/ [ Package "." ] Service "/" Method` part is auto-generated by the build step
/// but the `Base-URL [ Prefix ]` must be set to do proper call to remote services.
/// This is the `base_url` parameter.
/// If not filled, request URL is only going to be the auto-generated part.
#[derive(Clone)]
pub struct TwirpHttpClient<S: TwirpHttpService> {
    service: S,
    base_url: Option<String>,
    use_json: bool,
}

#[cfg(feature = "reqwest-012")]
impl TwirpHttpClient<Reqwest012Service> {
    /// Builds a new client using [`reqwest 0.12`](reqwest_012).
    ///
    /// Note that `base_url` must be absolute with a scheme like `https://`.
    ///
    /// ```
    /// use twurst_client::TwirpHttpClient;
    ///
    /// let _client = TwirpHttpClient::new_using_reqwest_012("http://example.com/twirp");
    /// ```
    pub fn new_using_reqwest_012(base_url: impl Into<String>) -> Self {
        Self::new_with_reqwest_012_client(reqwest_012::Client::new(), base_url)
    }

    /// Builds a new client using [`reqwest 0.12`](reqwest_012).
    ///
    /// Note that `base_url` must be absolute with a scheme like `https://`.
    ///
    /// ```
    /// # use reqwest_012::Client;
    /// use twurst_client::TwirpHttpClient;
    ///
    /// let _client =
    ///     TwirpHttpClient::new_with_reqwest_012_client(Client::new(), "http://example.com/twirp");
    /// ```
    pub fn new_with_reqwest_012_client(
        client: reqwest_012::Client,
        base_url: impl Into<String>,
    ) -> Self {
        Self::new_with_base(Reqwest012Service(client), base_url)
    }
}

impl<S: TwirpHttpService> TwirpHttpClient<S> {
    /// Builds a new client from a [`tower::Service`](Service) and a base URL to the Twirp endpoint.
    ///
    /// ```
    /// use http::Response;
    /// use std::convert::Infallible;
    /// use twurst_client::TwirpHttpClient;
    /// use twurst_error::TwirpError;
    ///
    /// let _client = TwirpHttpClient::new_with_base(
    ///     tower::service_fn(|_request| async {
    ///         Ok::<Response<String>, Infallible>(TwirpError::unimplemented("not implemented").into())
    ///     }),
    ///     "http://example.com/twirp",
    /// );
    /// ```
    pub fn new_with_base(service: S, base_url: impl Into<String>) -> Self {
        let mut base_url = base_url.into();
        // We remove the last '/' to make concatenation work
        if base_url.ends_with('/') {
            base_url.pop();
        }
        Self {
            service,
            base_url: Some(base_url),
            use_json: false,
        }
    }

    /// New service without base URL. Relative URLs will be used for requests!
    ///
    /// ```
    /// use http::Response;
    /// use std::convert::Infallible;
    /// use twurst_client::TwirpHttpClient;
    /// use twurst_error::TwirpError;
    ///
    /// let _client = TwirpHttpClient::new(tower::service_fn(|_request| async {
    ///     Ok::<Response<String>, Infallible>(TwirpError::unimplemented("not implemented").into())
    /// }));
    /// ```
    pub fn new(service: S) -> Self {
        Self {
            service,
            base_url: None,
            use_json: false,
        }
    }

    /// Use JSON for requests and response instead of binary protobuf encoding that is used by default
    pub fn use_json(&mut self) {
        self.use_json = true;
    }

    /// Use binary protobuf encoding for requests and response (the default)
    pub fn use_binary_protobuf(&mut self) {
        self.use_json = false;
    }

    /// Send a Twirp request and get a response.
    ///
    /// Used internally by the generated code.
    pub async fn call<I: ReflectMessage, O: ReflectMessage + Default>(
        &self,
        path: &str,
        request: &I,
    ) -> Result<O, TwirpError> {
        // We ensure that the service is ready
        self.service.ready().await.map_err(|e| {
            TwirpError::wrap(
                TwirpErrorCode::Unknown,
                format!("Service is not ready: {e}"),
                e,
            )
        })?;
        let request = self.build_request(path, request)?;
        let response = self.service.call(request).await.map_err(|e| {
            TwirpError::wrap(
                TwirpErrorCode::Unknown,
                format!("Transport error during the request: {e}"),
                e,
            )
        })?;
        self.extract_response(response).await
    }

    fn build_request<T: ReflectMessage>(
        &self,
        path: &str,
        message: &T,
    ) -> Result<Request<TwirpRequestBody>, TwirpError> {
        let mut request_builder = Request::builder().method(Method::POST);
        request_builder = if let Some(base_url) = &self.base_url {
            request_builder.uri(format!("{}{}", base_url, path))
        } else {
            request_builder.uri(path)
        };
        if self.use_json {
            request_builder
                .header(CONTENT_TYPE, APPLICATION_JSON)
                .body(json_encode(message)?.into())
        } else {
            let mut buffer = BytesMut::with_capacity(message.encoded_len());
            message.encode(&mut buffer).map_err(|e| {
                TwirpError::wrap(
                    TwirpErrorCode::Internal,
                    format!("Failed to serialize to protobuf: {e}"),
                    e,
                )
            })?;
            request_builder
                .header(CONTENT_TYPE, APPLICATION_PROTOBUF)
                .body(Bytes::from(buffer).into())
        }
        .map_err(|e| {
            TwirpError::wrap(
                TwirpErrorCode::Malformed,
                format!("Failed to construct request: {e}"),
                e,
            )
        })
    }

    async fn extract_response<T: ReflectMessage + Default>(
        &self,
        response: Response<S::ResponseBody>,
    ) -> Result<T, TwirpError> {
        // We collect the body
        // TODO: size limit
        let (parts, body) = response.into_parts();
        let body = body.collect().await.map_err(|e| {
            TwirpError::wrap(
                TwirpErrorCode::Internal,
                format!("Failed to load request body: {e}"),
                e,
            )
        })?;
        let response = Response::from_parts(parts, body);

        // Error
        if response.status() != StatusCode::OK {
            return Err(response.map(|b| b.to_bytes()).into());
        }

        // Success
        let content_type = response.headers().get(CONTENT_TYPE).cloned();
        let body = response.into_body();
        if content_type == Some(APPLICATION_PROTOBUF) {
            T::decode(body.aggregate()).map_err(|e| {
                TwirpError::wrap(
                    TwirpErrorCode::Malformed,
                    format!("Bad response binary protobuf encoding: {e}"),
                    e,
                )
            })
        } else if content_type == Some(APPLICATION_JSON) {
            json_decode(&body.to_bytes())
        } else if let Some(content_type) = content_type {
            Err(TwirpError::malformed(format!(
                "Unsupported response content-type: {}",
                String::from_utf8_lossy(content_type.as_bytes())
            )))
        } else {
            Err(TwirpError::malformed("No content-type in the response"))
        }
    }
}

/// A service that can be used to send Twirp requests eg. an HTTP client
///
/// Used by [`TwirpHttpClient`] to handle HTTP.
#[trait_variant::make(Send)]
pub trait TwirpHttpService: 'static {
    type ResponseBody: Body<Error: Error + Send + Sync>;
    type Error: Error + Send + Sync + 'static;

    async fn ready(&self) -> Result<(), Self::Error>;

    async fn call(
        &self,
        request: Request<TwirpRequestBody>,
    ) -> Result<Response<Self::ResponseBody>, Self::Error>;
}

impl<
        S: Service<
                Request<TwirpRequestBody>,
                Error: Error + Send + Sync + 'static,
                Response = Response<RespBody>,
                Future: Send,
            > + Clone
            + Send
            + Sync
            + 'static,
        RespBody: Body<Error: Error + Send + Sync + 'static>,
    > TwirpHttpService for S
{
    type ResponseBody = RespBody;
    type Error = S::Error;

    async fn ready(&self) -> Result<(), Self::Error> {
        poll_fn(|cx| Service::poll_ready(&mut self.clone(), cx)).await
    }

    async fn call(
        &self,
        request: Request<TwirpRequestBody>,
    ) -> Result<Response<RespBody>, S::Error> {
        Service::call(&mut self.clone(), request).await
    }
}

/// Request body for Twirp requests.
///
/// It is a thin wrapper on top of [`Bytes`] to implement [`Body`].
pub struct TwirpRequestBody(Bytes);

impl From<Bytes> for TwirpRequestBody {
    #[inline]
    fn from(body: Bytes) -> Self {
        Self(body)
    }
}

impl From<TwirpRequestBody> for Bytes {
    #[inline]
    fn from(body: TwirpRequestBody) -> Self {
        body.0
    }
}

impl Body for TwirpRequestBody {
    type Data = Bytes;
    type Error = Infallible;

    #[inline]
    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let data = take(&mut self.0);
        Poll::Ready(if data.has_remaining() {
            Some(Ok(Frame::data(data)))
        } else {
            None
        })
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
        !self.0.has_remaining()
    }

    #[inline]
    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.0.remaining() as u64)
    }
}

fn json_encode<T: ReflectMessage>(message: &T) -> Result<Bytes, TwirpError> {
    let mut serializer = serde_json::Serializer::new(Vec::new());
    message
        .transcode_to_dynamic()
        .serialize(&mut serializer)
        .map_err(|e| {
            TwirpError::wrap(
                TwirpErrorCode::Malformed,
                format!("Failed to serialize request to JSON: {e}"),
                e,
            )
        })?;
    Ok(serializer.into_inner().into())
}

fn json_decode<T: ReflectMessage + Default>(message: &[u8]) -> Result<T, TwirpError> {
    let dynamic_message = dynamic_json_decode::<T>(message).map_err(|e| {
        TwirpError::wrap(
            TwirpErrorCode::Malformed,
            format!("Failed to parse JSON response: {e}"),
            e,
        )
    })?;
    dynamic_message.transcode_to().map_err(|e| {
        TwirpError::internal(format!(
            "Internal error while parsing the JSON response: {e}"
        ))
    })
}

fn dynamic_json_decode<T: ReflectMessage + Default>(
    message: &[u8],
) -> Result<DynamicMessage, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_slice(message);
    let dynamic_message =
        DynamicMessage::deserialize(T::default().descriptor(), &mut deserializer)?;
    deserializer.end()?;
    Ok(dynamic_message)
}

/// Wraps a [`reqwest::Client`](reqwest_012::Client) into a [`tower::Service`](Service) compatible with [`TwirpHttpClient`].
#[cfg(feature = "reqwest-012")]
#[derive(Clone, Default)]
pub struct Reqwest012Service(reqwest_012::Client);

#[cfg(feature = "reqwest-012")]
impl Reqwest012Service {
    #[inline]
    pub fn new() -> Self {
        reqwest_012::Client::new().into()
    }
}

#[cfg(feature = "reqwest-012")]
impl From<reqwest_012::Client> for Reqwest012Service {
    #[inline]
    fn from(client: reqwest_012::Client) -> Self {
        Self(client)
    }
}

#[cfg(feature = "reqwest-012")]
impl<B: Into<reqwest_012::Body>> Service<Request<B>> for Reqwest012Service {
    type Response = Response<reqwest_012::Body>;
    type Error = reqwest_012::Error;
    type Future = Pin<
        Box<dyn Future<Output = Result<Response<reqwest_012::Body>, reqwest_012::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let req = match req.try_into() {
            Ok(req) => req,
            Err(e) => return Box::pin(async move { Err(e) }),
        };
        let future = self.0.call(req);
        Box::pin(async move { Ok(future.await?.into()) })
    }
}

#[cfg(feature = "reqwest-012")]
impl From<TwirpRequestBody> for reqwest_012::Body {
    #[inline]
    fn from(body: TwirpRequestBody) -> Self {
        body.0.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "reqwest-012")]
    use prost_reflect::prost::Message;
    use prost_reflect::prost_types::Timestamp;
    use std::future::Ready;
    use std::io;
    use std::task::{Context, Poll};
    use tower::service_fn;

    #[tokio::test]
    async fn not_ready_service() -> Result<(), Box<dyn Error>> {
        #[derive(Clone)]
        struct NotReadyService;

        impl<S> Service<S> for NotReadyService {
            type Response = Response<String>;
            type Error = TwirpError;
            type Future = Ready<Result<Response<String>, TwirpError>>;

            fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Err(TwirpError::internal("foo")))
            }

            fn call(&mut self, _: S) -> Self::Future {
                unimplemented!()
            }
        }

        let client = TwirpHttpClient::new(NotReadyService);
        assert_eq!(
            client
                .call::<_, Timestamp>("", &Timestamp::default())
                .await
                .unwrap_err()
                .to_string(),
            "Twirp Unknown error: Service is not ready: Twirp Internal error: foo"
        );
        Ok(())
    }

    #[tokio::test]
    async fn json_request_without_base_ok() -> Result<(), Box<dyn Error>> {
        let service = service_fn(|request: Request<TwirpRequestBody>| async move {
            assert_eq!(request.method(), Method::POST);
            assert_eq!(request.uri(), "/foo");
            Ok::<_, TwirpError>(
                Response::builder()
                    .header(CONTENT_TYPE, APPLICATION_JSON)
                    .body("\"1970-01-01T00:00:10Z\"".to_string())
                    .unwrap(),
            )
        });

        let mut client = TwirpHttpClient::new(service);
        client.use_json();
        let response = client
            .call::<_, Timestamp>(
                "/foo",
                &Timestamp {
                    seconds: 10,
                    nanos: 0,
                },
            )
            .await?;
        assert_eq!(
            response,
            Timestamp {
                seconds: 10,
                nanos: 0
            }
        );
        Ok(())
    }

    #[cfg(feature = "reqwest-012")]
    #[tokio::test]
    async fn binary_request_without_base_ok() -> Result<(), Box<dyn Error>> {
        let service = service_fn(|request: Request<TwirpRequestBody>| async move {
            assert_eq!(request.method(), Method::POST);
            assert_eq!(request.uri(), "/foo");
            Ok::<_, TwirpError>(
                Response::builder()
                    .header(CONTENT_TYPE, APPLICATION_PROTOBUF)
                    .body(reqwest_012::Body::from(
                        Timestamp {
                            seconds: 10,
                            nanos: 0,
                        }
                        .encode_to_vec(),
                    ))
                    .unwrap(),
            )
        });

        let response = TwirpHttpClient::new(service)
            .call::<_, Timestamp>(
                "/foo",
                &Timestamp {
                    seconds: 10,
                    nanos: 0,
                },
            )
            .await?;
        assert_eq!(
            response,
            Timestamp {
                seconds: 10,
                nanos: 0
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn request_with_base_twirp_error() -> Result<(), Box<dyn Error>> {
        let service = service_fn(|request: Request<TwirpRequestBody>| async move {
            assert_eq!(request.method(), Method::POST);
            assert_eq!(request.uri(), "http://example.com/twirp/foo");
            Ok::<Response<String>, TwirpError>(TwirpError::not_found("not found").into())
        });

        let response_error = TwirpHttpClient::new_with_base(service, "http://example.com/twirp")
            .call::<_, Timestamp>(
                "/foo",
                &Timestamp {
                    seconds: 10,
                    nanos: 0,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(response_error, TwirpError::not_found("not found"));
        Ok(())
    }

    #[tokio::test]
    async fn request_with_base_other_error() -> Result<(), Box<dyn Error>> {
        let service = service_fn(|request: Request<TwirpRequestBody>| async move {
            assert_eq!(request.method(), Method::POST);
            assert_eq!(request.uri(), "http://example.com/twirp/foo");
            Ok::<Response<String>, TwirpError>(
                Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body("foo".to_string())
                    .unwrap(),
            )
        });

        let response_error = TwirpHttpClient::new_with_base(service, "http://example.com/twirp/")
            .call::<_, Timestamp>(
                "/foo",
                &Timestamp {
                    seconds: 10,
                    nanos: 0,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(response_error, TwirpError::unauthenticated("foo"));
        Ok(())
    }

    #[tokio::test]
    async fn request_transport_error() -> Result<(), Box<dyn Error>> {
        let service = service_fn(|request: Request<TwirpRequestBody>| async move {
            assert_eq!(request.method(), Method::POST);
            assert_eq!(request.uri(), "/foo");
            Err::<Response<String>, _>(io::Error::other("Transport error"))
        });

        let response_error = TwirpHttpClient::new(service)
            .call::<_, Timestamp>(
                "/foo",
                &Timestamp {
                    seconds: 10,
                    nanos: 0,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(
            response_error,
            TwirpError::new(
                TwirpErrorCode::Unknown,
                "Transport error during the request: Transport error"
            )
        );
        Ok(())
    }

    #[tokio::test]
    async fn wrong_content_type_response() -> Result<(), Box<dyn Error>> {
        let service = service_fn(|request: Request<TwirpRequestBody>| async move {
            assert_eq!(request.method(), Method::POST);
            assert_eq!(request.uri(), "/foo");
            Ok::<Response<String>, TwirpError>(
                Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, "foo/bar")
                    .body("foo".into())
                    .unwrap(),
            )
        });

        let response_error = TwirpHttpClient::new(service)
            .call::<_, Timestamp>(
                "/foo",
                &Timestamp {
                    seconds: 10,
                    nanos: 0,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(
            response_error,
            TwirpError::malformed("Unsupported response content-type: foo/bar")
        );
        Ok(())
    }

    #[tokio::test]
    async fn invalid_protobuf_response() -> Result<(), Box<dyn Error>> {
        let service = service_fn(|request: Request<TwirpRequestBody>| async move {
            assert_eq!(request.method(), Method::POST);
            assert_eq!(request.uri(), "/foo");
            Ok::<Response<String>, TwirpError>(
                Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, APPLICATION_PROTOBUF)
                    .body("azerty".into())
                    .unwrap(),
            )
        });

        let mut client = TwirpHttpClient::new(service);
        client.use_json();
        let response_error = client
            .call::<_, Timestamp>(
                "/foo",
                &Timestamp {
                    seconds: 10,
                    nanos: 0,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(
            response_error,
            TwirpError::malformed("Bad response binary protobuf encoding: failed to decode Protobuf message: buffer underflow")
        );
        Ok(())
    }

    #[tokio::test]
    async fn invalid_json_response() -> Result<(), Box<dyn Error>> {
        let service = service_fn(|request: Request<TwirpRequestBody>| async move {
            assert_eq!(request.method(), Method::POST);
            assert_eq!(request.uri(), "/foo");
            Ok::<Response<String>, TwirpError>(
                Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, APPLICATION_JSON)
                    .body("foo".into())
                    .unwrap(),
            )
        });

        let mut client = TwirpHttpClient::new(service);
        client.use_json();
        let response_error = client
            .call::<_, Timestamp>(
                "/foo",
                &Timestamp {
                    seconds: 10,
                    nanos: 0,
                },
            )
            .await
            .unwrap_err();
        assert_eq!(
            response_error,
            TwirpError::malformed(
                "Failed to parse JSON response: expected ident at line 1 column 2"
            )
        );
        Ok(())
    }

    #[tokio::test]
    async fn response_future_is_send() {
        fn is_send<T: Send>(_: T) {}

        let service = service_fn(|_: Request<TwirpRequestBody>| async move {
            Ok::<_, TwirpError>(Response::new(String::new()))
        });
        let client = TwirpHttpClient::new(service);

        // This will fail to compile if the future is not Send
        is_send(client.call::<_, Timestamp>("/foo", &Timestamp::default()));
    }
}
