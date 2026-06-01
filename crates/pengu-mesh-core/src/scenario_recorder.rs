use anyhow::{Context, Result, anyhow, ensure};
use pengu_mesh_cdp::discover_installations;
use pengu_mesh_shared::{
    EnvironmentFingerprint, IdKind, LatencySample, ScenarioAssertion, ScenarioRun, ScenarioStep,
    StableId, utc_timestamp,
};
use pengu_mesh_state::StateStore;
use std::env;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct ScenarioRecorder {
    store: StateStore,
    run: ScenarioRun,
}

#[derive(Debug, Clone)]
pub struct StepRecorder {
    store: StateStore,
    step: ScenarioStep,
}

impl ScenarioRecorder {
    pub fn new(
        store: &StateStore,
        name: &str,
        family: &str,
        version: &str,
        surface: &str,
    ) -> Result<Self> {
        ensure_present("scenario name", name)?;
        ensure_present("scenario family", family)?;
        ensure_present("scenario version", version)?;
        ensure_present("tool surface", surface)?;

        let run_id = next_record_id(
            IdKind::ScenarioRun,
            &format!("{family}_{name}_{version}_{}", std::process::id()),
        );
        let fingerprint = capture_environment_fingerprint(&run_id);
        let run = ScenarioRun {
            id: run_id,
            scenario_name: name.trim().to_string(),
            scenario_family: family.trim().to_string(),
            scenario_version: version.trim().to_string(),
            tool_surface: surface.trim().to_string(),
            runtime_root: Some(store.paths().root_dir.clone()),
            commit_sha: git_output(&["rev-parse", "HEAD"]),
            branch_name: git_output(&["rev-parse", "--abbrev-ref", "HEAD"]),
            platform: fingerprint.platform.clone(),
            started_at: utc_timestamp(),
            finished_at: None,
            status: "running".to_string(),
            summary_path: None,
        };

        store
            .insert_scenario_run(&run)
            .context("insert initial scenario run")?;
        store
            .insert_environment_fingerprint(&fingerprint)
            .context("insert scenario environment fingerprint")?;

        Ok(Self {
            store: store.clone(),
            run,
        })
    }

    pub fn run(&self) -> &ScenarioRun {
        &self.run
    }

    pub fn into_run(self) -> ScenarioRun {
        self.run
    }

    pub fn step(&mut self, name: &str, kind: &str) -> Result<StepRecorder> {
        let step = create_scenario_step(&self.store, &self.run.id, name, kind, None)?;
        Ok(StepRecorder {
            store: self.store.clone(),
            step,
        })
    }

    pub fn finish(&mut self, status: &str) -> Result<()> {
        self.run = finish_run_record(&self.store, &self.run.id, status, None)?;
        Ok(())
    }
}

impl StepRecorder {
    pub fn step(&self) -> &ScenarioStep {
        &self.step
    }

    pub fn into_step(self) -> ScenarioStep {
        self.step
    }

    pub fn assert_eq(&mut self, name: &str, expected: &str, actual: &str) -> Result<bool> {
        ensure_present("assertion name", name)?;
        let passed = expected == actual;
        create_assertion_record(
            &self.store,
            &self.step.run_id,
            Some(&self.step.id),
            name,
            Some(expected),
            Some(actual),
            if passed { "passed" } else { "failed" },
            if passed { None } else { Some("mismatch") },
            None,
        )?;
        Ok(passed)
    }

    pub fn assert_true(&mut self, name: &str, value: bool) -> Result<bool> {
        ensure_present("assertion name", name)?;
        create_assertion_record(
            &self.store,
            &self.step.run_id,
            Some(&self.step.id),
            name,
            Some("true"),
            Some(if value { "true" } else { "false" }),
            if value { "passed" } else { "failed" },
            if value { None } else { Some("false") },
            None,
        )?;
        Ok(value)
    }

    pub fn latency(&mut self, metric: &str, ms: f64) -> Result<()> {
        create_latency_record(
            &self.store,
            &self.step.run_id,
            Some(&self.step.id),
            metric,
            ms,
            Some("wall_clock"),
        )?;
        Ok(())
    }

    pub fn finish(&mut self, status: &str) -> Result<()> {
        self.step = finish_step_record(&self.store, &self.step.id, status, None)?;
        Ok(())
    }
}

pub fn scenario_record_run(
    name: &str,
    family: &str,
    version: &str,
    surface: &str,
) -> Result<ScenarioRun> {
    let store = state_store_for_cli()?;
    let recorder = ScenarioRecorder::new(&store, name, family, version, surface)?;
    Ok(recorder.into_run())
}

