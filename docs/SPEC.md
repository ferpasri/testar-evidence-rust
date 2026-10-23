# Rust evidence library specification

The library implements [schema 3](https://github.com/ferpasri/testar-evidence-specs/blob/v0.3.0/docs/evidence-format.md). Adapters report facts from their tools; validation checks records and references before storage. The shared [terminology](https://github.com/ferpasri/testar-evidence-specs/blob/v0.3.0/docs/terminology.md) defines their meaning.

## Components

| Type | Responsibility |
| --- | --- |
| `TestRun` / `TestSequence` | Describe an invocation and its independent sequence histories. |
| `Observation` / `RecordedState` / `Widget` | Preserve captured moments, state content, widget hierarchy, properties, and limits. |
| `ActionDescription` / `AvailableAction` | Describe operations and reported choices, including choices never executed. |
| `ActionExecution` / `ActionExecutionResult` / `RecordedTransition` | Preserve selection, actual parameters, the runner's outcome, and a resulting-observation connection. |
| `Oracle` / `OracleResult` / `OracleTarget` | Identify checks and preserve their targets and outcomes. |
| `ObservedValue` / `OracleSeverity` | Preserve reported values, established absence or uncertainty, and optional source-defined importance. |
| `EvidenceEvent` / `Event` | Carry event identity/version and a typed event payload. |
| `ReconstructedEvidence` | Retain recorded facts and relationships in memory for validation and inspection. |
| `JsonlStore` / `EvidenceReader` | Write validated evidence and read it back. |

## Histories and transitions

A test run starts before its sequences. Sequence indexes increase within the run; observation indexes increase independently within each sequence. Sequences can be interleaved without mixing their executions. A run finishes after all its started sequences finish. Repeated captures remain separate observations, including when recorded-state content is shared.

Selection refers to the latest observation and a reported available-action/widget pair. Each sequence has at most one execution awaiting its execution result or resulting observation. A returned call awaits a later observation in the same sequence. An error retains its message and permits another selection; partial effects can be captured without inventing a completed transition. Finishing a sequence makes unfinished executions incomplete.

Execution status is `awaiting_execution_result`, `awaiting_resulting_observation`, `failed`, `completed`, or `incomplete`. Recorded transitions retain transition-recording order; executions retain selection order. An observation-order link is a viewer relationship rather than an action inferred by this library.

`validate` checks without mutation. `apply` validates before changing records. Identical event retries return false; conflicting event IDs or changed state/description content are rejected. A failed validation leaves history and the saved log intact. Domain errors expose a stable code and field when applicable.

## Properties and oracle results

Properties preserve JSON values, arbitrary-precision integers and decimals, nested objects, and explicit null. Missing fields remain distinct from supplied null where the contract permits both. Typed indexes and timestamps use `u64`. Widget parents remain inside one state and form an acyclic hierarchy.

Oracle targets cover a run, sequence, observation, widget, property, action execution, or inclusive observation range. Sequence-level references belong to the declared sequence and parent run. A whole-run target omits its sequence reference. A property target can name an absent property. Broad targets do not establish individual property coverage.

Expected values distinguish omission from supplied null. Observed values distinguish value, established absence, and unknown information. Oracle severity is optional and preserves its source label and optional numeric value without imposing a universal range or changing oracle status. Each result remains separate, including repeated or late results after histories end.

Optional assessment metadata preserves temporal semantics, reporting phase, and the corresponding ending reason. Sequence and run phases refer to the appropriate lifecycle. Ending a history does not change pending oracle statuses.

## Metadata

Run and sequence properties preserve supplied origin, test objective, adapter, and adapter-version descriptions. Unicode and spacing are retained; omitted descriptions remain absent. Sequence metadata remains separate from parent-run metadata. Capture descriptions, property aliases, and known omissions are retained as supplied; adapters establish source-property meanings.

## Persistence

The store holds an exclusive writer lock. Creation refuses existing evidence files. Each successful record is validated with referenced artifact hashes, appended as newline-terminated UTF-8 JSON, and synchronized before acknowledgment. A persistence failure invalidates the writer until reopening. A read-only reader rebuilds evidence without opening a writer.

Artifacts use SHA-256 content identities and portable relative paths. Identical bytes reuse stored files. PNG, JPEG, and other media use `.png`, `.jpeg`, and `.bin` paths. Hash verification detects missing or changed bytes when recording or reading. The existing path checks reject traversal and platform-specific unsafe names.

Opening an unfinished final line fails. Explicit `recover_tail` validates the complete prefix and artifacts before removing only the unfinished fragment; complete invalid records are retained and reported.

## Verification

Reader and producer tests use the newest prepared specs release and verify its checksums. They compare all released expected summaries and accepted payloads, rejection prefixes, retries, reopen behavior, and artifacts. Unit tests cover immutable state identities, hierarchy, interleaved sequences, transitions, exact numbers, oracle targets/severity, writer locks, failed writes, and recovery.
