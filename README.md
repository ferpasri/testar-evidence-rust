# testar-state-model

A standalone Rust library for recording concrete test evidence in the generic format defined by `testar-state-model-specs`.

A tool-specific adapter supplies runs, observations, widgets, properties, actions, executions, and evidence such as screenshots. This library validates those records, saves a local model directory, and exposes queries over the recorded execution history. It has no dependency on a particular testing tool or user-interface technology.

Each observation remains a separate visit. Completed executions connect their source and result observations in the recorded direction. The library does not merge similar screens or require an abstraction policy. Other applications can read the portable JSONL and artifacts to build reports, central storage, or derived models.

## Build and test

Run these commands in this repository:

```sh
cargo build --locked
cargo run --locked -p xtask -- prepare-conformance
cargo test --locked --workspace
```

`xtask` is a separate Rust package containing development tooling. It downloads and extracts the shared conformance data used by the tests.

Create and reopen a small model containing one completed execution and a PNG:

```sh
cargo run --locked --example local_model -- target/example-model
```

Use a new output directory for each example run.

## Shared conformance data

The setup command queries the public [specs releases](https://github.com/ferpasri/testar-state-model-specs/releases) on every run. It selects the newest publication by `published_at`, including prereleases and excluding drafts and unpublished entries. Ties use the release ID. This follows publication order rather than the highest version number.

Setup prints the selected release, downloads its source archive over HTTPS, and extracts its conformance files into this repository's ignored `target/conformance/` directory. It records the selected release and locally computed SHA-256 checksums in `target/conformance/current.json`. Those checksums detect later changes to cached files; they are not independent publisher signatures.

Archive paths and extraction sizes are checked. Incomplete case directories are rejected. A failed download or extraction returns an error and leaves the previous prepared selection intact. Setup never silently substitutes an older release when checking for the latest one fails.

After successful setup, tests use the prepared data offline and verify its checksums. Running tests alone does not check for newer releases. Repeat the setup command when checking compatibility with the latest specs, including before tests in CI. A newer release can change the test outcome without a Rust source change; the selected release is recorded to explain the result.

Conformance tests discover every case directory. Reader and producer tests compare accepted records, and rejected cases verify the error, unchanged model/log, and accepted prefix after reopening. Check results preserve rule identities, concrete targets, statuses, optional expectations, and value/absent/unknown readings. Rust regression tests also use the initial-observation case. The example PNG remains a local test asset.

Building the library and running its local example do not require downloaded specs data. See [SPEC.md](docs/SPEC.md) for implemented behavior and [TODO.md](TODO.md) for remaining work.