pub fn scenario_record_step(
    run_id: &str,
    name: &str,
    kind: &str,
    command_line: Option<&str>,
) -> Result<ScenarioStep> {
    let store = state_store_for_cli()?;
    create_scenario_step(&store, run_id, name, kind, command_line)
}

pub fn scenario_record_assertion(
    run_id: &str,
    step_id: Option<&str>,
    name: &str,
    expected: Option<&str>,
    actual: Option<&str>,
    status: &str,
    failure_category: Option<&str>,
    notes: Option<&str>,
) -> Result<ScenarioAssertion> {
    let store = state_store_for_cli()?;
    create_assertion_record(
        &store,
        run_id,
        step_id,
        name,
        expected,
        actual,
        status,
        failure_category,
        notes,
    )
}

pub fn scenario_record_latency(
    run_id: &str,
    step_id: Option<&str>,
    metric: &str,
    sample_ms: f64,
    capture_method: Option<&str>,
) -> Result<LatencySample> {
    let store = state_store_for_cli()?;
    create_latency_record(&store, run_id, step_id, metric, sample_ms, capture_method)
}

pub fn scenario_finish_step(
    step_id: &str,
    status: &str,
    error_code: Option<&str>,
) -> Result<ScenarioStep> {
    let store = state_store_for_cli()?;
    finish_step_record(&store, step_id, status, error_code)
}

pub fn scenario_finish_run(
    run_id: &str,
    status: &str,
    summary_path: Option<&str>,
) -> Result<ScenarioRun> {
    let store = state_store_for_cli()?;
    finish_run_record(&store, run_id, status, summary_path)
}

fn state_store_for_cli() -> Result<StateStore> {
    StateStore::new(crate::runtime_root()?)
}

fn create_scenario_step(
    store: &StateStore,
    run_id: &str,
    name: &str,
    kind: &str,
    command_line: Option<&str>,
) -> Result<ScenarioStep> {
    ensure_present("run id", run_id)?;
    ensure_present("step name", name)?;
    ensure_present("step kind", kind)?;
    ensure_run_active(store, run_id)?;

    store
        .create_scenario_step(
            &next_record_id(
                IdKind::ScenarioStep,
                &format!("{run_id}_{name}_{}", std::process::id()),
            ),
            run_id.trim(),
            name.trim(),
            kind.trim(),
            command_line
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            &utc_timestamp(),
        )
        .context("insert scenario step")
}

fn create_assertion_record(
    store: &StateStore,
    run_id: &str,
    step_id: Option<&str>,
    name: &str,
    expected: Option<&str>,
    actual: Option<&str>,
    status: &str,
    failure_category: Option<&str>,
    notes: Option<&str>,
) -> Result<ScenarioAssertion> {
    ensure_present("run id", run_id)?;
    ensure_present("assertion name", name)?;
    ensure_present("assertion status", status)?;
    ensure_run_active(store, run_id)?;
    if let Some(step_id) = step_id {
        ensure_step_belongs_to_run(store, step_id, run_id)?;
        ensure_step_active(store, step_id)?;
    }

    let assertion = ScenarioAssertion {
        id: next_record_id(
            IdKind::ScenarioAssertion,
            &format!("{run_id}_{name}_{}", std::process::id()),
        ),
        run_id: run_id.trim().to_string(),
        step_id: step_id.map(str::trim).map(str::to_string),
        assertion_name: name.trim().to_string(),
        expected_value: expected
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        actual_value: actual
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        status: status.trim().to_string(),
        failure_category: failure_category
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        notes: notes
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    };
    store
        .insert_scenario_assertion(&assertion)
        .context("insert scenario assertion")?;
    Ok(assertion)
}

fn create_latency_record(
    store: &StateStore,
    run_id: &str,
    step_id: Option<&str>,
    metric: &str,
    sample_ms: f64,
    capture_method: Option<&str>,
) -> Result<LatencySample> {
    ensure_present("run id", run_id)?;
    ensure_present("metric name", metric)?;
    ensure!(sample_ms >= 0.0, "sample_ms must be non-negative");
    ensure_run_active(store, run_id)?;
    if let Some(step_id) = step_id {
        ensure_step_belongs_to_run(store, step_id, run_id)?;
        ensure_step_active(store, step_id)?;
    }

    let sample = LatencySample {
        id: next_record_id(
            IdKind::LatencySample,
            &format!("{run_id}_{metric}_{}", std::process::id()),
        ),
        run_id: run_id.trim().to_string(),
        step_id: step_id.map(str::trim).map(str::to_string),
        metric_name: metric.trim().to_string(),
        sample_ms: sample_ms.into(),
        capture_method: capture_method
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    };
    store
        .insert_latency_sample(&sample)
        .context("insert latency sample")?;
    Ok(sample)
}

