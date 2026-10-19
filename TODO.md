# Remaining work

- Extend shared cases for repeated visits, incomplete/rejected executions, replay conflicts, capture limits, and artifact failures.
- Extend the conformance runner as specs introduce additional expectation formats and error codes.
- Reconnect the separate tool adapter to the concrete API and test a real execution through it.
- Establish format release/versioning and migration rules with the specs project before treating the format as stable.
- Add automated checks for supported platforms and document a release process.
- Prepare and publish the library on crates.io so other Rust projects can depend on released versions.
- Evaluate larger traces and persistence approaches when in-memory replay becomes a practical limit.

Optional abstraction, visual reports, and centralized merging can be built by consumers of the recorded evidence.
