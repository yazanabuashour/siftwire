use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;

use crate::adapter;
use crate::filesystem::{
    absolute, create_dir, create_dir_all, create_new_file, display, is_within, repo_root,
};
use crate::fixtures;
use crate::output;
use crate::report::{
    failed_scenario_error, round_seconds, scrub_results, validate_name, write_reduced,
};
use crate::scenarios;
use crate::types::{JobResult, RunOptions, RunResult, Scenario};
use crate::verify;

const RUN_ROOT_MARKER: &str = ".siftwire-agent-eval-root";
const RUN_ROOT_MARKER_CONTENT: &[u8] = b"siftwire-agent-eval-root-v1\n";
const RUN_ROOT_LOCK: &str = ".siftwire-agent-eval.lock";

pub struct RunRoot {
    path: PathBuf,
    _lock: File,
}

impl RunRoot {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for RunRoot {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(self.path.join(RUN_ROOT_LOCK)) {
            eprintln!("agent eval warning: release run-root lock: {error}");
        }
    }
}

pub fn command(arguments: &[String], stdout: &mut impl Write) -> Result<()> {
    let options = parse_options(arguments)?;
    let repository = repo_root()?;
    let selected = select_scenarios(&options.scenario)?;
    let executable = adapter::resolve_executable(&options.adapter)?;
    let run_root = prepare_run_root(&options, &repository)?;
    fixtures::build_binary(&repository, run_root.path())?;

    let started = Instant::now();
    let results = selected
        .into_iter()
        .map(|scenario| run_scenario(&repository, run_root.path(), scenario, &executable))
        .collect::<Vec<_>>();
    let report = RunResult {
        run_root: "<run-root>".to_owned(),
        scenario_count: results.len(),
        results: scrub_results(&display(run_root.path()), results),
        elapsed_seconds: round_seconds(started.elapsed().as_secs_f64()),
    };
    serde_json::to_writer_pretty(&mut *stdout, &report)?;
    writeln!(stdout)?;
    if !options.report_dir.is_empty() {
        write_reduced(
            Path::new(&options.report_dir),
            &options.report_name,
            &report,
        )?;
    }
    if let Some(error) = failed_scenario_error(&report.results) {
        bail!(error);
    }
    Ok(())
}

pub fn prepare_run_root(options: &RunOptions, repository: &Path) -> Result<RunRoot> {
    let (root, created) = if options.run_root.is_empty() {
        let eval_root = user_cache_root()?.join("siftwire/agent-eval");
        create_dir_all(&eval_root, 0o755).context("create eval cache")?;
        (temporary_run_root(&eval_root)?, true)
    } else {
        explicit_run_root(Path::new(&options.run_root), repository)?
    };
    let root = fs::canonicalize(root).context("canonicalize run root")?;
    if is_within(&root, repository) {
        bail!(
            "run root must be outside the repository: {}",
            root.display()
        );
    }
    if created {
        drop(
            create_new_file(&root.join(RUN_ROOT_MARKER), RUN_ROOT_MARKER_CONTENT, 0o600)
                .context("mark eval-owned run root")?,
        );
    } else {
        validate_run_root_marker(&root)?;
    }
    let lock = create_new_file(
        &root.join(RUN_ROOT_LOCK),
        format!("{}\n", std::process::id()).as_bytes(),
        0o600,
    )
    .with_context(|| {
        format!(
            "lock eval run root {}; another harness may own it",
            root.display()
        )
    })?;
    Ok(RunRoot {
        path: root,
        _lock: lock,
    })
}

fn explicit_run_root(requested: &Path, repository: &Path) -> Result<(PathBuf, bool)> {
    let requested = absolute(requested).context("absolute run root")?;
    if is_within(&requested, repository) {
        bail!(
            "run root must be outside the repository: {}",
            requested.display()
        );
    }
    if requested.try_exists().context("inspect run root")? {
        return Ok((requested, false));
    }
    let name = requested
        .file_name()
        .ok_or_else(|| anyhow!("run root must name a dedicated directory"))?
        .to_owned();
    let parent = requested
        .parent()
        .ok_or_else(|| anyhow!("run root must have a parent directory"))?;
    create_dir_all(parent, 0o755).context("create run-root parent")?;
    let parent = fs::canonicalize(parent).context("canonicalize run-root parent")?;
    let root = parent.join(name);
    if is_within(&root, repository) {
        bail!(
            "run root must be outside the repository: {}",
            root.display()
        );
    }
    match create_dir(&root, 0o700) {
        Ok(()) => Ok((root, true)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok((root, false)),
        Err(error) => Err(error).context("create run root"),
    }
}

fn validate_run_root_marker(root: &Path) -> Result<()> {
    let marker = root.join(RUN_ROOT_MARKER);
    let content = fs::read(&marker).with_context(|| {
        format!(
            "run root is not harness-owned; expected marker {}",
            marker.display()
        )
    })?;
    if content != RUN_ROOT_MARKER_CONTENT {
        bail!(
            "run root has an invalid ownership marker: {}",
            marker.display()
        );
    }
    Ok(())
}

fn user_cache_root() -> Result<PathBuf> {
    if let Some(value) = env::var_os("XDG_CACHE_HOME") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Ok(path);
        }
        bail!("resolve user cache: XDG_CACHE_HOME is not an absolute path");
    }
    let home = env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("resolve user cache: neither XDG_CACHE_HOME nor HOME is set"))?;
    Ok(PathBuf::from(home).join(".cache"))
}

