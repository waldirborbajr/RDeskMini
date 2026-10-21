# Changelog

Notable changes to RDeskMini are documented here.

## Unreleased

### Added

- Host approval for incoming remote-control requests. Requests expire after 30 seconds.
- SHA-256 certificate fingerprint display and pin verification for client TLS connections.
- US English (`US_ENG`), Brazilian Portuguese (`pt_BR`), and Spanish (`Español`) interface translations, with locale-based startup detection.
- Fingerprint parsing and locale detection tests.

### Security

- TLS clients no longer accept arbitrary server certificates. Users must verify the host fingerprint through a separate trusted channel.

### Changed

- Updated setup, security, and language documentation to describe the current behavior.
