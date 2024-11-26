fn main() -> std::io::Result<()> {
    twurst_build::TwirpBuilder::new()
        .with_server()
        .with_axum_request_extractor("headers", "::axum::http::HeaderMap")
        .compile_protos(&["../example_service.proto"], &[".."])
}
