## [0.0.2] - 2024-11-28

### Changed
- Server: do not insert the `fallback` directly in `axum` `Router`s,
  but provide `twirp_fallback` and `grpc_fallback` to let user set them easily.
- Build: do not shuffle the order of extractors.

## [0.0.1] - 2024-11-27

### Added
- First release for `twurst-error`, `twurst-client`, `twurst-server` and `twurst-build`
