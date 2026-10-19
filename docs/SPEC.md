# State model specification

The library records what a testing tool observes and which actions it executes. It keeps this evidence as a sequence of visits connected by completed actions. Applications can save the sequence, reopen it, and query its states, widgets, actions, check results, and screenshots.

## What the model records

| Record | Meaning |
| --- | --- |
| `Run` | One test run, including the tool that produced it and identifying properties. |
| `Observation` | One visit during a run: its position in the sequence, timestamp, captured state, available actions, and evidence references. |
| `ConcreteState` | The snapshot captured during a visit, containing state properties and widgets. |
| `Widget` | A captured interface element with properties and an optional parent widget. |
| `PropertyMap` | Named JSON values describing a record, such as text, a role, a measurement, or structured data. |
| `ActionDefinition` | The identity, kind, and descriptive properties of an action. |
| `ActionCandidate` | An action offered at one observation, optionally associated with a widget. |
| `Execution` | One selected action, its source observation, target widget, and execution parameters. |
| `CheckResult` | The recorded outcome and evidence from an evaluation performed by a test oracle. |
| `CheckRule` | The condition's ID, optional version, and optional parameters. |
| `CheckTarget` | The observation, widget, property, execution, run, or sequence being evaluated. |
| `CheckReading` | A reported JSON value, established absence, or unknown information. |
| `ExpectedValue` | An omitted expectation or a supplied JSON value, including null. |

Each observation represents a separate visit. For example, observing the same screen twice produces two observations. Both may reference the same concrete snapshot ID when their snapshot content is identical. This preserves the order of visits and the actions performed between them.

Widgets retain their supplied order and parent relationships. A snapshot may contain several widgets with no parent. Widget IDs are unique within a snapshot; each parent belongs to that snapshot, and following parent relationships leads to a root widget.

Properties preserve JSON values, including the difference between an absent property and an explicitly supplied null. Tool-specific properties can use a prefix identifying their source.

An observation also describes the available information. `CaptureScope` records whether widget capture is complete, filtered, or unavailable. `CandidateScope` records whether the action list has unknown status, was generated, or was marked eligible by the producer.

IDs are nonblank strings. The producer chooses IDs that distinguish records across its runs and other sources.

## Recording evaluations

A test oracle applies a rule to a target. One application is an evaluation; `CheckResult` stores its outcome and evidence. The adapter supplies these results, and the library validates their structure and references.

`CheckTarget` can identify a run, observation, state property, widget, widget property, execution, or range of observations. Referenced records belong to the result's run. A widget belongs to the identified observation. A sequence includes its endpoints, with the ending observation at or after the start.

A property target may name an absent property: the test oracle can evaluate whether that property exists. Each result describes the evaluation actually reported, separately from the properties captured in the snapshot.

`CheckReading` distinguishes a JSON value (including null), established absence, and unknown information. An omitted `actual` field means no reading was supplied. `ExpectedValue::Unspecified` omits `expected`; `ExpectedValue::Value` preserves the supplied expectation, including an explicit JSON null. Optional rule versions and parameter maps also preserve whether they were supplied.

Invalid target references return `Error::InvalidCheckTarget`, identifying the affected field, before the result changes the model or is appended to the log.

## How a run is recorded

The tool sends events in the order they happen:

1. `run_started` registers the run and its origin.
2. `action_defined` registers an action definition. An existing action ID retains the same definition.
3. `observed` records a visit. Observation sequence numbers increase within each run.
4. `action_selected` selects a candidate from the latest observation. The run can have one action awaiting completion at a time.
5. `action_submitted` records whether the submission was accepted or rejected.
6. `action_result` connects an accepted execution to a later observation in the same run.
7. `run_finished` records the ending reason and marks any pending execution as incomplete.

A `checked` event records a check result. Passed and failed describe whether the condition was satisfied. Error records a problem during evaluation; unknown means no outcome could be determined, and pending means evaluation is awaiting completion. Results retain their targets and may arrive after a run finishes. Submission acceptance and check results are recorded separately.

Every event is wrapped in an `EventEnvelope` containing its schema version, event ID, kind, and data. `StateModel::validate` checks the event against the current model. `StateModel::apply` performs that validation and updates the model.

An event ID identifies one fixed payload. Sending an identical event again returns `false` and preserves the existing evidence. Sending different content under that ID produces a conflict.

## Following the execution history

`ExecutionRecord` exposes the selected action, its current status, and its result observation when available. Status progresses from selected to submitted or rejected. Linking a submitted action to its resulting observation gives it the observed status. Pending actions become incomplete when their run finishes.

`ExecutionLink` connects the source and result observations of one completed execution, in the recorded direction. Each completed execution has its own link, including repeated actions between visits.

`StateModel` provides queries for runs, observations, snapshots, actions, executions, candidates, and check results. `incoming` and `outgoing` find links for a particular observation. `observations_for_snapshot` finds all visits referencing one snapshot.

`observations_for_run` returns visits in sequence order. Execution queries and links follow action selection order. Check results follow recording order. General observation, snapshot, run, and action lists are sorted by ID.

## Saving and reopening evidence

`JsonlStore` saves a model directory containing:

| File or directory | Contents |
| --- | --- |
| `manifest.json` | Model identity and schema version. |
| `evidence.jsonl` | The event history, with one JSON event per line. |
| `artifacts/` | Evidence files referenced by observations. |

An exclusive writer lock gives one writer access to the directory at a time. `record` validates each event before appending it, then flushes and synchronizes the file before acknowledging the write. Opening the directory validates and replays the event history to rebuild the model in memory.

Artifacts have a SHA-256 content identifier, a relative file path, a media type, and optional dimensions supplied by the caller. PNG files receive a `.png` extension, JPEG files `.jpeg`, and other media `.bin`. The store checks referenced paths and file hashes when recording and reopening evidence. Moving the whole model directory keeps the event history and its evidence together.

If an interrupted write leaves an unfinished final line, `recover_tail` validates the preceding events and removes that unfinished fragment. A failed write requires reopening the store before recording more events.

## Checking the implementation against the specification

`cargo run -p xtask -- prepare-conformance` selects the newest published specs release, including prereleases, and prepares its conformance files under `target/conformance/`. The selected release and file checksums are recorded in `current.json` inside that directory.

Tests verify the prepared file checksums and discover the released case directories. Accepted cases test both reading the supplied evidence and producing equivalent evidence through the public Rust API, including properties, check results, and artifact bytes. Rejected cases compare the invalid event and target field with the expected error, verify the accepted prefix, and reopen that prefix. Rejection preserves the model and saved evidence. Once prepared, this data is available for offline tests.

Additional Rust tests cover record validation, repeated visits, execution order, event retries, storage recovery, writer locking, artifact integrity, and moving a saved model.
