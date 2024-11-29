use crate::TwirpError;
use axum::body::Body;
pub use axum::extract::FromRequestParts;
use axum::extract::{Request, State};
use axum::http::header::CONTENT_TYPE;
pub use axum::http::request::Parts as RequestParts;
#[cfg(feature = "grpc")]
use axum::http::Method;
use axum::http::{HeaderMap, HeaderValue};
pub use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::post;
use axum::RequestExt;
pub use axum::Router;
use http_body_util::BodyExt;
use prost::bytes::BufMut;
use prost_reflect::bytes::{Bytes, BytesMut};
use prost_reflect::{DynamicMessage, ReflectMessage};
use serde::Serialize;
use std::convert::Infallible;
use std::future::Future;
#[cfg(feature = "grpc")]
use std::marker::PhantomData;
#[cfg(feature = "grpc")]
use std::pin::Pin;
#[cfg(feature = "twurst-stream")]
pub use tokio_stream::Stream;
#[cfg(feature = "twurst-stream")]
use tokio_stream::StreamExt;
use tracing::error;
pub use trait_variant::make as trait_variant_make;
use twurst_error::TwirpErrorCode;

const APPLICATION_JSON: HeaderValue = HeaderValue::from_static("application/json");
const APPLICATION_PROTOBUF: HeaderValue = HeaderValue::from_static("application/protobuf");
#[cfg(feature = "twurst-stream")]
const APPLICATION_JSONL: HeaderValue = HeaderValue::from_static("application/jsonl");
#[cfg(feature = "twurst-stream")]
const APPLICATION_PROTOBUF_STREAM: HeaderValue =
    HeaderValue::from_static("application/x-twurst-protobuf-stream");

pub struct TwirpRouter<S, RS = ()> {
    router: Router<RS>,
    service: S,
}

impl<S: Clone + Send + Sync + 'static, RS: Clone + Send + Sync + 'static> TwirpRouter<S, RS> {
    pub fn new(service: S) -> Self {
        Self {
            router: Router::new(),
            service,
        }
    }

    pub fn route<
        I: ReflectMessage + Default,
        O: ReflectMessage,
        F: Future<Output = Result<O, TwirpError>> + Send,
    >(
        mut self,
        path: &str,
        call: impl (Fn(S, I, RequestParts, RS) -> F) + Clone + Send + 'static,
    ) -> Self {
        let service = self.service.clone();
        self.router = self.router.route(
            path,
            post(
                move |State(state): State<RS>, request: Request| async move {
                    let (parts, body) = request.with_limited_body().into_parts();
                    let content_type = ContentType::from_headers(&parts.headers)?;
                    let request = parse_request(content_type, body).await?;
                    let response = call(service, request, parts, state).await?;
                    serialize_response(content_type, response)
                },
            ),
        );
        self
    }

    #[cfg(feature = "twurst-stream")]
    pub fn route_server_streaming<
        I: ReflectMessage + Default,
        O: ReflectMessage,
        F: Future<Output = Result<OS, TwirpError>> + Send,
        OS: Stream<Item = Result<O, TwirpError>> + Send + 'static,
    >(
        mut self,
        path: &str,
        call: impl (Fn(S, I, RequestParts, RS) -> F) + Clone + Send + 'static,
    ) -> Self {
        let service = self.service.clone();
        self.router = self.router.route(
            path,
            post(
                move |State(state): State<RS>, request: Request| async move {
                    let (parts, body) = request.with_limited_body().into_parts();
                    let content_type = ContentType::from_headers(&parts.headers)?;
                    let request = parse_request(content_type, body).await?;
                    let response = call(service, request, parts, state).await?;
                    serialize_stream_response(content_type.into(), response)
                },
            ),
        );
        self
    }

    pub fn build(self) -> Router<RS> {
        self.router
    }
}

#[derive(Clone, Copy)]
enum ContentType {
    Protobuf,
    Json,
}

impl ContentType {
    fn from_headers(headers: &HeaderMap) -> Result<Self, TwirpError> {
        let content_type = headers
            .get(CONTENT_TYPE)
            .ok_or_else(|| TwirpError::malformed("No content-type header"))?;
        if content_type == APPLICATION_PROTOBUF {
            Ok(Self::Protobuf)
        } else if content_type == APPLICATION_JSON {
            Ok(Self::Json)
        } else {
            Err(TwirpError::malformed(format!(
                "Unsupported content type: {}",
                String::from_utf8_lossy(content_type.as_bytes())
            )))
        }
    }
}

