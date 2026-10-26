# Recording example

Run these commands from the repository root.

The example writes demonstration evidence through the public API. It illustrates recording boundaries and metadata; the [implementation specification](SPEC.md) defines behavior.

[record_evidence.rs](../examples/record_evidence.rs) records a demonstration test run and sequence, two observations, an available action, a completed recorded transition, a PNG, and a late oracle result. Comments explain each recording step.

```sh
cargo run --locked --example record_evidence -- target/traces demo submit-form "Example script"
```

Arguments select the parent directory, SUT label, optional test objective, and optional origin. The example reserves a timestamped child directory such as `demo_submit-form_trace-2026-10-22_15-30-12-123`; collisions add a numeric suffix. Original metadata descriptions are preserved, while filename labels are sanitized. These labels describe demonstration evidence rather than launch an application.
