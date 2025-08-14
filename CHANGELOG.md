## [0.3.0] - 2025-08-14

### Changed
- upgrade to `prost` 0.14, `prost-reflect` 0.16 and `tonic` 0.14
- drop `twurst-error` features `tonic-012` and `tonic-013` in favor of `tonic-014`
- add dependency on `tonic-prost` (required now to use Tonic with Prost). It is needed explicitly when using `twurst-server` with the `grpc` feature
- bump MSRV to 1.85

## [0.2.2] - 2025-05-07

### Changed
- `prost-reflect`: now supporting both 0.14 and 0.15
- `tokio`: bump from 1.44.1 to 1.44.2

## [0.2.1] - 2025-03-26

### Changed
- Error: add feature `tonic-013` to support Tonic 0.13
- Server: use Tonic 0.13 instead of Tonic 0.12

## [0.2.0] - 2025-01-24

### Changed
- axum upgraded from 0.7 to 0.8
- feature `axum-07` as been removed in favor of feature `axum-08`
- converting back a `TwirpError` to `tonic::Status` when it as been built from a `tonic::Status` keep the status details
- `TwirpError::invalid_argument` now only takes a single error message argument

## [0.1.0] - 2024-12-26

### Added
- Error: `From` implementation from/to Tonic `Code` and `Status`.

### Changed
- Build: Removes the "grpc" feature and make it a build option named `with_grpc`.

## [0.0.3] - 2024-12-19

### Added
- Server: Streaming requests and responses in the gRPC router (but not in the Twirp router).
- Server: Nice error when calling streaming methods with Twirp.

### Changed
- Build: Streaming methods are silently ignored instead of failing the build.

## [0.0.2] - 2024-11-28

### Changed
- Server: do not insert the `fallback` directly in `axum` `Router`s,
  but provide `twirp_fallback` and `grpc_fallback` to let user set them easily.
- Build: do not shuffle the order of extractors.

## [0.0.1] - 2024-11-27

### Added
- First release for `twurst-error`, `twurst-client`, `twurst-server` and `twurst-build`