impl From<ContentType> for HeaderValue {
    fn from(content_type: ContentType) -> Self {
        match content_type {
            ContentType::Protobuf => APPLICATION_PROTOBUF,
            ContentType::Json => APPLICATION_JSON,
        }
    }
}

#[cfg(feature = "twurst-stream")]
#[derive(Clone, Copy)]
enum StreamContentType {
    Protobuf,
    Json,
}

#[cfg(feature = "twurst-stream")]
impl From<StreamContentType> for HeaderValue {
    fn from(content_type: StreamContentType) -> Self {
        match content_type {
            StreamContentType::Protobuf => APPLICATION_PROTOBUF_STREAM,
            StreamContentType::Json => APPLICATION_JSONL,
        }
    }
}

#[cfg(feature = "twurst-stream")]
impl From<ContentType> for StreamContentType {
    fn from(content_type: ContentType) -> Self {
        match content_type {
            ContentType::Protobuf => Self::Protobuf,
            ContentType::Json => Self::Json,
        }
    }
}

async fn parse_request<I: ReflectMessage + Default>(
    content_type: ContentType,
    body: Body,
) -> Result<I, TwirpError> {
    let body = body.collect().await.map_err(|e| {
        TwirpError::wrap(
            TwirpErrorCode::Internal,
            "Failed to read the request body",
            e,
        )
    })?;
    match content_type {
        ContentType::Protobuf => I::decode(body.aggregate()).map_err(|e| {
            TwirpError::wrap(
                TwirpErrorCode::Malformed,
                format!("Invalid binary protobuf request: {e}"),
                e,
            )
        }),
        ContentType::Json => json_decode(&body.to_bytes()), // TODO: avoid to_bytes?
    }
}

fn serialize_response<O: ReflectMessage>(
    content_type: ContentType,
    response: O,
) -> Result<Response, TwirpError> {
    match content_type {
        ContentType::Protobuf => {
            let mut buffer = BytesMut::with_capacity(response.encoded_len());
            response.encode(&mut buffer).map_err(|e| {
                TwirpError::wrap(
                    TwirpErrorCode::Internal,
                    format!("Failed to serialize to protobuf: {e}"),
                    e,
                )
            })?;
            build_response(ContentType::Protobuf, Bytes::from(buffer))
        }
        ContentType::Json => build_response(
            ContentType::Json,
            Bytes::from(json_encode(&response, BytesMut::new())?),
        ),
    }
}

#[cfg(feature = "twurst-stream")]
fn serialize_stream_response<O: ReflectMessage>(
    content_type: StreamContentType,
    response: impl Stream<Item = Result<O, TwirpError>> + Send + 'static, // TODO: infallible?
) -> Result<Response, TwirpError> {
    match content_type {
        StreamContentType::Protobuf => build_response(
            StreamContentType::Protobuf,
            Body::from_stream(
                response
                    .map(|chunk| {
                        let chunk = chunk?;
                        let chunk_len = chunk.encoded_len();
                        let mut buffer = BytesMut::with_capacity(5 + chunk_len);
                        buffer.put_u8(0);
                        buffer.put_u32(u32::try_from(chunk_len).map_err(|e| {
                            TwirpError::wrap(
                                TwirpErrorCode::Internal,
                                "Too large message, its length must fit in 32bits",
                                e,
                            )
                        })?);
                        chunk.encode(&mut buffer).map_err(|e| {
                            TwirpError::wrap(
                                TwirpErrorCode::Internal,
                                format!("Failed to serialize to protobuf: {e}"),
                                e,
                            )
                        })?;
                        Ok::<_, TwirpError>(buffer)
                    })
                    .map(|result| {
                        // We transmit errors as regular messages
                        Ok::<_, Infallible>(match result {
                            Ok(buffer) => buffer,
                            Err(e) => {
                                let e = serde_json::to_vec(&e).unwrap();
                                let mut buffer = BytesMut::with_capacity(5 + e.len());
                                buffer.put_u8(48);
                                buffer.put_u32(e.len().try_into().unwrap_or_default());
                                buffer.put_slice(&e);
                                buffer
                            }
                        })
                    }),
            ),
        ),
        StreamContentType::Json => build_response(
            StreamContentType::Json,
            Body::from_stream(
                response
                    .map(|chunk| {
                        let chunk = chunk?;
                        let mut buffer = BytesMut::new();
                        buffer.put_slice(b"{\"message\":");
                        let mut buffer = json_encode(&chunk, buffer)?;
                        buffer.put_slice(b"}");
                        Ok(buffer)
                    })
                    .map(|result| {
                        // We transmit errors as regular messages
                        Ok::<_, Infallible>(match result {
                            Ok(buffer) => buffer,
                            Err(error) => {
                                #[derive(Serialize)]
                                struct JsonStreamError {
                                    error: TwirpError,
                                }
                                Bytes::from(serde_json::to_vec(&JsonStreamError { error }).unwrap())
                                    .into()
                            }
                        })
                    }),
            ),
        ),
    }
}

