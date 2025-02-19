# Twurst

Twurst is an implementation of [Twirp v7](https://twitchtv.github.io/twirp/docs/spec_v7.html) in Rust
on top of [`prost`](https://docs.rs/prost/), the [`tower`](https://docs.rs/tower) ecosystem and [`axum`](https://docs.rs/axum).
It fully supports JSON and its server can also serve regular gRPC.

<div align="center">

<img src="docs/img/twurst.png" alt="Twurst logo" width="200px" height="auto" />

# `Twurst`

</div>

## What is Twirp?

[Twirp](https://twitchtv.github.io/twirp/docs/spec_v7.html) is a protocol similar to gRPC but with some differences:
- Based on protobuf and gRPC `service` definitions
- Uses plain HTTP 1.1 as a wire format (with upgrade to HTTP 2/3 if the client and server agree)
- No stream requests/responses
- Protobuf JSON format is a first class citizen and can be used instead of protobuf binary encoding
- Errors are encoded as a JSON payload and correct HTTP status code must be used

Useful properties:
- Same protocol level behaviors as web browsers and tools like `curl` or `reqwest` just work
- JSON if text based human-readable communications are required
- Same definition files as gRPC, fairly simple migration coming from gRPC

Twirp has been created by Twitch and its [original implementation](https://github.com/twitchtv/twirp) is in Go.
A [list of other implementations is provided](https://github.com/twitchtv/twirp?tab=readme-ov-file#implementations-in-other-languages).

## This implementation

This is a simple implementation of Twirp.
Its aim is to be a replacement of [`tonic`](https://docs.rs/tonic/) using Twirp instead of gRPC.
It is also based on [`prost`](https://docs.rs/prost/) with the help of [`prost-reflect`](https://docs.rs/prost-reflect/) for JSON support.

It is split into multiple crates:
- [`twurst-build`](./build) is the equivalent of `tonic-build` for Twirp: it generates code from the `.proto` files.
- [`twurst-client`](./client) wraps a [`reqwest::Client`](https://docs.rs/reqwest/latest/reqwest/struct.Client.html) or any [`tower::Service`](https://docs.rs/tower/latest/tower/trait.Service.html)
  and provides the needed code for the `*Client` structs generated by `twurst-build` to work.
- [`twurst-server`](./server) implements a Twirp server on top of [`axum`](https://docs.rs/axum/) from an implementation of the service trait generated by `twurst-build`.
  It is not a fully fledged server but only an [`axum::Router`](https://docs.rs/axum/latest/axum/struct.Router.html) that can be integrated into a bigger HTTP server
  (remind, Twirp uses regular HTTP(S)).
- [`twurst-error`](./error) provides the `TwirpError` type (think [`tonic::Status`](https://docs.rs/tonic/latest/tonic/struct.Status.html) but for Twirp).
  It is reexported by the other crates, and you should not need to depend on it directly.

Client and server examples are provided in the `example` directory.
`example/js-client` provides an example of a naive JS client.

Support for gRPC is also provided behind the `grpc` feature in `twurst-build` and `twurst-server`.
It allows to easily serve both Twirp and gRPC.
`example/server` provides an example.

For more detailed documentation see the [client](./client) and the [server](./server) READMEs.

## Getting started

1. Read the `example/client` and `example/server` directories.
2. Copy/paste and adapt the code if you want to build a server or a client.
3. Read the [client](./client) and/or the [server](./server) READMEs for more documentation.

## Differences with other Twirp implementations
- [`prost_twirp`](https://docs.rs/prost-twirp) does not support JSON, Twirp 7 and last commit was on January 2023.
- [`twirp-rs`](https://github.com/github/twirp-rs) does not allow any Tower `Service` in the client, has a more restricted JSON support based on `prost-wkt` and not on `prost-reflect` leading to eg. troubles with [`oneOf` support](https://github.com/fdeantoni/prost-wkt?tab=readme-ov-file#oneof-types) or does not support custom fields based on Axum extractors added to the server generated methods.

## License

Copyright 2024 Helsing GmbH

Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License.
You may obtain a copy of the License at <http://www.apache.org/licenses/LICENSE-2.0>

Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and limitations under the License.
