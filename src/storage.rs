use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use sha2::{Digest, Sha256};

use crate::{
    ArtifactReference, Error, Event, EvidenceEvent, ReconstructedEvidence, Manifest,
    Result,
};

pub struct JsonlStore {
    directory: PathBuf,
    _lock: File,
    log: File,
    evidence: ReconstructedEvidence,
    writer_valid: bool,
}

pub struct EvidenceReader;

impl EvidenceReader {
    pub fn read(path: impl AsRef<Path>) -> Result<ReconstructedEvidence> {
        let directory = fs::canonicalize(path.as_ref())?;
        let manifest = read_manifest(&directory)?;
        manifest.validate()?;
        let mut log = File::open(directory.join("evidence.jsonl"))?;
        replay(&directory, &mut log, manifest).map(|(evidence, _)| evidence)
    }
}

impl JsonlStore {
    pub fn create(path: impl AsRef<Path>, manifest: Manifest) -> Result<Self> {
        let evidence = ReconstructedEvidence::new(manifest)?;
        fs::create_dir_all(path.as_ref())?;
        let directory = fs::canonicalize(path.as_ref())?;
        let lock = acquire_lock(&directory)?;
        let manifest_path = directory.join("manifest.json");
        let log_path = directory.join("evidence.jsonl");
        if manifest_path.try_exists()? || log_path.try_exists()? {
            return Err(Error::Conflict(
                "evidence files already exist; open the evidence instead of creating it".into(),
            ));
        }

        let mut manifest_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(manifest_path)?;
        serde_json::to_writer_pretty(&mut manifest_file, evidence.manifest())?;
        manifest_file.write_all(b"\n")?;
        manifest_file.flush()?;
        manifest_file.sync_all()?;
        let log = open_new_log(&log_path)?;
        log.sync_all()?;

        Ok(Self {
            directory,
            _lock: lock,
            log,
            evidence,
            writer_valid: true,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let directory = fs::canonicalize(path.as_ref())?;
        let lock = acquire_lock(&directory)?;
        let manifest = read_manifest(&directory)?;
        let mut log = OpenOptions::new()
            .read(true)
            .append(true)
            .open(directory.join("evidence.jsonl"))?;
        let (evidence, incomplete_bytes) =
            replay(&directory, &mut log, manifest)?;
        if incomplete_bytes != 0 {
            return Err(Error::InvalidRecord(format!(
                "evidence.jsonl has an incomplete final fragment ({incomplete_bytes} bytes); \
                 call JsonlStore::recover_tail explicitly before reopening"
            )));
        }

        Ok(Self {
            directory,
            _lock: lock,
            log,
            evidence,
            writer_valid: true,
        })
    }

    pub fn recover_tail(path: impl AsRef<Path>) -> Result<u64> {
        let directory = fs::canonicalize(path.as_ref())?;
        let _lock = acquire_lock(&directory)?;
        let manifest = read_manifest(&directory)?;
        let mut log = OpenOptions::new()
            .read(true)
            .write(true)
            .open(directory.join("evidence.jsonl"))?;
        let (_, incomplete_bytes) = replay(&directory, &mut log, manifest)?;
        if incomplete_bytes != 0 {
            let length = log.metadata()?.len();
            log.set_len(length - incomplete_bytes)?;
            log.sync_all()?;
        }
        Ok(incomplete_bytes)
    }

    pub fn evidence(&self) -> &ReconstructedEvidence {
        &self.evidence
    }

    pub fn record(&mut self, envelope: EvidenceEvent) -> Result<bool> {
        self.ensure_writer_valid()?;
        if !self.evidence.validate(&envelope)? {
            return Ok(false);
        }
        verify_event_artifacts(&self.directory, &envelope)?;
        let mut record = serde_json::to_vec(&envelope)?;
        record.push(b'\n');
        if let Err(error) = self.persist_record(&record) {
            self.writer_valid = false;
            return Err(error.into());
        }

        // Replay is the authority if an acknowledged append cannot be projected.
        match self.evidence.apply(envelope) {
            Ok(applied) => Ok(applied),
            Err(error) => {
                self.writer_valid = false;
                Err(error)
            }
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        self.ensure_writer_valid()?;
        if let Err(error) = self.persist_record(&[]) {
            self.writer_valid = false;
            return Err(error.into());
        }
        Ok(())
    }

    pub fn put_artifact(
        &mut self,
        bytes: &[u8],
        media_type: impl Into<String>,
        width: Option<u64>,
        height: Option<u64>,
    ) -> Result<ArtifactReference> {
        self.ensure_writer_valid()?;
        let digest = sha256(bytes);
        let media_type = media_type.into();
        let extension = match media_type.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpeg",
            _ => "bin",
        };
        let reference = ArtifactReference {
            id: digest.clone(),
            path: format!("artifacts/{digest}.{extension}"),
            media_type,
            sha256: digest,
            width,
            height,
        };
        reference.validate()?;

        let result = self.write_artifact(&reference, bytes);
        if matches!(&result, Err(Error::Io(_))) {
            self.writer_valid = false;
        }
        result?;
        Ok(reference)
    }

    pub fn read_artifact(
        &self,
        reference: &ArtifactReference,
    ) -> Result<Vec<u8>> {
        read_artifact(&self.directory, reference)
    }

    fn ensure_writer_valid(&self) -> Result<()> {
        if !self.writer_valid {
            return Err(Error::InvalidRecord(
                "store writer is invalid after a persistence failure; reopen through recovery".into(),
            ));
        }
        Ok(())
    }

    fn persist_record(&mut self, record: &[u8]) -> std::io::Result<()> {
        self.log.write_all(record)?;
        self.log.flush()?;
        self.log.sync_data()
    }

    fn write_artifact(
        &self,
        reference: &ArtifactReference,
        bytes: &[u8],
    ) -> Result<()> {
        let artifact_directory = self.directory.join("artifacts");
        fs::create_dir_all(&artifact_directory)?;
        let artifact_directory = fs::canonicalize(artifact_directory)?;
        ensure_inside_directory(&self.directory, &artifact_directory)?;
        let artifact_path = self.directory.join(&reference.path);

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&artifact_path)
        {
            Ok(mut file) => {
                file.write_all(bytes)?;
                file.flush()?;
                file.sync_all()?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = read_artifact(&self.directory, reference)?;
                if existing != bytes {
                    return Err(Error::Conflict(format!(
                        "artifact bytes conflict at {}",
                        reference.path
                    )));
                }
                OpenOptions::new()
                    .write(true)
                    .open(artifact_path)?
                    .sync_all()?;
            }
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }
}

fn acquire_lock(directory: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(".writer.lock"))?;
    file.try_lock_exclusive()?;
    Ok(file)
}

fn open_new_log(path: &Path) -> Result<File> {
    Ok(OpenOptions::new()
        .read(true)
        .append(true)
        .create_new(true)
        .open(path)?)
}

fn read_manifest(directory: &Path) -> Result<Manifest> {
    Ok(serde_json::from_reader(File::open(
        directory.join("manifest.json"),
    )?)?)
}

fn replay(
    directory: &Path,
    log: &mut File,
    manifest: Manifest,
) -> Result<(ReconstructedEvidence, u64)> {
    let mut evidence = ReconstructedEvidence::new(manifest)?;
    let mut reader = BufReader::new(log);
    let mut record = Vec::new();
    let mut line = 0u64;
    loop {
        record.clear();
        let length = reader.read_until(b'\n', &mut record)?;
        if length == 0 {
            return Ok((evidence, 0));
        }
        line += 1;
        if record.last() != Some(&b'\n') {
            return Ok((evidence, length as u64));
        }
        let envelope: EvidenceEvent =
            serde_json::from_slice(&record).map_err(|error| {
                Error::InvalidRecord(format!(
                    "evidence.jsonl line {line}: {error}"
                ))
            })?;
        verify_event_artifacts(directory, &envelope)
            .map_err(|error| with_line_number(line, error))?;
        evidence
            .apply(envelope)
            .map_err(|error| with_line_number(line, error))?;
    }
}

fn with_line_number(line: u64, error: Error) -> Error {
    let location = format!("evidence.jsonl line {line}");
    match error {
        Error::Conflict(message) => {
            Error::Conflict(format!("{location}: {message}"))
        }
        Error::UnknownReference(message) => {
            Error::UnknownReference(format!("{location}: {message}"))
        }
        Error::Validation {
            code,
            field,
            message,
        } => Error::Validation {
            code,
            field,
            message: format!("{location}: {message}"),
        },
        error => Error::InvalidRecord(format!("{location}: {error}")),
    }
}

fn verify_event_artifacts(
    directory: &Path,
    envelope: &EvidenceEvent,
) -> Result<()> {
    if let Event::ObservationRecorded(observation) = &envelope.event {
        for reference in &observation.artifacts {
            read_artifact(directory, reference)?;
        }
    }
    Ok(())
}

fn read_artifact(
    directory: &Path,
    reference: &ArtifactReference,
) -> Result<Vec<u8>> {
    reference.validate()?;
    let artifact_path = fs::canonicalize(directory.join(&reference.path))?;
    ensure_inside_directory(directory, &artifact_path)?;
    let bytes = fs::read(artifact_path)?;
    if !sha256(&bytes).eq_ignore_ascii_case(&reference.sha256) {
        return Err(Error::Conflict(format!(
            "artifact digest mismatch at {}",
            reference.path
        )));
    }
    Ok(bytes)
}

fn ensure_inside_directory(directory: &Path, path: &Path) -> Result<()> {
    if !path.starts_with(directory) {
        return Err(Error::InvalidRecord(
            "artifact path resolves outside the evidence directory".into(),
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::{
        EventId, EvidenceId, PropertyMap, SCHEMA_VERSION, TestRun, TestRunId,
    };

    #[test]
    fn failed_append_invalidates_the_writer_without_applying_the_event() {
        let directory = tempdir().unwrap();
        let manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            evidence_id: EvidenceId::new("evidence").unwrap(),
        };
        let mut store = JsonlStore::create(directory.path(), manifest).unwrap();
        store.log =
            File::open(directory.path().join("evidence.jsonl")).unwrap();
        let envelope = EvidenceEvent {
            schema_version: SCHEMA_VERSION,
            id: EventId::new("event").unwrap(),
            event: Event::TestRunStarted(TestRun {
                id: TestRunId::new("run").unwrap(),
                producer: "test".into(),
                source_run: "0".into(),
                properties: PropertyMap::new(),
            }),
        };

        assert!(matches!(store.record(envelope.clone()), Err(Error::Io(_))));
        assert_eq!(store.evidence().event_count(), 0);
        let error = store.record(envelope).unwrap_err();
        assert!(error.to_string().contains("store writer is invalid"));
        assert!(store.flush().is_err());
        drop(store);
        assert_eq!(
            JsonlStore::open(directory.path())
                .unwrap()
                .evidence()
                .event_count(),
            0
        );
    }
}