fn finish_step_record(
    store: &StateStore,
    step_id: &str,
    status: &str,
    error_code: Option<&str>,
) -> Result<ScenarioStep> {
    ensure_present("step id", step_id)?;
    ensure_present("step status", status)?;
    let mut step = store
        .get_scenario_step(step_id)?
        .ok_or_else(|| anyhow!("unknown scenario step {step_id}"))?;
    ensure!(
        step.finished_at.is_none() && step.status == "running",
        "scenario step {step_id} is already finished with status {}",
        step.status
    );
    step.finished_at = Some(utc_timestamp());
    step.status = status.trim().to_string();
    step.error_code = error_code
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| step.error_code.clone());
    store
        .update_scenario_step(&step)
        .context("finish scenario step")?;
    Ok(step)
}

fn finish_run_record(
    store: &StateStore,
    run_id: &str,
    status: &str,
    summary_path: Option<&str>,
) -> Result<ScenarioRun> {
    ensure_present("run id", run_id)?;
    ensure_present("run status", status)?;
    let mut run = store
        .get_scenario_run(run_id)?
        .ok_or_else(|| anyhow!("unknown scenario run {run_id}"))?;
    ensure!(
        run.finished_at.is_none() && run.status == "running",
        "scenario run {run_id} is already finished with status {}",
        run.status
    );
    ensure!(
        store
            .list_scenario_steps(run_id)?
            .iter()
            .all(|step| step.finished_at.is_some()),
        "scenario run {run_id} still has active steps"
    );
    run.finished_at = Some(utc_timestamp());
    run.status = status.trim().to_string();
    run.summary_path = summary_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| run.summary_path.clone());
    store
        .update_scenario_run(&run)
        .context("finish scenario run")?;
    Ok(run)
}

fn capture_environment_fingerprint(run_id: &str) -> EnvironmentFingerprint {
    let platform = command_output("uname", &["-s"], None)
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| env::consts::OS.to_string());
    let arch = command_output("uname", &["-m"], None).unwrap_or_else(|| env::consts::ARCH.into());
    let os_version = command_output("uname", &["-s", "-r"], None)
        .or_else(|| command_output("uname", &["-r"], None));
    let rust_version = env::var("RUST_VERSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| command_output("rustc", &["--version"], None));
    let cargo_version = env::var("CARGO_VERSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| command_output("cargo", &["--version"], None));
    let (chrome_channel, chrome_version) = first_installed_browser_details();

    EnvironmentFingerprint {
        id: next_record_id(
            IdKind::EnvironmentFingerprint,
            &format!("{run_id}_environment"),
        ),
        run_id: run_id.to_string(),
        platform,
        arch,
        os_version,
        rust_version,
        cargo_version,
        chrome_channel,
        chrome_version,
    }
}

fn first_installed_browser_details() -> (Option<String>, Option<String>) {
    discover_installations()
        .into_iter()
        .find(|install| install.installed)
        .map(|install| {
            let version = command_output(&install.binary_path, &["--version"], None)
                .map(|text| extract_version_token(&text).unwrap_or(text));
            (Some(install.channel.as_str().to_string()), version)
        })
        .unwrap_or((None, None))
}

fn extract_version_token(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| token.contains('.') && token.chars().any(|ch| ch.is_ascii_digit()))
        .map(str::to_string)
}

fn ensure_present(label: &str, value: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "{label} must not be empty");
    Ok(())
}

fn ensure_run_active(store: &StateStore, run_id: &str) -> Result<()> {
    let run = store
        .get_scenario_run(run_id)?
        .ok_or_else(|| anyhow!("unknown scenario run {run_id}"))?;
    ensure!(
        run.finished_at.is_none() && run.status == "running",
        "scenario run {run_id} is already finished with status {}",
        run.status
    );
    Ok(())
}

fn ensure_step_belongs_to_run(store: &StateStore, step_id: &str, run_id: &str) -> Result<()> {
    let step = store
        .get_scenario_step(step_id)?
        .ok_or_else(|| anyhow!("unknown scenario step {step_id}"))?;
    ensure!(
        step.run_id == run_id,
        "scenario step {step_id} does not belong to run {run_id}"
    );
    Ok(())
}

fn ensure_step_active(store: &StateStore, step_id: &str) -> Result<()> {
    let step = store
        .get_scenario_step(step_id)?
        .ok_or_else(|| anyhow!("unknown scenario step {step_id}"))?;
    ensure!(
        step.finished_at.is_none() && step.status == "running",
        "scenario step {step_id} is already finished with status {}",
        step.status
    );
    Ok(())
}

fn git_output(args: &[&str]) -> Option<String> {
    let workspace = crate::workspace_root().ok()?;
    command_output("git", args, Some(&workspace))
}

fn command_output(program: &str, args: &[&str], current_dir: Option<&Path>) -> Option<String> {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return Some(stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        None
    } else {
        Some(stderr)
    }
}

