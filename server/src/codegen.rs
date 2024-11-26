use crate::TwirpError;
use axum::body::Body;
pub use axum::extract::FromRequestParts;
use axum::extract::{Request, State};
use axum::http::header::CONTENT_TYPE;
pub use axum::http::request::Parts as RequestParts;
#[cfg(feature = "grpc")]
use axum::http::Method;
use axum::http::{HeaderMap, HeaderValue, Uri};
pub use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::post;
use axum::RequestExt;
pub use axum::Router;
use http_body_util::BodyExt;
use prost_reflect::bytes::{Bytes, BytesMut};
use prost_reflect::{DynamicMessage, ReflectMessage};
use serde::Serialize;
use std::future::Future;
#[cfg(feature = "grpc")]
use std::marker::PhantomData;
#[cfg(feature = "grpc")]
use std::pin::Pin;
use tracing::error;
pub use trait_variant::make as trait_variant_make;
use twurst_error::TwirpErrorCode;

const APPLICATION_JSON: HeaderValue = HeaderValue::from_static("application/json");
const APPLICATION_PROTOBUF: HeaderValue = HeaderValue::from_static("application/protobuf");

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

    pub fn build(self) -> Router<RS> {
        self.router.fallback(|uri: Uri| async move {
            TwirpError::new(
                TwirpErrorCode::BadRoute,
                format!("{} is not a supported Twirp method", uri.path()),
            )
        })
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
            Ok(ContentType::Protobuf)
        } else if content_type == APPLICATION_JSON {
            Ok(ContentType::Json)
        } else {
            Err(TwirpError::malformed(format!(
                "Unsupported content type: {}",
                String::from_utf8_lossy(content_type.as_bytes())
            )))
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
    let (content_type, body) = match content_type {
        ContentType::Protobuf => {
            let mut buffer = BytesMut::with_capacity(response.encoded_len());
            response.encode(&mut buffer).map_err(|e| {
                TwirpError::wrap(
                    TwirpErrorCode::Internal,
                    format!("Failed to serialize to protobuf: {e}"),
                    e,
                )
            })?;
            (APPLICATION_PROTOBUF, buffer.into())
        }
        ContentType::Json => (APPLICATION_JSON, json_encode(&response)?),
    };
    Response::builder()
        .header(CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .map_err(|e| {
            error!("Failed to build the response: {e}");
            TwirpError::internal("Failed to build the response")
        })
}

fn json_encode<T: ReflectMessage>(message: &T) -> Result<Bytes, TwirpError> {
    let mut serializer = serde_json::Serializer::new(Vec::new());
    message
        .transcode_to_dynamic()
        .serialize(&mut serializer)
        .map_err(|e| {
            error!("Failed to serialize the JSON response: {e}");
            TwirpError::internal("Failed to build the response")
        })?;
    Ok(serializer.into_inner().into())
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
        use tonic::{Code, Status};

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
                    // TODO: Why do we need these?
                    // (vsiles) I/O seems to be necessary for the UnaryService impl.
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
                            dyn Future<Output = Result<tonic::Response<Self::Response>, Status>>
                                + Send
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
                            match result_future.await {
                                Ok(response) => Ok(tonic::Response::new(response)),
                                Err(error) => Err(Status::new(
                                    // TODO: extract this into proper `From` impl
                                    match error.code() {
                                        TwirpErrorCode::Canceled => Code::Cancelled,
                                        TwirpErrorCode::Unknown => Code::Unknown,
                                        TwirpErrorCode::InvalidArgument => Code::InvalidArgument,
                                        TwirpErrorCode::Malformed => Code::InvalidArgument,
                                        TwirpErrorCode::DeadlineExceeded => Code::DeadlineExceeded,
                                        TwirpErrorCode::NotFound => Code::NotFound,
                                        TwirpErrorCode::BadRoute => Code::NotFound,
                                        TwirpErrorCode::AlreadyExists => Code::AlreadyExists,
                                        TwirpErrorCode::PermissionDenied => Code::PermissionDenied,
                                        TwirpErrorCode::Unauthenticated => Code::Unauthenticated,
                                        TwirpErrorCode::ResourceExhausted => {
                                            Code::ResourceExhausted
                                        }
                                        TwirpErrorCode::FailedPrecondition => {
                                            Code::FailedPrecondition
                                        }
                                        TwirpErrorCode::Aborted => Code::Aborted,
                                        TwirpErrorCode::OutOfRange => Code::OutOfRange,
                                        TwirpErrorCode::Unimplemented => Code::Unimplemented,
                                        TwirpErrorCode::Internal => Code::Internal,
                                        TwirpErrorCode::Unavailable => Code::Unavailable,
                                        TwirpErrorCode::Dataloss => Code::DataLoss,
                                    },
                                    error.into_message(),
                                )),
                            }
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

    pub fn build(self) -> Router {
        self.router.fallback(|uri: Uri| async move {
            tonic::Status::new(
                tonic::Code::NotFound,
                format!("{} is not a supported gRPC method", uri.path()),
            )
            .into_http()
        })
    }
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
        let router = TwirpRouter::new(()).build();
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
