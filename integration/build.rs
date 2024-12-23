use std::path::PathBuf;
use std::{env, fs};

fn main() -> std::io::Result<()> {
    twurst_build::TwirpBuilder::new()
        .with_client()
        .with_server()
        .with_grpc()
        .with_axum_request_extractor("bearer_token", "crate::server::ExtractBearerToken")
        .compile_protos(&["integration.proto"], &["."])?;

    // Custom out dir to not override Twirp
    let dir = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("tonic");
    fs::create_dir_all(&dir)?;
    tonic_build::configure()
        .out_dir(dir)
        .compile_protos(&["integration.proto"], &["."])
}
