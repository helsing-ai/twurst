Compile-time library to build `proto` files via [`prost-build`](https://docs.rs/prost-build)
and generate service stubs to be used with `twurst-server`.

## Getting started

Create a `build.rs` with:

```rust,no_run
fn main() -> std::io::Result<()> {
    twurst_build::TwirpBuilder::new()
        .with_server() // If you want to build a Twirp server
        .with_client() // If you want to build a Twirp client
        .compile_protos(&["proto/service.proto"], &["proto"])
}
```

and add to your `Cargo.toml`:

```toml
[dependencies]
prost = ""
prost-types = ""
prost-reflect = ""

[build-dependencies]
twurst-build = ""
```

Note that `protoc` must be available, see [`prost-build` documentation on this topic](https://docs.rs/prost-build/latest/prost_build/#sourcing-protoc).
If you have nix installed, we also provide a dev-shell that provides `protoc`. Use `nix develop` or `direnv` to enter the dev-shell.

See `twurst-client` and `twurst-server` for more detailed documentation on the server and client usages.

## Cargo features
- `grpc` generate server stubs for a gRPC server using [`tonic`](https://docs.rs/tonic/). See `twurst-server` documentation more more details.

## License

Copyright 2024 Helsing GmbH

Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License.
You may obtain a copy of the License at <http://www.apache.org/licenses/LICENSE-2.0>

Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and limitations under the License.
