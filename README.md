# TESTAR evidence for Rust

A standalone Rust library that validates, saves, and reads portable GUI execution evidence according to [TESTAR evidence specifications v0.3.0](https://github.com/ferpasri/testar-evidence-specs/releases/tag/v0.3.0).

Adapters supply test runs, test sequences, observations, recorded states, widgets, properties, available actions, action executions, recorded transitions, artifacts, and oracle results. The library preserves their reported meaning and checks references and lifecycle ordering before writing. Consumers can inspect traces or apply abstraction rules separately.

## Build and test

Use Rust 1.85 or later. From this repository:

```sh
cargo build --locked
cargo run --locked -p xtask -- prepare-conformance
cargo test --locked --workspace
```

Preparation selects the newest published specs release, including prereleases, and downloads its conformance files into ignored `target/conformance/`. It records the selected release and file checksums. Tests verify that cache and use it offline. Repeat preparation when checking a newer release, including in CI.

## Import the library

```toml
[dependencies]
testar-evidence = { path = "../testar-evidence-rust", version = "0.3.0" }
```

The public API is exported by `testar_evidence`. The format uses `schema_version: 3`. The `0.3` release series follows the specs; patch releases can advance independently.

## Recording and reading

`ReconstructedEvidence` holds recorded facts and relationships in memory for validation and inspection.

`JsonlStore::create(directory, manifest)` starts a fresh evidence collection. Report `EvidenceEvent` values through `record`. A test run can contain several test sequences; each sequence has its own observation and execution history. An action's normal return is recorded separately from its resulting observation and recorded transition.

`EvidenceReader::read(directory)` reads and validates evidence without opening a writer. `JsonlStore::open(directory)` reopens it for continued recording. `store.evidence()` exposes validated records through shared references. Lookup methods include `test_sequences_for_run`, `observations_for_sequence`, `executions_for_sequence`, `incoming`, and `outgoing`.

The portable directory contains `manifest.json`, `evidence.jsonl`, and referenced files in `artifacts/`. Capture descriptions, source properties, aliases, omissions, optional metadata, and oracle severities remain as supplied. Recorded oracle status and optional source severity describe separate information.

## Example

[record_evidence.rs](examples/record_evidence.rs) records a demonstration test run and sequence, two observations, an available action, a completed recorded transition, a PNG, and a late oracle result. Comments explain each recording step.

```sh
cargo run --locked --example record_evidence -- target/traces demo submit-form "Example script"
```

Arguments select the parent directory, SUT label, optional test objective, and optional origin. The example reserves a timestamped child directory such as `demo_submit-form_trace-2026-10-22_15-30-12-123`; collisions add a numeric suffix. Original metadata descriptions are preserved, while filename labels are sanitized. These labels describe demonstration evidence rather than launch an application.

## Shared conformance data

Each released case supplies an input trace: manifest, JSONL events, and referenced artifacts. Reader tests reconstruct its records; producer tests report the same events through the public API and reopen the output. Both compare the normalized result with `expected.json` and verify complete accepted event payloads, artifact bytes, identical retries, and rejection without mutation.

Cases are generic and discovered from the prepared specs release. The [conformance explanation](https://github.com/ferpasri/testar-evidence-specs/blob/v0.3.0/conformance/README.md) distinguishes input trace, reconstructed evidence, and expected result.

See [SPEC](docs/SPEC.md) for implemented behavior and [TODO](TODO.md) for future work.
