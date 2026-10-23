# TESTAR evidence for Rust

A standalone Rust library for validating and saving portable test execution evidence in the shared [TESTAR evidence format](https://github.com/ferpasri/testar-evidence-specs).

Tool adapters supply runs, observed states, widgets, properties, discovered actions, executions, screenshots, and test oracle results. The library checks that the records and their references are valid, then saves them as portable evidence files.

Every visit remains a separate observation. Each completed action connects the observation before the action to its resulting observation, forming a state-action-state execution trace. Discovered candidates remain available even when never selected. Consumers can use the saved evidence to display traces or build derived models.

Captured properties and oracle evaluations are separate evidence. Check results identify what was evaluated, the rule, its outcome, and any reported reading.

The adapter supplies captured values and describes which widgets and properties it collected. The library preserves optional capture descriptions, known omissions, and declared property aliases. The [shared capture conventions](https://github.com/ferpasri/testar-evidence-specs/blob/main/docs/capture-evidence.md) explain their meaning.

## Release versions

Releases use `0.X.Y`. The `0.X` release series is shared with the specs and related state-model projects. Each project increments `Y` independently for fixes and improvements within that series. Different patch versions can coexist within the same release series.

## Build and test

Use Rust 1.85 or later. Run these commands from this repository:

```sh
cargo build --locked
cargo run --locked -p xtask -- prepare-conformance
cargo test --locked --workspace
```

`xtask` is the Rust package containing the conformance preparation tooling.

## Import the library

A consuming Cargo project can declare a local dependency:

```toml
[dependencies]
testar-evidence = { path = "../testar-evidence-rust", version = "0.2.2" }
```

The public API is exported by `testar_evidence`.

## Example

[local_model.rs](examples/local_model.rs) records and reopens a demonstration model containing one completed execution and a PNG. Comments explain the recording steps.

```sh
cargo run --locked --example local_model -- target/traces demo submit-form
```

Arguments select the parent directory, SUT label, optional test objective, and optional origin. The fourth argument can name the source, for example `"Example script"`. Without a test objective, the filename uses `run` and the objective property is omitted. Each run reserves its own directory, such as `target/traces/demo_submit-form_trace-2026-10-22_15-30-12-123/`. A collision adds a numeric suffix. Each run also has distinct model and run identities. Labels are retained in run properties, while their filename versions are sanitized and limited to 32 characters each. The example uses demonstration records. Open a complete generated trace directory in the dashboard.

## Run metadata

Run properties support optional origin, test objective, adapter name, and adapter version using the [shared run metadata names](https://github.com/ferpasri/testar-evidence-specs/blob/main/docs/run-metadata.md). Supplied descriptions keep their original text; omitted descriptions stay omitted. See [the implementation specification](docs/SPEC.md#run-metadata) for their use.

## Reading and inspecting saved traces

The library remembers earlier records while writing so it can check new ones. For example, a selected action must refer to an existing observation and one of its candidates. `JsonlStore::open(directory)` reads and validates saved records before continuing a recording. `observations_for_run`, `executions_for_run`, and `outgoing` provide simple lookups over those records. Reading and inspection support recording; consumers decide how to group visits into derived models.

## Shared conformance data

Each case supplies an input trace: `manifest.json`, `evidence.jsonl` (one recorded event per line), and referenced artifacts. The library reads the events to reconstruct a model in memory. Tests compare a summary of that model with `expected.json`, which describes the expected records or rejection. Tests also verify complete accepted event contents and artifact bytes. The reconstructed model preserves concrete evidence; it is separate from any later abstract or merged model.

See the shared [input trace, reconstructed model, and expected result explanation](https://github.com/ferpasri/testar-evidence-specs/blob/main/conformance/README.md#input-trace-reconstructed-model-and-expected-result) for the file roles, an event-sequence example, and reader/producer test behavior.

The preparation command queries the public [specs releases](https://github.com/ferpasri/testar-evidence-specs/releases). It selects the newest publication by `published_at`, including prereleases and excluding draft or unpublished entries. Ties use the release ID. Publication order determines the selection.

Preparation downloads the release archive over HTTPS and extracts its conformance files into the ignored `target/conformance/` directory. `current.json` records the selected release and locally computed SHA-256 checksums. Tests verify those checksums to detect changes to cached files.

Archive paths, extraction sizes, and case completeness are checked. Preparation reports a failure and leaves the previous prepared selection intact. A failed latest-release lookup is reported to the caller.

After successful preparation, tests use the cached cases offline. Running tests alone uses that prepared selection; repeat preparation to check compatibility with the latest release, including before tests in CI. Deleting `target/` removes both the cache and build output.

Tests discover every released case directory. Reader tests load supplied evidence. Producer tests record equivalent evidence through the public API and reopen it. Both compare the records and rejected-case expectations, preserve complete accepted event payloads and artifact bytes, and verify that rejection leaves the accepted evidence unchanged.

## Documents

- [Shared model semantics](https://github.com/ferpasri/testar-evidence-specs/blob/main/docs/model-semantics.md): the meaning of generic evidence.
- [Implementation specification](docs/SPEC.md): this library's API and implemented behavior.
- [TODO](TODO.md): planned work.
