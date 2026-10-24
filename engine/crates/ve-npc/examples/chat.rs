#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "interactive development example communicates with its user"
)]
//! Interactive development chat with one NPC and a local command provider.
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};
use ve_npc::{
    Character, CommandProvider, MindBudget, NpcHost, NpcSpec, Script, run_script, run_until_idle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut npc_file = None;
    let mut character_file = None;
    let mut script_file = None;
    let mut command = None;
    let mut timeout = 60.0_f64;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--npc-file" => npc_file = args.next(),
            "--character" => character_file = args.next(),
            "--script" => script_file = args.next(),
            "--cmd" => command = args.next(),
            "--timeout" => timeout = args.next().ok_or("missing --timeout value")?.parse()?,
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    let command = command.ok_or("usage: chat [--character path.json | --npc-file path.json] [--script path.json] --cmd \"shell command\" [--timeout secs]")?;
    let character = if let Some(path) = character_file {
        Character::from_json(&fs::read_to_string(path)?)?
    } else if let Some(path) = npc_file {
        Character {
            npc: serde_json::from_str::<NpcSpec>(&fs::read_to_string(path)?)?,
            secrets: vec![],
            examples: vec![],
            deflections: vec![],
            filler: None,
            fallback: vec![],
        }
    } else {
        Character::example_baker()
    };
    let npc = character.npc.name.clone();
    let mut host = NpcHost::new(MindBudget::default());
    character.apply(&mut host)?;
    let mut provider = CommandProvider::new("sh")
        .args(["-c", &command])
        .with_timeout(Duration::from_secs_f64(timeout));
    if let Some(path) = script_file {
        return run_script_mode(&path, &npc, &mut host, &mut provider);
    }
    run_interactive(&npc, &mut host, &mut provider)
}

fn run_script_mode(
    path: &str,
    npc: &str,
    host: &mut NpcHost,
    provider: &mut CommandProvider,
) -> Result<(), Box<dyn std::error::Error>> {
    let script = Script::from_json(&fs::read_to_string(path)?)?;
    let started = Instant::now();
    let mut millis = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let report = run_script(host, npc, &script, provider, &mut millis);
    println!("{}", report.to_json());
    eprintln!(
        "{} turns; {:.0}% valid; {:.0}% fallback; {} ms mean",
        report.summary.turns,
        report.summary.valid_rate * 100.0,
        report.summary.fallback_rate * 100.0,
        report.summary.mean_ms
    );
    Ok(())
}

fn run_interactive(
    npc: &str,
    host: &mut NpcHost,
    provider: &mut CommandProvider,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    loop {
        print!("you> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if line == ":memory" {
            if let Some(memory) = host.memory(npc) {
                for item in memory.recent(10) {
                    println!("- {}", item.text);
                }
            }
            continue;
        }
        if let Some(path) = line.strip_prefix(":save ") {
            fs::write(path, host.to_json())?;
            println!("saved {path}");
            continue;
        }
        let now = 1.0;
        host.player_says(npc, line, now)?;
        let started = Instant::now();
        run_until_idle(host, provider, now, 32);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        if let Some(turn) = host
            .session(npc)
            .and_then(|session| session.transcript().last())
        {
            println!(
                "anna{}> {}",
                if turn.fallback { " [canned]" } else { "" },
                turn.text
            );
        }
        let delivery = host.last_delivery(npc);
        let valid = delivery.map_or(0, |d| d.intents.len());
        let rejected = delivery.map_or(0, |d| d.rejected.len());
        eprintln!("model {elapsed:.0} ms; intents valid={valid} rejected={rejected}");
    }
    Ok(())
}
