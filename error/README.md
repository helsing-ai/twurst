Small library implementing the `TwirpError` struct.
Please don't use it directly but rely on `twurst-client` or `twurst-server`that re-export this type.

## Cargo features
- `serde` allows to (de)serialize the error using [Serde](https://serde.rs/) following the official Twirp serialization.
- `http` allows to convert between [`http::Response`](https://docs.rs/http/latest/http/response/struct.Response.html) objects and Twirp errors,
  properly deserializing the error if possible, or building an as good as possible equivalent if not.
- `axum-07` implements the [`axum::response::IntoResponse`](https://docs.rs/axum/0.7/axum/response/trait.IntoResponse.html) trait on `TwirpError`.
- `tonic-012` implements `From` conversions between `TwirpError`and [`tonic::Status`](https://docs.rs/tonic/0.12/tonic/struct.Status.html) in both directions.

## License

Copyright 2024 Helsing GmbH

Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with the License.
You may obtain a copy of the License at <http://www.apache.org/licenses/LICENSE-2.0>

Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and limitations under the License.
