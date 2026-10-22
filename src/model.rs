use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::event::validate_version;
use crate::{
    ActionCandidate, ActionDefinition, ActionId, CheckResult, CheckTarget,
    ConcreteState, ConcreteStateId, Error, Event, EventId, EvidenceEvent,
    Execution, ExecutionId, Manifest, Observation, ObservationId, Result, Run,
    RunId,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Selected,
    Submitted,
    Rejected,
    Observed,
    Incomplete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionRecord {
    pub execution: Execution,
    pub status: ExecutionStatus,
    pub result: Option<ObservationId>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunStatus {
    Running,
    Finished { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionLink {
    pub execution: ExecutionId,
    pub run: RunId,
    pub source: ObservationId,
    pub action: ActionId,
    pub target: ObservationId,
}

#[derive(Clone, Debug)]
pub struct StateModel {
    manifest: Manifest,
    runs: BTreeMap<RunId, (Run, RunStatus)>,
    sequences: BTreeMap<RunId, u64>,
    concrete_states: BTreeMap<ConcreteStateId, ConcreteState>,
    observations: BTreeMap<ObservationId, Observation>,
    actions: BTreeMap<ActionId, ActionDefinition>,
    executions: BTreeMap<ExecutionId, ExecutionRecord>,
    execution_order: Vec<ExecutionId>,
    checks: Vec<CheckResult>,
    events: BTreeMap<EventId, EvidenceEvent>,
}

impl StateModel {
    pub fn new(manifest: Manifest) -> Result<Self> {
        manifest.validate()?;
        Ok(Self {
            manifest,
            runs: BTreeMap::new(),
            sequences: BTreeMap::new(),
            concrete_states: BTreeMap::new(),
            observations: BTreeMap::new(),
            actions: BTreeMap::new(),
            executions: BTreeMap::new(),
            execution_order: Vec::new(),
            checks: Vec::new(),
            events: BTreeMap::new(),
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
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
            Event::RunStarted(run) => {
                run.validate()?;
                if self.runs.contains_key(&run.id) {
                    return Err(Error::Conflict(format!("run {}", run.id)));
                }
            }
            Event::ActionDefined(action) => {
                action.validate()?;
                if self
                    .actions
                    .get(&action.id)
                    .is_some_and(|old| old != action)
                {
                    return Err(Error::Conflict(format!(
                        "action {}",
                        action.id
                    )));
                }
            }
            Event::Observed(observation) => {
                self.validate_observation(observation)?;
            }
            Event::ActionSelected(execution) => {
                self.validate_selection(execution)?;
            }
            Event::ActionSubmitted {
                execution,
                accepted,
                error,
            } => {
                let record = self.require_execution(execution)?;
                self.require_running(&record.execution.run)?;
                if record.status != ExecutionStatus::Selected {
                    return Err(Error::Conflict(format!(
                        "execution {execution} is no longer awaiting submission"
                    )));
                }
                if *accepted && error.is_some() {
                    return Err(Error::InvalidRecord(
                        "accepted submission cannot carry an error".into(),
                    ));
                }
            }
            Event::ActionResult {
                execution,
                observation,
            } => {
                self.validate_result(execution, observation)?;
            }
            Event::Checked(check) => {
                self.validate_check(check)?;
            }
            Event::RunFinished { run, reason } => {
                self.require_running(run)?;
                if reason.trim().is_empty() {
                    return Err(Error::InvalidRecord(
                        "empty run ending reason".into(),
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

    pub fn concrete_state_count(&self) -> usize {
        self.concrete_states.len()
    }

    pub fn observation_count(&self) -> usize {
        self.observations.len()
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    pub fn execution_count(&self) -> usize {
        self.executions.len()
    }

    pub fn link_count(&self) -> usize {
        self.executions
            .values()
            .filter(|record| record.result.is_some())
            .count()
    }

    pub fn observation(&self, id: &ObservationId) -> Option<&Observation> {
        self.observations.get(id)
    }

    pub fn observations(&self) -> impl Iterator<Item = &Observation> {
        self.observations.values()
    }

    pub fn concrete_state(
        &self,
        id: &ConcreteStateId,
    ) -> Option<&ConcreteState> {
        self.concrete_states.get(id)
    }

    pub fn concrete_states(&self) -> impl Iterator<Item = &ConcreteState> {
        self.concrete_states.values()
    }

    pub fn observations_for_snapshot(
        &self,
        id: &ConcreteStateId,
    ) -> Vec<&Observation> {
        self.observations
            .values()
            .filter(|observation| &observation.state.id == id)
            .collect()
    }

    pub fn observations_for_run(&self, run: &RunId) -> Vec<&Observation> {
        let mut observations: Vec<_> = self
            .observations
            .values()
            .filter(|observation| &observation.run == run)
            .collect();
        observations.sort_by_key(|observation| observation.sequence);
        observations
    }

    pub fn execution(&self, id: &ExecutionId) -> Option<&ExecutionRecord> {
        self.executions.get(id)
    }

    pub fn executions(&self) -> impl Iterator<Item = &ExecutionRecord> {
        self.execution_order.iter().map(|id| &self.executions[id])
    }

    pub fn executions_for_run(&self, run: &RunId) -> Vec<&ExecutionRecord> {
        self.executions()
            .filter(|record| &record.execution.run == run)
            .collect()
    }

    pub fn run_status(&self, id: &RunId) -> Option<&RunStatus> {
        self.runs.get(id).map(|(_, status)| status)
    }

    pub fn run(&self, id: &RunId) -> Option<&Run> {
        self.runs.get(id).map(|(run, _)| run)
    }

    pub fn runs(&self) -> impl Iterator<Item = &Run> {
        self.runs.values().map(|(run, _)| run)
    }

    pub fn action(&self, id: &ActionId) -> Option<&ActionDefinition> {
        self.actions.get(id)
    }

    pub fn actions(&self) -> impl Iterator<Item = &ActionDefinition> {
        self.actions.values()
    }

    pub fn checks(&self) -> &[CheckResult] {
        &self.checks
    }

    pub fn candidates(
        &self,
        observation: &ObservationId,
    ) -> Option<&[ActionCandidate]> {
        self.observations
            .get(observation)
            .map(|value| value.candidates.as_slice())
    }

    pub fn links(&self) -> impl Iterator<Item = ExecutionLink> + '_ {
        self.executions().filter_map(|record| {
            let target = record.result.as_ref()?;
            Some(ExecutionLink {
                execution: record.execution.id.clone(),
                run: record.execution.run.clone(),
                source: record.execution.source.clone(),
                action: record.execution.action.clone(),
                target: target.clone(),
            })
        })
    }

    pub fn outgoing(&self, observation: &ObservationId) -> Vec<ExecutionLink> {
        self.links()
            .filter(|link| &link.source == observation)
            .collect()
    }

    pub fn incoming(&self, observation: &ObservationId) -> Vec<ExecutionLink> {
        self.links()
            .filter(|link| &link.target == observation)
            .collect()
    }

    fn require_running(&self, id: &RunId) -> Result<()> {
        match self.runs.get(id) {
            Some((_, RunStatus::Running)) => Ok(()),
            Some(_) => Err(Error::Conflict(format!("run {id} has finished"))),
            None => Err(Error::UnknownReference(format!("run {id}"))),
        }
    }

    fn require_observation(&self, id: &ObservationId) -> Result<&Observation> {
        self.observations
            .get(id)
            .ok_or_else(|| Error::UnknownReference(format!("observation {id}")))
    }

    fn require_execution(&self, id: &ExecutionId) -> Result<&ExecutionRecord> {
        self.executions
            .get(id)
            .ok_or_else(|| Error::UnknownReference(format!("execution {id}")))
    }

    fn validate_observation(&self, observation: &Observation) -> Result<()> {
        observation.validate()?;
        self.require_running(&observation.run)?;
        if self.observations.contains_key(&observation.id) {
            return Err(Error::Conflict(format!(
                "observation {}",
                observation.id
            )));
        }
        if self
            .sequences
            .get(&observation.run)
            .is_some_and(|last| observation.sequence <= *last)
        {
            return Err(Error::InvalidRecord(
                "observation sequence must increase within a run".into(),
            ));
        }
        if self
            .concrete_states
            .get(&observation.state.id)
            .is_some_and(|state| state != &observation.state)
        {
            return Err(Error::Conflict(format!(
                "concrete state {}",
                observation.state.id
            )));
        }
        for candidate in &observation.candidates {
            if !self.actions.contains_key(&candidate.action) {
                return Err(Error::UnknownReference(format!(
                    "action {}",
                    candidate.action
                )));
            }
        }
        Ok(())
    }

    fn validate_selection(&self, execution: &Execution) -> Result<()> {
        execution.validate()?;
        self.require_running(&execution.run)?;
        if self.executions.contains_key(&execution.id) {
            return Err(Error::Conflict(format!("execution {}", execution.id)));
        }
        let source = self.require_observation(&execution.source)?;
        if source.run != execution.run {
            return Err(Error::InvalidRecord(
                "execution source belongs to another run".into(),
            ));
        }
        if self.sequences[&execution.run] != source.sequence {
            return Err(Error::InvalidRecord(
                "execution source is not the latest observation".into(),
            ));
        }
        if !source.candidates.iter().any(|candidate| {
            candidate.action == execution.action
                && candidate.target == execution.target
        }) {
            return Err(Error::UnknownReference(
                "selected action/target is not a source candidate".into(),
            ));
        }
        if self.executions.values().any(|record| {
            record.execution.run == execution.run
                && matches!(
                    record.status,
                    ExecutionStatus::Selected | ExecutionStatus::Submitted
                )
        }) {
            return Err(Error::Conflict(
                "run already has a pending action".into(),
            ));
        }
        Ok(())
    }

    fn validate_result(
        &self,
        execution: &ExecutionId,
        observation: &ObservationId,
    ) -> Result<()> {
        let record = self.require_execution(execution)?;
        self.require_running(&record.execution.run)?;
        if record.status != ExecutionStatus::Submitted {
            return Err(Error::Conflict(format!(
                "execution {execution} is not awaiting a result"
            )));
        }
        let source = self.require_observation(&record.execution.source)?;
        let target = self.require_observation(observation)?;
        if source.run != target.run || target.sequence <= source.sequence {
            return Err(Error::InvalidRecord(
                "action result must be a later observation in the same run"
                    .into(),
            ));
        }
        Ok(())
    }

    fn validate_check(&self, check: &CheckResult) -> Result<()> {
        check.validate()?;
        if !self.runs.contains_key(&check.run) {
            return Err(Error::UnknownReference(format!("run {}", check.run)));
        }
        match &check.target {
            CheckTarget::Run {} => {}
            CheckTarget::Observation { observation }
            | CheckTarget::StateProperty { observation, .. } => {
                self.check_observation(
                    observation,
                    &check.run,
                    "target.observation",
                )?;
            }
            CheckTarget::Widget {
                observation,
                widget,
            }
            | CheckTarget::WidgetProperty {
                observation,
                widget,
                ..
            } => {
                let captured = self.check_observation(
                    observation,
                    &check.run,
                    "target.observation",
                )?;
                if !captured.state.widgets.iter().any(|item| item.id == *widget)
                {
                    return Err(Error::InvalidCheckTarget {
                        field: "target.widget",
                        message: format!(
                            "widget {widget} is not in observation {observation}"
                        ),
                    });
                }
            }
            CheckTarget::Execution { execution } => {
                let valid = self
                    .executions
                    .get(execution)
                    .is_some_and(|record| record.execution.run == check.run);
                if !valid {
                    return Err(Error::InvalidCheckTarget {
                        field: "target.execution",
                        message: format!(
                            "execution {execution} must belong to run {}",
                            check.run
                        ),
                    });
                }
            }
            CheckTarget::Sequence {
                from_observation,
                to_observation,
            } => {
                let start = self.check_observation(
                    from_observation,
                    &check.run,
                    "target.from_observation",
                )?;
                let end = self.check_observation(
                    to_observation,
                    &check.run,
                    "target.to_observation",
                )?;
                if end.sequence < start.sequence {
                    return Err(Error::InvalidCheckTarget {
                        field: "target.to_observation",
                        message: "sequence end precedes its start".into(),
                    });
                }
            }
        }
        Ok(())
    }

    fn check_observation(
        &self,
        id: &ObservationId,
        run: &RunId,
        field: &'static str,
    ) -> Result<&Observation> {
        self.observations
            .get(id)
            .filter(|observation| observation.run == *run)
            .ok_or_else(|| Error::InvalidCheckTarget {
                field,
                message: format!("observation {id} must belong to run {run}"),
            })
    }

    fn project(&mut self, event: &Event) {
        match event {
            Event::RunStarted(run) => {
                self.runs
                    .insert(run.id.clone(), (run.clone(), RunStatus::Running));
            }
            Event::ActionDefined(action) => {
                self.actions.insert(action.id.clone(), action.clone());
            }
            Event::Observed(observation) => {
                self.concrete_states.insert(
                    observation.state.id.clone(),
                    observation.state.clone(),
                );
                self.sequences
                    .insert(observation.run.clone(), observation.sequence);
                self.observations
                    .insert(observation.id.clone(), observation.clone());
            }
            Event::ActionSelected(execution) => {
                self.execution_order.push(execution.id.clone());
                self.executions.insert(
                    execution.id.clone(),
                    ExecutionRecord {
                        execution: execution.clone(),
                        status: ExecutionStatus::Selected,
                        result: None,
                        error: None,
                    },
                );
            }
            Event::ActionSubmitted {
                execution,
                accepted,
                error,
            } => {
                let record = self.executions.get_mut(execution).unwrap();
                record.status = if *accepted {
                    ExecutionStatus::Submitted
                } else {
                    ExecutionStatus::Rejected
                };
                record.error = error.clone();
            }
            Event::ActionResult {
                execution,
                observation,
            } => {
                self.project_result(execution, observation);
            }
            Event::Checked(check) => {
                self.checks.push(check.clone());
            }
            Event::RunFinished { run, reason } => {
                self.runs.get_mut(run).unwrap().1 = RunStatus::Finished {
                    reason: reason.clone(),
                };
                for record in self.executions.values_mut() {
                    if &record.execution.run == run
                        && matches!(
                            record.status,
                            ExecutionStatus::Selected
                                | ExecutionStatus::Submitted
                        )
                    {
                        record.status = ExecutionStatus::Incomplete;
                    }
                }
            }
        }
    }

    fn project_result(
        &mut self,
        execution: &ExecutionId,
        observation: &ObservationId,
    ) {
        let record = self.executions.get_mut(execution).unwrap();
        record.status = ExecutionStatus::Observed;
        record.result = Some(observation.clone());
    }
}
