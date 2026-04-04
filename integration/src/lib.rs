pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/integration.rs"));
}

/// Generated with a custom `out_dir` and `skip_prost_reflect`, exercising the
/// code path where callers configure prost-reflect externally.
pub mod custom_out_dir {
    use prost_reflect::DescriptorPool;
    use std::sync::LazyLock;

    static DESCRIPTOR_POOL: LazyLock<DescriptorPool> = LazyLock::new(|| {
        DescriptorPool::decode(
            include_bytes!(concat!(env!("OUT_DIR"), "/custom/file_descriptor_set.bin")).as_ref(),
        )
        .expect("failed to decode descriptor pool")
    });

    include!(concat!(env!("OUT_DIR"), "/custom/integration.rs"));
}

pub mod client;
pub mod server;
