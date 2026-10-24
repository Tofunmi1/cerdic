//! Native command-line provider for development and local model clients.
use crate::{NpcHost, Request, Response};
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Runs a command for each prompt, using stdin/stdout as the provider boundary.
#[derive(Clone, Debug)]
pub struct CommandProvider {
    program: String,
    args: Vec<String>,
    timeout: Duration,
}

impl CommandProvider {
    /// Creates a provider that starts `program` with a 60 second timeout.
    #[must_use]
    pub fn new(program: &str) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            timeout: Duration::from_mins(1),
        }
    }

    /// Sets the program arguments, preserving each argument as one string.
    #[must_use]
    pub fn args<I: IntoIterator<Item = S>, S: AsRef<str>>(mut self, args: I) -> Self {
        self.args = args
            .into_iter()
            .map(|arg| arg.as_ref().to_owned())
            .collect();
        self
    }

    /// Sets the maximum time allowed for a command to finish.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Executes one prompt and returns trimmed stdout or a descriptive error.
    pub fn respond(&mut self, request: &Request) -> Response {
        let id = request.id;
        let result = self.run(&request.prompt);
        Response {
            id,
            outcome: result,
        }
    }

    fn run(&self, prompt: &str) -> Result<String, String> {
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("cannot start {}: {error}", self.program))?;
        let Some(mut stdin) = child.stdin.take() else {
            return Err("cannot open command stdin".into());
        };
        let Some(stdout) = child.stdout.take() else {
            return Err("cannot open command stdout".into());
        };
        let Some(stderr) = child.stderr.take() else {
            return Err("cannot open command stderr".into());
        };
        let input = prompt.to_owned();
        let writer = thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
        let out_reader = thread::spawn(move || read_all(stdout));
        let err_reader = thread::spawn(move || read_all(stderr));
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if started.elapsed() < self.timeout => {
                    thread::sleep(Duration::from_millis(10));
                }
                Ok(None) | Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };
        let _ = writer.join();
        if status.is_none() {
            return Err(format!(
                "timed out after {} s",
                duration_seconds(self.timeout)
            ));
        }
        let output = out_reader.join().unwrap_or_default();
        let error = err_reader.join().unwrap_or_default();
        let Some(status) = status else {
            return Err("command did not produce an exit status".into());
        };
        if !status.success() {
            let message: String = error.trim().chars().take(300).collect();
            return Err(format!(
                "exit code {}: {message}",
                status
                    .code()
                    .map_or_else(|| "unknown".into(), |code| code.to_string())
            ));
        }
        let reply = output.trim();
        if reply.is_empty() {
            Err("empty reply".into())
        } else {
            Ok(reply.into())
        }
    }
}

fn read_all(mut reader: impl std::io::Read) -> String {
    let mut output = Vec::new();
    let _ = std::io::Read::read_to_end(&mut reader, &mut output);
    String::from_utf8_lossy(&output).into_owned()
}

fn duration_seconds(duration: Duration) -> String {
    let seconds = duration.as_secs_f64();
    if seconds.fract() == 0.0 {
        format!("{seconds:.0}")
    } else {
        format!("{seconds}")
    }
}

/// Handles pending host requests sequentially until idle or the request limit.
pub fn run_until_idle<P: crate::Provider + ?Sized>(
    host: &mut NpcHost,
    provider: &mut P,
    now: f64,
    max_requests: usize,
) -> usize {
    let mut handled = 0;
    while handled < max_requests {
        let requests = host.mind.take_outbox_limit(max_requests - handled);
        for request in &requests {
            host.requests.insert(request.id, request.npc.clone());
        }
        if requests.is_empty() {
            break;
        }
        for request in requests {
            if handled == max_requests {
                break;
            }
            let response = provider.respond(&request);
            let _ = host.deliver(response, now);
            handled += 1;
        }
    }
    handled
}