fn next_record_id(kind: IdKind, seed: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = format!(
        "{seed}_{}_{}_{}",
        OffsetDateTime::now_utc().unix_timestamp_nanos(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    StableId::new(kind, suffix).into_string()
}

#[cfg(test)]
mod tests {
    use super::{ScenarioRecorder, extract_version_token};
    use crate::StageOneRuntime;
    use pengu_mesh_state::StateStore;
    use tempfile::tempdir;

    #[test]
    fn extracts_numeric_browser_version_tokens() {
        assert_eq!(
            extract_version_token("Google Chrome 147.0.7719.3 dev"),
            Some("147.0.7719.3".to_string())
        );
        assert_eq!(
            extract_version_token("Chromium 146.0.0.0"),
            Some("146.0.0.0".to_string())
        );
        assert_eq!(extract_version_token("Google Chrome Dev"), None);
    }

    #[test]
    fn scenario_recorder_round_trips_through_runtime_queries() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");
        let runtime = StageOneRuntime::new_in_root(tempdir.path(), "pengu-mesh-recorder-test")
            .expect("runtime");

        let mut recorder = ScenarioRecorder::new(
            &store,
            "startup-readiness",
            "startup-readiness",
            "v1",
            "cli",
        )
        .expect("scenario recorder");
        let run_id = recorder.run().id.clone();

        let mut health = recorder.step("health", "command").expect("health step");
        let step_id = health.step().id.clone();
        assert!(
            health
                .assert_true("health ok", true)
                .expect("health assertion")
        );
        assert!(
            health
                .assert_eq("health state", "ready", "ready")
                .expect("health eq assertion")
        );
        health.latency("health", 12.5).expect("health latency");
        health.finish("passed").expect("finish health step");

        recorder.finish("passed").expect("finish scenario");

        let list = runtime
            .scenario_list(Some("startup-readiness"), 10)
            .expect("scenario list");
        assert_eq!(list.runs.len(), 1);
        assert_eq!(list.runs[0].id, run_id);
        assert_eq!(list.runs[0].status, "passed");

        let detail = runtime
            .scenario_run_detail(&run_id)
            .expect("scenario detail");
        assert_eq!(detail.run.id, run_id);
        assert_eq!(detail.steps.len(), 1);
        assert_eq!(detail.steps[0].id, step_id);
        assert_eq!(detail.steps[0].status, "passed");
        assert_eq!(detail.assertions.len(), 2);
        assert!(
            detail
                .assertions
                .iter()
                .all(|assertion| assertion.status == "passed")
        );
        assert_eq!(detail.latency_samples.len(), 1);
        assert_eq!(detail.latency_samples[0].metric_name, "health");
        assert_eq!(detail.latency_samples[0].sample_ms.into_inner(), 12.5);
        assert!(detail.environment_fingerprint.is_some());
        assert_eq!(
            detail
                .environment_fingerprint
                .as_ref()
                .expect("environment fingerprint")
                .run_id,
            run_id
        );
    }

    #[test]
    fn scenario_recorder_rejects_mutation_after_finish() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");

        let mut recorder = ScenarioRecorder::new(
            &store,
            "structured-failure",
            "structured-failure",
            "v1",
            "cli",
        )
        .expect("scenario recorder");
        let mut step = recorder.step("probe", "command").expect("scenario step");
        step.assert_true("probe ok", true)
            .expect("step assertion before finish");
        step.finish("passed").expect("finish step");
        assert!(
            step.assert_true("late assertion", true)
                .expect_err("late assertion should fail")
                .to_string()
                .contains("already finished")
        );
        assert!(
            step.latency("late latency", 1.0)
                .expect_err("late latency should fail")
                .to_string()
                .contains("already finished")
        );

        recorder.finish("passed").expect("finish run");
        assert!(
            recorder
                .step("late step", "command")
                .expect_err("late step should fail")
                .to_string()
                .contains("already finished")
        );
        assert!(
            recorder
                .finish("passed")
                .expect_err("second finish should fail")
                .to_string()
                .contains("already finished")
        );
    }

    #[test]
    fn scenario_run_finish_requires_all_steps_to_be_terminal() {
        let tempdir = tempdir().expect("tempdir");
        let store = StateStore::new(tempdir.path()).expect("state store");

        let mut recorder = ScenarioRecorder::new(
            &store,
            "startup-readiness",
            "startup-readiness",
            "v1",
            "cli",
        )
        .expect("scenario recorder");
        let _step = recorder.step("health", "command").expect("health step");

        assert!(
            recorder
                .finish("passed")
                .expect_err("finish should reject active steps")
                .to_string()
                .contains("still has active steps")
        );
    }
}