fn build_response(
    content_type: impl Into<HeaderValue>,
    body: impl Into<Body>,
) -> Result<Response, TwirpError> {
    Response::builder()
        .header(CONTENT_TYPE, content_type)
        .body(body.into())
        .map_err(|e| {
            error!("Failed to build the response: {e}");
            TwirpError::internal("Failed to build the response")
        })
}

fn json_encode<T: ReflectMessage>(message: &T, buffer: BytesMut) -> Result<BytesMut, TwirpError> {
    let mut serializer = serde_json::Serializer::new(buffer.writer());
    message
        .transcode_to_dynamic()
        .serialize(&mut serializer)
        .map_err(|e| {
            error!("Failed to serialize the JSON response: {e}");
            TwirpError::internal("Failed to build the response")
        })?;
    Ok(serializer.into_inner().into_inner())
}

fn json_decode<T: ReflectMessage + Default>(message: &[u8]) -> Result<T, TwirpError> {
    let dynamic_message = dynamic_json_decode::<T>(message).map_err(|e| {
        TwirpError::wrap(
            TwirpErrorCode::Malformed,
            format!("Invalid JSON protobuf request: {e}"),
            e,
        )
    })?;
    dynamic_message.transcode_to().map_err(|e| {
        error!("Failed to cast input message: {e}");
        TwirpError::internal("Internal error while parsing the JSON request")
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

#[cfg(feature = "grpc")]
pub struct GrpcRouter<S> {
    router: Router,
    service: S,
}

#[cfg(feature = "grpc")]
impl<S: Clone + Send + Sync + 'static> GrpcRouter<S> {
    pub fn new(service: S) -> Self {
        Self {
            router: Router::new(),
            service,
        }
    }

    pub fn route<
        I: ReflectMessage + Default + 'static,
        O: ReflectMessage + 'static,
        C: (Fn(S, I, RequestParts) -> F) + Clone + Send + 'static,
        F: Future<Output = Result<O, TwirpError>> + Send + 'static,
    >(
        mut self,
        path: &str,
        callback: C,
    ) -> Self {
        let service = self.service.clone();
        self.router = self.router.route(
            path,
            post(move |request: Request| async move {
                struct SimpleUnaryService<
                    S: Clone + Send + Sync + 'static,
                    I: ReflectMessage + Default + 'static,
                    O: ReflectMessage + 'static,
                    C: (Fn(S, I, RequestParts) -> F) + Clone + Send + 'static,
                    F: Future<Output = Result<O, TwirpError>> + Send + 'static,
                > {
                    service: S,
                    callback: C,
                    input: PhantomData<I>,
                    output: PhantomData<O>,
                    future: PhantomData<F>,
                }

                impl<
                        S: Clone + Send + Sync + 'static,
                        I: ReflectMessage + Default + 'static,
                        O: ReflectMessage + 'static,
                        C: (Fn(S, I, RequestParts) -> F) + Clone + Send + 'static,
                        F: Future<Output = Result<O, TwirpError>> + Send + 'static,
                    > tonic::server::UnaryService<I> for SimpleUnaryService<S, I, O, C, F>
                {
                    type Response = O;
                    type Future = Pin<
                        Box<
                            dyn Future<
                                    Output = Result<tonic::Response<Self::Response>, tonic::Status>,
                                > + Send
                                + 'static,
                        >,
                    >;

                    fn call(&mut self, request: tonic::Request<I>) -> Self::Future {
                        let (metadata, extensions, request) = request.into_parts();
                        let mut request_builder = Request::builder().method(Method::POST);
                        *request_builder.headers_mut().unwrap() = metadata.into_headers();
                        *request_builder.extensions_mut().unwrap() = extensions;
                        let (parts, ()) = request_builder.body(()).unwrap().into_parts();
                        let result_future = (self.callback)(self.service.clone(), request, parts);
                        Box::pin(async move {
                            Ok(tonic::Response::new(
                                result_future.await.map_err(grpc_status_for_twirp_error)?,
                            ))
                        })
                    }
                }
                let method = SimpleUnaryService {
                    service,
                    callback,
                    input: PhantomData,
                    output: PhantomData,
                    future: PhantomData,
                };
                let codec = tonic::codec::ProstCodec::default();
                let mut grpc = tonic::server::Grpc::new(codec);
                grpc.unary(method, request).await
            }),
        );
        self
    }

    pub fn route_server_streaming<
        I: ReflectMessage + Default + 'static,
        O: ReflectMessage + 'static,
        C: (Fn(S, I, RequestParts) -> F) + Clone + Send + 'static,
        F: Future<Output = Result<OS, TwirpError>> + Send + 'static,
        OS: Stream<Item = Result<O, TwirpError>> + Send + 'static,
    >(
        mut self,
        path: &str,
        callback: C,
    ) -> Self {
        let service = self.service.clone();
        self.router = self.router.route(
            path,
            post(move |request: Request| async move {
                struct ServerStreamingService<
                    S: Clone + Send + Sync + 'static,
                    I: ReflectMessage + Default + 'static,
                    O: ReflectMessage + 'static,
                    C: (Fn(S, I, RequestParts) -> F) + Clone + Send + 'static,
                    F: Future<Output = Result<OS, TwirpError>> + Send + 'static,
                    OS: Stream<Item = Result<O, TwirpError>> + Send + 'static,
                > {
                    service: S,
                    callback: C,
                    input: PhantomData<I>,
                    output: PhantomData<O>,
                    future: PhantomData<F>,
                }

                impl<
                        S: Clone + Send + Sync + 'static,
                        I: ReflectMessage + Default + 'static,
                        O: ReflectMessage + 'static,
                        C: (Fn(S, I, RequestParts) -> F) + Clone + Send + 'static,
                        F: Future<Output = Result<OS, TwirpError>> + Send + 'static,
                        OS: Stream<Item = Result<O, TwirpError>> + Send + 'static,
                    > tonic::server::ServerStreamingService<I>
                    for ServerStreamingService<S, I, O, C, F, OS>
                {
                    type Response = O;
                    type ResponseStream =
                        Pin<Box<dyn Stream<Item = Result<Self::Response, tonic::Status>> + Send>>;
                    type Future = Pin<
                        Box<
                            dyn Future<
                                    Output = Result<
                                        tonic::Response<Self::ResponseStream>,
                                        tonic::Status,
                                    >,
                                > + Send,
                        >,
                    >;

                    fn call(&mut self, request: tonic::Request<I>) -> Self::Future {
                        let (metadata, extensions, request) = request.into_parts();
                        let mut request_builder = Request::builder().method(Method::POST);
                        *request_builder.headers_mut().unwrap() = metadata.into_headers();
                        *request_builder.extensions_mut().unwrap() = extensions;
                        let (parts, ()) = request_builder.body(()).unwrap().into_parts();
                        let result_future = (self.callback)(self.service.clone(), request, parts);
                        Box::pin(async move {
                            Ok(tonic::Response::new(Box::pin(
                                result_future
                                    .await
                                    .map_err(grpc_status_for_twirp_error)?
                                    .map(|item| item.map_err(grpc_status_for_twirp_error)),
                            )
                                as Self::ResponseStream))
                        })
                    }
                }
                let method = ServerStreamingService {
                    service,
                    callback,
                    input: PhantomData,
                    output: PhantomData,
                    future: PhantomData,
                };
                let codec = tonic::codec::ProstCodec::default();
                let mut grpc = tonic::server::Grpc::new(codec);
                grpc.server_streaming(method, request).await
            }),
        );
        self
    }

    pub fn build(self) -> Router {
        self.router
    }
}

#[cfg(feature = "grpc")]
fn grpc_status_for_twirp_error(error: TwirpError) -> tonic::Status {
    tonic::Status::new(
        // TODO: extract this into proper `From` impl
        match error.code() {
            TwirpErrorCode::Canceled => tonic::Code::Cancelled,
            TwirpErrorCode::Unknown => tonic::Code::Unknown,
            TwirpErrorCode::InvalidArgument => tonic::Code::InvalidArgument,
            TwirpErrorCode::Malformed => tonic::Code::InvalidArgument,
            TwirpErrorCode::DeadlineExceeded => tonic::Code::DeadlineExceeded,
            TwirpErrorCode::NotFound => tonic::Code::NotFound,
            TwirpErrorCode::BadRoute => tonic::Code::NotFound,
            TwirpErrorCode::AlreadyExists => tonic::Code::AlreadyExists,
            TwirpErrorCode::PermissionDenied => tonic::Code::PermissionDenied,
            TwirpErrorCode::Unauthenticated => tonic::Code::Unauthenticated,
            TwirpErrorCode::ResourceExhausted => tonic::Code::ResourceExhausted,
            TwirpErrorCode::FailedPrecondition => tonic::Code::FailedPrecondition,
            TwirpErrorCode::Aborted => tonic::Code::Aborted,
            TwirpErrorCode::OutOfRange => tonic::Code::OutOfRange,
            TwirpErrorCode::Unimplemented => tonic::Code::Unimplemented,
            TwirpErrorCode::Internal => tonic::Code::Internal,
            TwirpErrorCode::Unavailable => tonic::Code::Unavailable,
            TwirpErrorCode::Dataloss => tonic::Code::DataLoss,
        },
        error.into_message(),
    )
}

pub async fn twirp_error_from_response(response: impl IntoResponse) -> TwirpError {
    let (parts, body) = response.into_response().into_parts();
    let body = match body.collect().await {
        Ok(body) => body.to_bytes(),
        Err(e) => {
            error!("Failed to load the body of the HTTP payload when building a TwirpError from a generic HTTP response: {e}");
            return TwirpError::wrap(
                TwirpErrorCode::Internal,
                "Failed to map an internal error",
                e,
            );
        }
    };
    Response::from_parts(parts, body).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twirp_fallback;
    #[cfg(feature = "grpc")]
    use axum::http::uri::PathAndQuery;
    use axum::http::{Method, Request, StatusCode};
    use http_body_util::BodyExt;
    use prost::Message;
    #[cfg(feature = "grpc")]
    use tonic::client::Grpc;
    #[cfg(feature = "grpc")]
    use tonic::codec::ProstCodec;
    #[cfg(feature = "grpc")]
    use tonic::Code;
    use tower_service::Service;

    const FILE_DESCRIPTOR_SET_BYTES: &[u8] = &[
        10, 107, 10, 21, 101, 120, 97, 109, 112, 108, 101, 95, 115, 101, 114, 118, 105, 99, 101,
        46, 112, 114, 111, 116, 111, 18, 7, 112, 97, 99, 107, 97, 103, 101, 34, 11, 10, 9, 77, 121,
        77, 101, 115, 115, 97, 103, 101, 74, 52, 10, 6, 18, 4, 0, 0, 5, 1, 10, 8, 10, 1, 12, 18, 3,
        0, 0, 18, 10, 8, 10, 1, 2, 18, 3, 2, 0, 16, 10, 10, 10, 2, 4, 0, 18, 4, 4, 0, 5, 1, 10, 10,
        10, 3, 4, 0, 1, 18, 3, 4, 8, 17, 98, 6, 112, 114, 111, 116, 111, 51,
    ];

    #[derive(Message, ReflectMessage, PartialEq)]
    #[prost_reflect(
        file_descriptor_set_bytes = "crate::codegen::tests::FILE_DESCRIPTOR_SET_BYTES",
        message_name = "package.MyMessage"
    )]
    pub struct MyMessage {}

    #[tokio::test]
    async fn test_bad_route() {
        let router = TwirpRouter::new(()).build().fallback(twirp_fallback);
        let response = router
            .into_service()
            .call(Request::new(Body::empty()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            b"{\"code\":\"bad_route\",\"msg\":\"/ is not a supported Twirp method\"}".as_slice()
        );
    }

    #[tokio::test]
    async fn test_no_content_type() {
        let router = TwirpRouter::new(())
            .route(
                "/package.MyService/MyMethod",
                |(), request: MyMessage, _, _| async move { Ok(request) },
            )
            .build();
        let response = router
            .into_service()
            .call(
                Request::builder()
                    .method(Method::POST)
                    .uri("/package.MyService/MyMethod")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            b"{\"code\":\"malformed\",\"msg\":\"No content-type header\"}".as_slice()
        );
    }

    #[tokio::test]
    async fn test_ok_binary() {
        let router = TwirpRouter::new(())
            .route(
                "/package.MyService/MyMethod",
                |(), request: MyMessage, _, _| async move { Ok(request) },
            )
            .build();
        let response = router
            .into_service()
            .call(
                Request::builder()
                    .method(Method::POST)
                    .header(CONTENT_TYPE, APPLICATION_PROTOBUF)
                    .uri("/package.MyService/MyMethod")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            [].as_slice()
        );
    }

    #[tokio::test]
    async fn test_bad_binary() {
        let router = TwirpRouter::new(())
            .route(
                "/package.MyService/MyMethod",
                |(), request: MyMessage, _, _| async move { Ok(request) },
            )
            .build();
        let response = router
            .into_service()
            .call(
                Request::builder()
                    .method(Method::POST)
                    .header(CONTENT_TYPE, APPLICATION_PROTOBUF)
                    .uri("/package.MyService/MyMethod")
                    .body(Body::from(b"1234".to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            b"{\"code\":\"malformed\",\"msg\":\"Invalid binary protobuf request: failed to decode Protobuf message: buffer underflow\"}".as_slice()
        );
    }

    #[tokio::test]
    async fn test_ok_json() {
        let router = TwirpRouter::new(())
            .route(
                "/package.MyService/MyMethod",
                |(), request: MyMessage, _, _| async move { Ok(request) },
            )
            .build();
        let response = router
            .into_service()
            .call(
                Request::builder()
                    .method(Method::POST)
                    .header(CONTENT_TYPE, APPLICATION_JSON)
                    .uri("/package.MyService/MyMethod")
                    .body(Body::from(b"{}".to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            b"{}".as_slice()
        );
    }

    #[tokio::test]
    async fn test_bad_json() {
        let router = TwirpRouter::new(())
            .route(
                "/package.MyService/MyMethod",
                |(), request: MyMessage, _, _| async move { Ok(request) },
            )
            .build();
        let response = router
            .into_service()
            .call(
                Request::builder()
                    .method(Method::POST)
                    .header(CONTENT_TYPE, APPLICATION_JSON)
                    .uri("/package.MyService/MyMethod")
                    .body(Body::from(b"foo".to_vec()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            b"{\"code\":\"malformed\",\"msg\":\"Invalid JSON protobuf request: expected ident at line 1 column 2\"}".as_slice()
        );
    }

    #[tokio::test]
    async fn test_bad_content_type() {
        let router = TwirpRouter::new(())
            .route(
                "/package.MyService/MyMethod",
                |(), request: MyMessage, _, _| async move { Ok(request) },
            )
            .build();
        let response = router
            .into_service()
            .call(
                Request::builder()
                    .method(Method::POST)
                    .header(CONTENT_TYPE, "foo/bar")
                    .uri("/package.MyService/MyMethod")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            b"{\"code\":\"malformed\",\"msg\":\"Unsupported content type: foo/bar\"}".as_slice()
        );
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn test_grpc_request() {
        let router = GrpcRouter::new(())
            .route(
                "/package.MyService/MyMethod",
                |(), request: MyMessage, _| async move { Ok(request) },
            )
            .build();
        let path = PathAndQuery::from_static("/package.MyService/MyMethod");
        let response: MyMessage = Grpc::new(router)
            .unary(
                tonic::Request::new(MyMessage {}),
                path,
                ProstCodec::default(),
            )
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response, MyMessage {})
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn test_grpc_request_with_error() {
        let router = GrpcRouter::new(())
            .route(
                "/package.MyService/MyMethod",
                |(), _: MyMessage, _| async move {
                    Err::<MyMessage, _>(TwirpError::not_found("foo not found"))
                },
            )
            .build();
        let path = PathAndQuery::from_static("/package.MyService/MyMethod");
        let status = Grpc::new(router)
            .unary::<_, MyMessage, _>(
                tonic::Request::new(MyMessage {}),
                path,
                ProstCodec::default(),
            )
            .await
            .unwrap_err();
        assert_eq!(status.code(), Code::NotFound);
        assert_eq!(status.message(), "foo not found");
    }
}
