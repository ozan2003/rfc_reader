# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.2] - 2025-08-06

### Removed

- Remove `rustls` dependency from [Cargo.toml](Cargo.toml) to use only native-tls for ureq

## [0.4.1] - 2025-08-03

### Changed

- Replace `return Err(anyhow!(...));` with `bail!(...)` cleaner code in [main.rs](src/main.rs)
and [cache.rs](src/cache.rs)

## [0.4.0] - 2025-07-30

### Added

- Added a minimum size for the application in [app.rs](src/ui/app.rs)
- Added a message to the user when the terminal is too small in [app.rs](src/ui/app.rs)

## [0.3.1] - 2025-07-30

### Changed

- Moved `tempfile` to dev dependencies.

## [0.3.0] - 2025-07-29

### Added

- Refactored `RfcClient` to use `native-tls` in [client.rs](src/client.rs)
- Added timeout parameter to `RfcClient` in [client.rs](src/client.rs)

## [0.2.4] - 2025-07-29

### Changed

- Update lint configuration and adjust thread count for builds

## [0.2.3] - 2025-07-15

### Changed

- Return `Result` instead of `Option` for better error handling in [cache.rs](src/cache.rs)

## [0.2.2] - 2025-07-13

### Changed

- Use binary search instead of relying on `HashSet` in [app.rs](src/ui/app.rs)

## [0.2.1] - 2025-07-13

### Changed

- update lint configuration in [Cargo.toml](Cargo.toml)

## [0.2.0] - 2025-07-11

### Added

- Show the version by a `-v` flag

## [0.1.2] - 2025-07-11

### Fixed

- Used `Option` to avoid confusing sentinels in [app.rs](src/ui/app.rs)

## [0.1.1] - 2025-07-11

### Added

- Early return pattern for search matches to reduce code complexity in [app.rs](src/ui/app.rs)

### Changed

- Refactored line highlighting logic in [app.rs](src/ui/app.rs)
- Improved comments in [app.rs](src/ui/app.rs) for clarity and consistency

## [0.1.0] - Initial Release

### Added

- Core displaying, fetching and caching functionalities