fn temporary_run_root(parent: &Path) -> Result<PathBuf> {
    let prefix = format!(
        "run-{}-{}",
        Utc::now().timestamp_micros(),
        std::process::id()
    );
    for nonce in 0_u64..=u64::MAX {
        let path = parent.join(format!("{prefix}-{nonce}"));
        match create_dir(&path, 0o700) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error).context("create run root"),
        }
    }
    bail!("create run root: exhausted unique names")
}

pub fn parse_options(arguments: &[String]) -> Result<RunOptions> {
    let mut options = RunOptions::default();
    let mut positional = Vec::new();
    let mut iterator = arguments.iter();
    while let Some(argument) = iterator.next() {
        if argument == "--" {
            positional.extend(iterator.cloned());
            break;
        }
        if !argument.starts_with('-') || argument == "-" {
            positional.push(argument.to_owned());
            positional.extend(iterator.cloned());
            break;
        }
        if matches!(argument.as_str(), "-h" | "--help" | "-help") {
            bail!("flag: help requested");
        }
        let (name, attached) = split_option(argument);
        let value = match attached {
            Some(value) => value.to_owned(),
            None => iterator
                .next()
                .ok_or_else(|| anyhow!("flag needs an argument: {name}"))?
                .to_owned(),
        };
        match name {
            "-run-root" | "--run-root" => options.run_root = value,
            "-scenario" | "--scenario" => options.scenario = value,
            "--adapter" => options.adapter = value,
            "-report-dir" | "--report-dir" => options.report_dir = value,
            "-report-name" | "--report-name" => options.report_name = value,
            _ => bail!("flag provided but not defined: {name}"),
        }
    }
    if !positional.is_empty() {
        bail!("unexpected positional arguments: {positional:?}");
    }
    if options.adapter.trim().is_empty() {
        bail!("--adapter executable is required; select an implementation explicitly");
    }
    if !options.report_dir.is_empty() && options.report_name.trim().is_empty() {
        bail!("--report-name is required with --report-dir");
    }
    if !options.report_name.is_empty() {
        validate_name(&options.report_name)?;
    }
    Ok(options)
}

fn split_option(argument: &str) -> (&str, Option<&str>) {
    argument
        .split_once('=')
        .map_or((argument, None), |(name, value)| (name, Some(value)))
}

fn select_scenarios(id: &str) -> Result<Vec<Scenario>> {
    let all = scenarios::all();
    if id.trim().is_empty() {
        return Ok(all);
    }
    all.into_iter()
        .find(|scenario| scenario.id == id)
        .map(|scenario| vec![scenario])
        .ok_or_else(|| anyhow!("unknown scenario {id:?}"))
}

fn run_scenario(
    repository: &Path,
    run_root: &Path,
    scenario: Scenario,
    executable: &Path,
) -> JobResult {
    let started = Instant::now();
    let run_dir = run_root.join(scenario.id);
    let workspace = run_dir.join("workspace");
    let database = run_dir.join("siftwire.sqlite");
    let mut result = JobResult {
        scenario_id: scenario.id.to_owned(),
        run_dir: display(&run_dir),
        database: display(&database),
        ..JobResult::default()
    };
    if let Err(error) = reset_workspace(&run_dir, &workspace) {
        return finish(result, started, Some(error));
    }
    if let Err(error) = fixtures::install_skill(repository, &workspace) {
        return finish(result, started, Some(error));
    }
    let scenario = match fixtures::prepare(scenario, &run_dir) {
        Ok(value) => value,
        Err(error) => return finish(result, started, Some(error)),
    };
    result.prompts = scenario
        .turns
        .iter()
        .map(|turn| turn.prompt.clone())
        .collect();
    let request = adapter::request(run_root, &run_dir, result.prompts.clone());
    let parsed = match adapter::run(executable, &run_dir, &request)
        .and_then(|bytes| output::parse(&bytes, &request))
    {
        Ok(value) => value,
        Err(error) => return finish(result, started, Some(error)),
    };
    result.runtime = Some(parsed.runtime);
    result.metrics = parsed.metrics;
    result.final_message = parsed.final_message;
    result.verification = verify::scenario(
        &result.database,
        scenario.id,
        &result.final_message,
        &result.metrics,
    );
    result.passed = result.verification.passed;
    let error = (!result.passed).then(|| anyhow!(result.verification.details.clone()));
    finish(result, started, error)
}

fn reset_workspace(run_dir: &Path, workspace: &Path) -> Result<()> {
    match fs::remove_dir_all(run_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("reset scenario directory"),
    }
    create_dir_all(workspace, 0o755).context("create scenario workspace")
}

fn finish(mut result: JobResult, started: Instant, error: Option<anyhow::Error>) -> JobResult {
    result.seconds = round_seconds(started.elapsed().as_secs_f64());
    if let Some(error) = error {
        result.error = error.to_string();
    }
    result
}
