fn main() -> std::io::Result<()> {
    twurst_build::TwirpBuilder::new()
        .with_client()
        .compile_protos(&["../example_service.proto"], &[".."])
}
