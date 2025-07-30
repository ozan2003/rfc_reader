# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2025-07-30

### Added

- Added a minimum size for the application
- Added a message to the user when the terminal is too small

## [0.3.1] - 2025-07-30

### Changed

- Moved `tempfile` to dev dependencies.

## [0.3.0] - 2025-07-29

### Added

- Refactored `RfcClient` to use `native-tls`
- Added timeout parameter to `RfcClient`

## [0.2.4] - 2025-07-29

### Changed

- Update lint configuration and adjust thread count for builds

## [0.2.3] - 2025-07-15

### Changed

- Return `Result` instead of `Option` for better error handling

## [0.2.2] - 2025-07-13

### Changed

- Use binary search instead of relying on `HashSet`

## [0.2.1] - 2025-07-13

### Changed

- update lint configuration in Cargo files

## [0.2.0] - 2025-07-11

### Added

- Show the version by a `-v` flag

## [0.1.2] - 2025-07-11

### Fixed

- Used `Option` to avoid confusing sentinels

## [0.1.1] - 2025-07-11

### Added

- Early return pattern for search matches to reduce code complexity

### Changed

- Refactored line highlighting logic
- Improved comments in app.rs for clarity and consistency

## [0.1.0] - Initial Release

### Added

- Core displaying, fetching and caching functionalities
