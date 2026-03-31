use std::path::PathBuf;
use std::{env, fs};

fn main() -> std::io::Result<()> {
    twurst_build::TwirpBuilder::new()
        .with_client()
        .with_server()
        .with_grpc()
        .with_default_axum_request_extractor("bearer_token", "crate::server::ExtractBearerToken")
        .compile_protos(&["integration.proto"], &["."])?;

    // Custom out dir to not override Twirp
    let tonic_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("tonic");
    fs::create_dir_all(&tonic_dir)?;
    tonic_prost_build::configure()
        .out_dir(&tonic_dir)
        .type_attribute("Int", "#[allow(dead_code)]") // to make clippy happy
        .compile_protos(&["integration.proto"], &["."])?;

    // Custom out dir with skip_prost_reflect: caller configures prost-reflect externally
    let custom_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("custom");
    fs::create_dir_all(&custom_dir)?;
    let mut custom_config = twurst_build::prost::Config::new();
    custom_config.out_dir(&custom_dir);
    prost_reflect_build::Builder::new()
        .file_descriptor_set_path(custom_dir.join("file_descriptor_set.bin"))
        .descriptor_pool("crate::custom_out_dir::DESCRIPTOR_POOL")
        .configure(&mut custom_config, &["integration.proto"], &["."])?;
    twurst_build::TwirpBuilder::from_prost(custom_config)
        .skip_prost_reflect()
        .with_server()
        .compile_protos(&["integration.proto"], &["."])?;

    Ok(())
}
