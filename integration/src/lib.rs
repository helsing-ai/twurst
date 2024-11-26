pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/integration.rs"));
}

pub mod client;
pub mod server;
