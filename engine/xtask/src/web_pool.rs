//! Runs the browser test scripts in parallel. Each script starts its own server on an ephemeral
//! port and its own headless Chromium, so independent ones can run side by side; the ones that
//! measure frame pacing against a budget must run alone, so they are kept out of the pool.

#![expect(clippy::print_stdout, reason = "reports progress to the terminal")]

use std::collections::VecDeque;
use std::fs;
use std::process::Command;
use std::sync::Mutex;
use std::time::Instant;

use crate::Failure;

/// Test scripts that compare frame times with a budget; contention would make them flaky.
pub(crate) const TIMING_SENSITIVE: [&str; 3] = ["quality", "perf", "bench-session"];

/// One browser test script and how to start it.
pub(crate) struct Job {
    /// Script path relative to `dir`, such as `tests/engine.mjs`.
    pub(crate) script: String,
    /// Working directory, relative to `engine/`.
    pub(crate) dir: &'static str,
    /// Extra environment variables.
    pub(crate) env: Vec<(&'static str, String)>,
}

impl Job {
    /// A script run as `bun <script>` from `dir`.
    pub(crate) fn new(script: &str, dir: &'static str) -> Self {
        Self {
            script: script.to_owned(),
            dir,
            env: Vec::new(),
        }
    }

    fn label(&self) -> String {
        match self.env.iter().find(|(key, _)| *key == "GALLERY_SHARD") {
            Some((_, shard)) => format!("{} (shard {shard})", self.script),
            None => self.script.clone(),
        }
    }
}

/// The scripts in `web/package.json`'s `test` chain, in order.
pub(crate) fn chain_scripts() -> Result<Vec<String>, Failure> {
    let manifest = fs::read_to_string("web/package.json")?;
    let line = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("\"test\":"))
        .ok_or("no test script in web/package.json")?;
    let command = line
        .split_once(": \"")
        .ok_or("malformed test script")?
        .1
        .trim_end_matches([',', '"']);
    Ok(command
        .split(" && ")
        .filter_map(|step| step.trim().strip_prefix("bun "))
        .map(str::to_owned)
        .collect())
}

/// How many scripts to run at once: half the cores, at most four, or `VE_WEB_JOBS`.
pub(crate) fn workers() -> usize {
    std::env::var("VE_WEB_JOBS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(2, |cores| (cores.get() / 2).clamp(1, 4))
        })
        .max(1)
}

/// Runs `jobs` with `workers` at a time, longest first as given. Each job's output is printed as
/// one block when it ends. Fails, naming every failed script, if any failed.
pub(crate) fn run_all(jobs: Vec<Job>, workers: usize) -> Result<(), Failure> {
    println!("-- web-test: {} scripts, {workers} at a time", jobs.len());
    let queue = Mutex::new(VecDeque::from(jobs));
    let failed = Mutex::new(Vec::<String>::new());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let next = queue.lock().ok().and_then(|mut jobs| jobs.pop_front());
                    let Some(job) = next else { return };
                    if !run_one(&job)
                        && let Ok(mut names) = failed.lock()
                    {
                        names.push(job.label());
                    }
                }
            });
        }
    });
    let names = failed.into_inner().unwrap_or_default();
    if names.is_empty() {
        Ok(())
    } else {
        let commands = names
            .iter()
            .map(|name| {
                let script = name
                    .split(" (shard ")
                    .next()
                    .unwrap_or(name)
                    .rsplit('/')
                    .next()
                    .unwrap_or(name)
                    .trim_end_matches(".mjs");
                format!("cargo xtask web-test dev {script}")
            })
            .collect::<Vec<_>>();
        Err(format!(
            "browser tests failed: {}; rerun failed scripts with: {}",
            names.join(", "),
            commands.join("; ")
        )
        .into())
    }
}

/// Runs one script and prints its whole output at once. Returns whether it passed.
fn run_one(job: &Job) -> bool {
    let started = Instant::now();
    let mut command = Command::new("bun");
    command.arg(&job.script).current_dir(job.dir);
    for (key, value) in &job.env {
        command.env(key, value);
    }
    let output = command.output();
    let seconds = started.elapsed().as_secs_f64();
    match output {
        Ok(output) => {
            let passed = output.status.success();
            println!(
                "== {} {} in {seconds:.0}s\n{}{}",
                job.label(),
                if passed { "passed" } else { "FAILED" },
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            passed
        }
        Err(error) => {
            println!("== {} FAILED to start: {error}", job.label());
            false
        }
    }
}
