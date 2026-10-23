use crate::event::validate_version;
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionExecutionStatus {
    AwaitingExecutionResult,
    AwaitingResultingObservation,
    Failed,
    Completed,
    Incomplete,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActionExecutionRecord {
    pub action_execution: ActionExecution,
    pub status: ActionExecutionStatus,
    pub execution_result: Option<ActionExecutionResult>,
    pub resulting_observation: Option<ObservationId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryStatus {
    Running,
    Finished { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RecordedTransition {
    pub action_execution: ActionExecutionId,
    pub test_sequence: TestSequenceId,
    pub source_observation: ObservationId,
    pub resulting_observation: ObservationId,
    pub action_description: ActionDescriptionId,
    pub target_widget: Option<WidgetId>,
}

/// Recorded facts and relationships held in memory for validation and inspection.
#[derive(Clone, Debug)]
pub struct ReconstructedEvidence {
    manifest: Manifest,
    test_runs: BTreeMap<TestRunId, (TestRun, HistoryStatus)>,
    test_sequences: BTreeMap<TestSequenceId, (TestSequence, HistoryStatus)>,
    recorded_states: BTreeMap<RecordedStateId, RecordedState>,
    observations: BTreeMap<ObservationId, Observation>,
    action_descriptions: BTreeMap<ActionDescriptionId, ActionDescription>,
    action_executions: BTreeMap<ActionExecutionId, ActionExecutionRecord>,
    execution_order: Vec<ActionExecutionId>,
    recorded_transitions: Vec<RecordedTransition>,
    oracle_results: Vec<OracleResult>,
    events: BTreeMap<EventId, EvidenceEvent>,
}

fn issue(
    code: &'static str,
    field: &'static str,
    message: impl Into<String>,
) -> Error {
    Error::Validation {
        code,
        field,
        message: message.into(),
    }
}

impl ReconstructedEvidence {
    pub fn new(manifest: Manifest) -> Result<Self> {
        manifest.validate()?;
        Ok(Self {
            manifest,
            test_runs: BTreeMap::new(),
            test_sequences: BTreeMap::new(),
            recorded_states: BTreeMap::new(),
            observations: BTreeMap::new(),
            action_descriptions: BTreeMap::new(),
            action_executions: BTreeMap::new(),
            execution_order: Vec::new(),
            recorded_transitions: Vec::new(),
            oracle_results: Vec::new(),
            events: BTreeMap::new(),
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
    pub fn observation_count(&self) -> usize {
        self.observations.len()
    }
    pub fn test_run_count(&self) -> usize {
        self.test_runs.len()
    }
    pub fn test_sequence_count(&self) -> usize {
        self.test_sequences.len()
    }
    pub fn recorded_state_count(&self) -> usize {
        self.recorded_states.len()
    }
    pub fn action_description_count(&self) -> usize {
        self.action_descriptions.len()
    }
    pub fn action_execution_count(&self) -> usize {
        self.action_executions.len()
    }
    pub fn recorded_transition_count(&self) -> usize {
        self.recorded_transitions.len()
    }
    pub fn test_run(&self, id: &TestRunId) -> Option<&TestRun> {
        self.test_runs.get(id).map(|(record, _)| record)
    }
    pub fn test_runs(&self) -> impl Iterator<Item = &TestRun> {
        self.test_runs.values().map(|(record, _)| record)
    }
    pub fn test_run_status(&self, id: &TestRunId) -> Option<&HistoryStatus> {
        self.test_runs.get(id).map(|(_, status)| status)
    }
    pub fn test_sequence(&self, id: &TestSequenceId) -> Option<&TestSequence> {
        self.test_sequences.get(id).map(|(record, _)| record)
    }
    pub fn test_sequences(&self) -> impl Iterator<Item = &TestSequence> {
        self.test_sequences.values().map(|(record, _)| record)
    }
    pub fn test_sequence_status(
        &self,
        id: &TestSequenceId,
    ) -> Option<&HistoryStatus> {
        self.test_sequences.get(id).map(|(_, status)| status)
    }
    pub fn observation(&self, id: &ObservationId) -> Option<&Observation> {
        self.observations.get(id)
    }
    pub fn observations(&self) -> impl Iterator<Item = &Observation> {
        self.observations.values()
    }
    pub fn recorded_state(
        &self,
        id: &RecordedStateId,
    ) -> Option<&RecordedState> {
        self.recorded_states.get(id)
    }
    pub fn recorded_states(&self) -> impl Iterator<Item = &RecordedState> {
        self.recorded_states.values()
    }
    pub fn action_description(
        &self,
        id: &ActionDescriptionId,
    ) -> Option<&ActionDescription> {
        self.action_descriptions.get(id)
    }
    pub fn action_descriptions(
        &self,
    ) -> impl Iterator<Item = &ActionDescription> {
        self.action_descriptions.values()
    }
    pub fn action_execution(
        &self,
        id: &ActionExecutionId,
    ) -> Option<&ActionExecutionRecord> {
        self.action_executions.get(id)
    }
    pub fn action_executions(
        &self,
    ) -> impl Iterator<Item = &ActionExecutionRecord> {
        self.execution_order
            .iter()
            .map(|id| &self.action_executions[id])
    }
    pub fn recorded_transitions(&self) -> &[RecordedTransition] {
        &self.recorded_transitions
    }
    pub fn oracle_results(&self) -> &[OracleResult] {
        &self.oracle_results
    }
    pub fn available_actions(
        &self,
        observation: &ObservationId,
    ) -> Option<&[AvailableAction]> {
        self.observation(observation)
            .map(|value| value.available_actions.as_slice())
    }

    pub fn test_sequences_for_run(
        &self,
        run: &TestRunId,
    ) -> Vec<&TestSequence> {
        let mut result: Vec<_> = self
            .test_sequences()
            .filter(|value| &value.test_run == run)
            .collect();
        result.sort_by_key(|value| value.sequence_index);
        result
    }

    pub fn observations_for_sequence(
        &self,
        sequence: &TestSequenceId,
    ) -> Vec<&Observation> {
        let mut result: Vec<_> = self
            .observations()
            .filter(|value| &value.test_sequence == sequence)
            .collect();
        result.sort_by_key(|value| value.observation_index);
        result
    }

    pub fn observations_for_recorded_state(
        &self,
        id: &RecordedStateId,
    ) -> Vec<&Observation> {
        self.observations()
            .filter(|value| &value.recorded_state.id == id)
            .collect()
    }

    pub fn executions_for_sequence(
        &self,
        sequence: &TestSequenceId,
    ) -> Vec<&ActionExecutionRecord> {
        self.action_executions()
            .filter(|value| &value.action_execution.test_sequence == sequence)
            .collect()
    }

    pub fn outgoing(
        &self,
        observation: &ObservationId,
    ) -> Vec<&RecordedTransition> {
        self.recorded_transitions
            .iter()
            .filter(|value| &value.source_observation == observation)
            .collect()
    }

    pub fn incoming(
        &self,
        observation: &ObservationId,
    ) -> Vec<&RecordedTransition> {
        self.recorded_transitions
            .iter()
            .filter(|value| &value.resulting_observation == observation)
            .collect()
    }

    pub fn validate(&self, envelope: &EvidenceEvent) -> Result<bool> {
        validate_version(envelope.schema_version)?;
        if let Some(previous) = self.events.get(&envelope.id) {
            if previous == envelope {
                return Ok(false);
            }
            return Err(Error::Conflict(format!("event {}", envelope.id)));
        }
        match &envelope.event {
            Event::TestRunStarted(run) => {
                run.validate()?;
                if self.test_runs.contains_key(&run.id) {
                    return Err(Error::Conflict(
                        "test run already exists".into(),
                    ));
                }
            }
            Event::TestSequenceStarted(sequence) => {
                sequence.validate()?;
                let status = self
                    .test_run_status(&sequence.test_run)
                    .ok_or_else(|| {
                        issue(
                            "invalid_reference",
                            "test_run",
                            "unknown parent run",
                        )
                    })?;
                if *status != HistoryStatus::Running {
                    return Err(issue(
                        "invalid_lifecycle",
                        "test_run",
                        "parent run finished",
                    ));
                }
                if self.test_sequences.contains_key(&sequence.id) {
                    return Err(Error::Conflict(
                        "test sequence already exists".into(),
                    ));
                }
                if self.test_sequences_for_run(&sequence.test_run).iter().any(
                    |value| value.sequence_index >= sequence.sequence_index,
                ) {
                    return Err(issue(
                        "invalid_order",
                        "sequence_index",
                        "sequence indexes must increase",
                    ));
                }
            }
            Event::ActionDescribed(action) => {
                action.validate()?;
                if self
                    .action_description(&action.id)
                    .is_some_and(|previous| previous != action)
                {
                    return Err(Error::Conflict(
                        "action description content changed".into(),
                    ));
                }
            }
            Event::ObservationRecorded(observation) => {
                self.validate_observation(observation)?
            }
            Event::ActionSelected(execution) => {
                self.validate_selection(execution)?
            }
            Event::ActionExecutionReported(result) => {
                result.validate()?;
                let record =
                    self.require_execution(&result.action_execution)?;
                self.require_active_sequence(
                    &record.action_execution.test_sequence,
                )?;
                if record.status
                    != ActionExecutionStatus::AwaitingExecutionResult
                {
                    return Err(issue(
                        "invalid_lifecycle",
                        "action_execution",
                        "execution result already reported",
                    ));
                }
            }
            Event::TransitionRecorded {
                action_execution,
                resulting_observation,
            } => {
                let record = self.require_execution(action_execution)?;
                self.require_active_sequence(
                    &record.action_execution.test_sequence,
                )?;
                if record.status
                    != ActionExecutionStatus::AwaitingResultingObservation
                {
                    return Err(issue(
                        "invalid_lifecycle",
                        "action_execution",
                        "execution is not awaiting an observation",
                    ));
                }
                let source = &self.observations
                    [&record.action_execution.source_observation];
                let result = self
                    .observation(resulting_observation)
                    .ok_or_else(|| {
                        issue(
                            "invalid_transition",
                            "resulting_observation",
                            "unknown observation",
                        )
                    })?;
                if result.test_sequence != source.test_sequence
                    || result.observation_index <= source.observation_index
                {
                    return Err(issue(
                        "invalid_transition",
                        "resulting_observation",
                        "result must be later in the same sequence",
                    ));
                }
            }
            Event::OracleResultRecorded(result) => {
                self.validate_oracle(result)?
            }
            Event::TestSequenceFinished { test_sequence, .. } => {
                self.require_active_sequence(test_sequence)?
            }
            Event::TestRunFinished { test_run, .. } => {
                if self.test_run_status(test_run)
                    != Some(&HistoryStatus::Running)
                {
                    return Err(issue(
                        "invalid_lifecycle",
                        "test_run",
                        "run is not active",
                    ));
                }
                if self.test_sequences_for_run(test_run).iter().any(
                    |sequence| {
                        self.test_sequence_status(&sequence.id)
                            == Some(&HistoryStatus::Running)
                    },
                ) {
                    return Err(issue(
                        "invalid_lifecycle",
                        "test_run",
                        "finish its sequences first",
                    ));
                }
            }
        }
        Ok(true)
    }

    pub fn apply(&mut self, envelope: EvidenceEvent) -> Result<bool> {
        if !self.validate(&envelope)? {
            return Ok(false);
        }
        self.project(&envelope.event);
        self.events.insert(envelope.id.clone(), envelope);
        Ok(true)
    }

    fn require_active_sequence(&self, sequence: &TestSequenceId) -> Result<()> {
        match self.test_sequence_status(sequence) {
            Some(HistoryStatus::Running) => Ok(()),
            Some(_) => Err(issue(
                "invalid_lifecycle",
                "test_sequence",
                "sequence finished",
            )),
            None => Err(issue(
                "invalid_reference",
                "test_sequence",
                "unknown sequence",
            )),
        }
    }

    fn require_execution(
        &self,
        id: &ActionExecutionId,
    ) -> Result<&ActionExecutionRecord> {
        self.action_execution(id).ok_or_else(|| {
            issue("invalid_reference", "action_execution", "unknown execution")
        })
    }

    fn validate_observation(&self, observation: &Observation) -> Result<()> {
        observation.validate()?;
        self.require_active_sequence(&observation.test_sequence)?;
        if self.observations.contains_key(&observation.id) {
            return Err(Error::Conflict("observation already exists".into()));
        }
        if self
            .observations_for_sequence(&observation.test_sequence)
            .last()
            .is_some_and(|last| {
                observation.observation_index <= last.observation_index
            })
        {
            return Err(issue(
                "invalid_order",
                "observation_index",
                "observation indexes must increase",
            ));
        }
        if self
            .recorded_state(&observation.recorded_state.id)
            .is_some_and(|previous| previous != &observation.recorded_state)
        {
            return Err(Error::Conflict(
                "recorded-state content changed".into(),
            ));
        }
        for available in &observation.available_actions {
            if self
                .action_description(&available.action_description)
                .is_none()
            {
                return Err(Error::UnknownReference(
                    "unknown action description".into(),
                ));
            }
        }
        Ok(())
    }

    fn validate_selection(&self, execution: &ActionExecution) -> Result<()> {
        execution.validate()?;
        self.require_active_sequence(&execution.test_sequence)?;
        if self.action_executions.contains_key(&execution.id) {
            return Err(Error::Conflict("execution already exists".into()));
        }
        let source = self
            .observation(&execution.source_observation)
            .ok_or_else(|| {
                issue(
                    "invalid_reference",
                    "source_observation",
                    "unknown observation",
                )
            })?;
        if source.test_sequence != execution.test_sequence
            || self
                .observations_for_sequence(&execution.test_sequence)
                .last()
                .map(|value| &value.id)
                != Some(&source.id)
        {
            return Err(issue(
                "invalid_reference",
                "source_observation",
                "source must be the sequence's latest observation",
            ));
        }
        if !source.available_actions.iter().any(|value| {
            value.action_description == execution.action_description
                && value.target_widget == execution.target_widget
        }) {
            return Err(Error::UnknownReference(
                "selected action/target is not available at source".into(),
            ));
        }
        if self
            .executions_for_sequence(&execution.test_sequence)
            .iter()
            .any(|record| {
                matches!(
                    record.status,
                    ActionExecutionStatus::AwaitingExecutionResult
                        | ActionExecutionStatus::AwaitingResultingObservation
                )
            })
        {
            return Err(issue(
                "invalid_lifecycle",
                "test_sequence",
                "sequence already has an unfinished execution",
            ));
        }
        Ok(())
    }

    fn oracle_observation(
        &self,
        id: &ObservationId,
        sequence: &TestSequenceId,
        field: &'static str,
    ) -> Result<&Observation> {
        self.observation(id)
            .filter(|value| &value.test_sequence == sequence)
            .ok_or_else(|| {
                issue(
                    "invalid_oracle_target",
                    field,
                    "observation must belong to result's test sequence",
                )
            })
    }

    fn validate_oracle(&self, result: &OracleResult) -> Result<()> {
        result.validate()?;
        if self.test_run(&result.test_run).is_none() {
            return Err(issue(
                "invalid_reference",
                "test_run",
                "unknown test run",
            ));
        }
        if matches!(result.oracle_target, OracleTarget::TestRun {}) {
            if result.test_sequence.is_some() {
                return Err(issue(
                    "invalid_oracle_target",
                    "test_sequence",
                    "run-wide result has no sequence",
                ));
            }
        } else {
            let sequence = result
                .test_sequence
                .as_ref()
                .and_then(|id| self.test_sequence(id))
                .filter(|sequence| sequence.test_run == result.test_run)
                .ok_or_else(|| {
                    issue(
                        "invalid_oracle_target",
                        "test_sequence",
                        "unknown sequence or wrong parent",
                    )
                })?;
            match &result.oracle_target {
                OracleTarget::TestRun {} | OracleTarget::TestSequence {} => {}
                OracleTarget::Observation { observation }
                | OracleTarget::StateProperty { observation, .. } => {
                    self.oracle_observation(
                        observation,
                        &sequence.id,
                        "oracle_target.observation",
                    )?;
                }
                OracleTarget::Widget {
                    observation,
                    widget,
                }
                | OracleTarget::WidgetProperty {
                    observation,
                    widget,
                    ..
                } => {
                    let captured = self.oracle_observation(
                        observation,
                        &sequence.id,
                        "oracle_target.observation",
                    )?;
                    if !captured
                        .recorded_state
                        .widgets
                        .iter()
                        .any(|value| value.id == *widget)
                    {
                        return Err(issue(
                            "invalid_oracle_target",
                            "oracle_target.widget",
                            "widget is not captured at target observation",
                        ));
                    }
                }
                OracleTarget::ActionExecution { action_execution } => {
                    if !self.action_execution(action_execution).is_some_and(
                        |value| {
                            value.action_execution.test_sequence == sequence.id
                        },
                    ) {
                        return Err(issue(
                            "invalid_oracle_target",
                            "oracle_target.action_execution",
                            "execution belongs to another sequence",
                        ));
                    }
                }
                OracleTarget::ObservationRange {
                    first_observation,
                    last_observation,
                } => {
                    let first = self.oracle_observation(
                        first_observation,
                        &sequence.id,
                        "oracle_target.first_observation",
                    )?;
                    let last = self.oracle_observation(
                        last_observation,
                        &sequence.id,
                        "oracle_target.last_observation",
                    )?;
                    if last.observation_index < first.observation_index {
                        return Err(issue(
                            "invalid_oracle_target",
                            "oracle_target.last_observation",
                            "range ends before it starts",
                        ));
                    }
                }
            }
        }
        self.validate_assessment(result)
    }

    fn validate_assessment(&self, result: &OracleResult) -> Result<()> {
        let Some(context) = result.oracle_details.get("evidence.assessment")
        else {
            return Ok(());
        };
        let phase = context
            .get("phase")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let semantics = context
            .get("semantics")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if !matches!(semantics, "open_ended" | "recorded_interval") {
            return Err(issue(
                "invalid_record",
                "oracle_details.evidence.assessment.semantics",
                "unknown temporal semantics",
            ));
        }
        let status = match phase {
            "during_run" | "end_of_run" => {
                self.test_run_status(&result.test_run)
            }
            "during_sequence" | "end_of_sequence" => result
                .test_sequence
                .as_ref()
                .and_then(|id| self.test_sequence_status(id)),
            _ => {
                return Err(issue(
                    "invalid_record",
                    "oracle_details.evidence.assessment.phase",
                    "unknown phase",
                ));
            }
        }
        .ok_or_else(|| {
            issue(
                "invalid_oracle_target",
                "test_sequence",
                "sequence assessment needs a sequence",
            )
        })?;
        let ending = phase.starts_with("end_");
        if ending == matches!(status, HistoryStatus::Running) {
            return Err(issue(
                "invalid_lifecycle",
                "oracle_details.evidence.assessment.phase",
                "phase does not match history status",
            ));
        }
        if let Some(reason) = context.get("termination_reason") {
            if !matches!(status, HistoryStatus::Finished { reason: actual } if reason.as_str() == Some(actual.as_str()))
            {
                return Err(issue(
                    "invalid_record",
                    "oracle_details.evidence.assessment.termination_reason",
                    "reason must match history ending",
                ));
            }
        }
        Ok(())
    }

    fn project(&mut self, event: &Event) {
        match event {
            Event::TestRunStarted(run) => {
                self.test_runs.insert(
                    run.id.clone(),
                    (run.clone(), HistoryStatus::Running),
                );
            }
            Event::TestSequenceStarted(sequence) => {
                self.test_sequences.insert(
                    sequence.id.clone(),
                    (sequence.clone(), HistoryStatus::Running),
                );
            }
            Event::ActionDescribed(action) => {
                self.action_descriptions
                    .insert(action.id.clone(), action.clone());
            }
            Event::ObservationRecorded(observation) => {
                self.recorded_states.insert(
                    observation.recorded_state.id.clone(),
                    observation.recorded_state.clone(),
                );
                self.observations
                    .insert(observation.id.clone(), observation.clone());
            }
            Event::ActionSelected(execution) => {
                self.execution_order.push(execution.id.clone());
                self.action_executions.insert(
                    execution.id.clone(),
                    ActionExecutionRecord {
                        action_execution: execution.clone(),
                        status: ActionExecutionStatus::AwaitingExecutionResult,
                        execution_result: None,
                        resulting_observation: None,
                    },
                );
            }
            Event::ActionExecutionReported(result) => {
                let record = self
                    .action_executions
                    .get_mut(&result.action_execution)
                    .unwrap();
                record.status = if result.outcome == ActionOutcome::Returned {
                    ActionExecutionStatus::AwaitingResultingObservation
                } else {
                    ActionExecutionStatus::Failed
                };
                record.execution_result = Some(result.clone());
            }
            Event::TransitionRecorded {
                action_execution,
                resulting_observation,
            } => {
                let record =
                    self.action_executions.get_mut(action_execution).unwrap();
                record.status = ActionExecutionStatus::Completed;
                record.resulting_observation =
                    Some(resulting_observation.clone());
                let execution = &record.action_execution;
                self.recorded_transitions.push(RecordedTransition {
                    action_execution: execution.id.clone(),
                    test_sequence: execution.test_sequence.clone(),
                    source_observation: execution.source_observation.clone(),
                    resulting_observation: resulting_observation.clone(),
                    action_description: execution.action_description.clone(),
                    target_widget: execution.target_widget.clone(),
                });
            }
            Event::OracleResultRecorded(result) => {
                self.oracle_results.push(result.clone())
            }
            Event::TestSequenceFinished {
                test_sequence,
                reason,
            } => {
                self.test_sequences.get_mut(test_sequence).unwrap().1 =
                    HistoryStatus::Finished {
                        reason: reason.clone(),
                    };
                for record in self.action_executions.values_mut() {
                    if &record.action_execution.test_sequence == test_sequence && matches!(record.status,
                        ActionExecutionStatus::AwaitingExecutionResult | ActionExecutionStatus::AwaitingResultingObservation) {
                        record.status = ActionExecutionStatus::Incomplete;
                    }
                }
            }
            Event::TestRunFinished { test_run, reason } => {
                self.test_runs.get_mut(test_run).unwrap().1 =
                    HistoryStatus::Finished {
                        reason: reason.clone(),
                    };
            }
        }
    }
}
