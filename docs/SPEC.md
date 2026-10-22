# Rust evidence library specification

The library validates and saves what a testing tool observed, which actions it discovered, what it executed, and the outcomes reported by its test oracles. An adapter translates tool data into these generic records.

The [shared model semantics](https://github.com/ferpasri/testar-evidence-specs/blob/main/docs/model-semantics.md) define the evidence meaning. This document describes the Rust API and its implemented behavior.

## Records and properties

| Type | What it represents |
| --- | --- |
| `Run` | One testing session and its producer information. |
| `Observation` | One visit, with its sequence position, timestamp, state, candidates, and artifact references. |
| `ConcreteState` | Captured state properties and an ordered widget hierarchy. |
| `Widget` | A captured interface element, its local ID, optional parent, and properties. |
| `PropertyMap` | Named JSON values, including nested data and explicit null values. |
| `ActionDefinition` | An action's stable ID, kind, and descriptive properties. |
| `ActionCandidate` | An action discovered at one observation, optionally bound to a widget. |
| `Execution` | One selection, including its source, candidate binding, and actual parameters. |
| `CheckResult` | The outcome and supporting information from one test oracle evaluation. |
| `CheckRule` | The condition's ID, optional version, and optional parameters. |
| `CheckTarget` | The run, visit, widget, property, execution, or sequence being evaluated. |
| `CheckReading` | A reported JSON value, established absence, or unknown information. |
| `ExpectedValue` | An omitted expectation or a supplied JSON value, including null. |
| `ArtifactReference` | A local evidence file, its media type, SHA-256 hash, and optional dimensions. |

Every visit has its own observation ID. Identical snapshot content can share a snapshot ID while the visits and execution history remain separate. Snapshot IDs retain fixed content.

Widget IDs are local to their snapshot. Parents refer to widgets in that snapshot, and the hierarchy is acyclic. Several root widgets are allowed.

IDs and property names are nonblank strings without control characters. The producer supplies IDs that distinguish its records across runs and sources.

## Capture and property meanings

Capture scope records complete, filtered, or unavailable widget information. A complete widget tree can still contain only selected properties. The capture description identifies which properties are collected and, if known, explains exclusions or limitations.

`PropertyMap` preserves supplied JSON values, including nested objects, lists, empty strings, and explicit nulls. Missing keys remain distinct from recorded null values. Optional `evidence.capture`, `evidence.omitted_properties`, and `evidence.property_aliases` values use these maps as described in the [shared capture conventions](https://github.com/ferpasri/testar-evidence-specs/blob/main/docs/capture-evidence.md).

The adapter supplies properties with their source meanings. State-level `ui.title` is the application's page, screen, or window title; `ui.url` is the full captured page address. A widget's actual title attribute, visible text, and accessible name are distinct properties. A common property name requires a verified source meaning. The library retains the supplied names and values.

A declared alias gives another name to the same captured property within one state or widget record. The adapter declares that relationship from the source definition; equal values alone do not establish it. Check results retain the exact property key evaluated.

## Discovered actions and exploration

An observation retains its candidates even when they are never selected. Candidate scope records whether the producer reports generated actions, actions marked eligible under its selection rules, or an unknown scope.

For example, a tool can report ten candidate actions and execute one. The observation keeps all ten candidates; the execution identifies the selected action and its widget binding.

Consumers can identify candidates with no recorded selection from that observation. Selection, accepted submission, and a resulting observation are separate stages. Comparing exploration across different visits requires a policy for matching observations and actions. Candidate lists describe what the producer reported, so their counts reflect that scope.

## Events and execution order

Each `EvidenceEvent` describes one reported event together with its stable identity and format version.

| Event | Recorded behavior |
| --- | --- |
| `run_started` | Registers the run and its origin. |
| `action_defined` | Registers an action definition whose ID retains the same content. |
| `observed` | Records a visit, with sequence numbers increasing within its run. |
| `action_selected` | Selects a matching action/target pair from the latest observation. |
| `action_submitted` | Records whether the testing tool accepted or rejected execution of the action, with any reported error. |
| `action_result` | Links an accepted execution to a later observation in the same run. |
| `checked` | Records one test oracle evaluation's outcome and supporting evidence. |
| `run_finished` | Records the ending reason and marks outstanding executions incomplete. |

A run starts before its observations, and action definitions precede their candidates. A run can have one execution awaiting submission or a result. Rejected submission permits another selection.

`StateModel::validate` checks an event without applying it. `StateModel::apply` validates before updating the model. An identical event retry returns `false`. Different content using an existing event ID raises a conflict.

`ExecutionRecord` exposes the selected action and its status: selected, submitted, rejected, observed, or incomplete. A completed action connects the observation before it to its resulting observation, forming a state-action-state execution trace represented by an `ExecutionLink`. Submission acceptance records what happened at that stage; test oracle results describe evaluated conditions separately.

A rejected submission reports that the testing tool's runner, driver, or execution wrapper could not accept or execute the action. Its reported error is retained, and that execution has no completed result observation. An executed action that produces an application message such as "Incorrect password" has a result observation containing that response.

## Test oracle results

A test oracle applies a rule to a target. The adapter records one evaluation as a `CheckResult`, with its name, rule, target, status, and supporting details. Captured properties and recorded evaluations remain separate evidence.

Targets can identify a run, observation, state property, widget, widget property, execution, or inclusive observation sequence. Referenced records belong to the result's run. A widget belongs to its referenced observation. A property's presence can itself be evaluated, so the target may name an absent property. Property targets use exact top-level keys: `ui.alt` is one literal key, and an object stored under `source.attributes` is one property. A result about an entire observation or run describes that target; individual property coverage comes from results naming the properties.

Passed and failed report whether the condition was satisfied. Pending means the evaluation had no definite result when recorded; unknown means the oracle could not determine an outcome. Error records an evaluation problem. Finishing a run retains these reported statuses. Results may arrive after the run finishes.

Each reported result remains a separate record. Optional `evidence.assessment` information in `details` preserves the oracle's reporting phase, temporal meaning, and supplied termination reason. An earlier pending result retains its recorded status when a later assessment arrives.

`ExpectedValue::Unspecified` omits an expectation. `ExpectedValue::Value` preserves the supplied JSON value, including null. An actual reading can contain a value, established absence, or unknown information; omitting the reading means none was supplied. Optional rule versions and parameter maps retain whether they were supplied.

An invalid target returns `Error::InvalidCheckTarget`, identifying the affected field, such as `target.widget`. Rejection preserves the model and stored log.

## Reading and inspecting recorded evidence

The library keeps track of earlier records so it can validate new events. Reading saved events builds those same records and links in memory for inspection or continued recording. Simple lookup methods return runs, visits, snapshots, actions, candidates, executions, links, and check results. Visits within a run are ordered by sequence; executions and links follow selection order; evaluations follow recording order. General run, observation, action, and snapshot lists are sorted by ID.

`incoming` and `outgoing` find completed links for a visit. `observations_for_snapshot` finds the visits referencing a snapshot. `observations_for_run` returns a run's ordered visits.

The store exposes its model through a shared reference. Updating the standalone model or recording events requires mutable access. Observation sequence numbers and timestamps use `u64`; properties use JSON values.

## Example trace directories

The example takes a parent directory followed by optional SUT and test objective labels. It reserves a new directory named `<sut>_<objective>_trace-<local timestamp>`, including milliseconds. Labels are made safe for filenames and limited to 32 characters each. A collision adds `-2`, `-3`, and so on. Each run also receives distinct model and run identities. These labels describe demonstration records; they do not launch an application or choose a test script. The storage API continues accepting an explicit directory.

## Storage

`JsonlStore` manages a directory containing `manifest.json`, `evidence.jsonl`, and referenced `artifacts/`. `create` creates a collection; `open` validates and replays it for continued recording.

An exclusive file lock allows one writer per directory. `record` validates the event and its artifacts, appends one UTF-8 JSON line, then synchronizes it to storage before acknowledging the write. A persistence failure invalidates the writer until it is reopened.

`put_artifact` identifies supplied bytes by SHA-256 and reuses identical content. PNG paths end in `.png`, JPEG paths in `.jpeg`, and other media paths in `.bin`. The caller supplies the media type and optional dimensions. Referenced files are checked by hash when recording or reopening. Relative artifact paths keep evidence portable when the whole directory is moved.

Each JSONL record ends with a newline. `recover_tail` acquires the writer lock, validates the complete preceding events and artifacts, and removes an unfinished final fragment. Complete invalid records cause recovery to fail without changing the log.

## Verification

The [README](../README.md#shared-conformance-data) describes preparation of the newest published specs cases and the local cache. Tests discover released cases and verify their cached checksums.

Reader tests compare accepted records or rejected targets with the released expectations. Producer tests write equivalent evidence through the public API, compare the complete persisted event payloads and artifact bytes, check identical retries and rejection behavior, and reopen the accepted evidence. Released cases exercise capture descriptions and aliases, unselected candidates, scripted executions, rejected submissions, and temporal reports alongside targeted property evaluations.

Unit tests cover record validation, candidate preservation, repeated visits, execution ordering, evaluation targets, event retries, writer locking, artifact integrity, and recovery.
