mod adapter;
mod filesystem;
mod fixtures;
mod output;
mod report;
mod run;
mod scenarios;
mod types;
mod verify;
mod verify_checks;

#[cfg(test)]
mod contract_tests;
#[cfg(test)]
mod tests;

use std::env;
use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let Some((command, options)) = arguments.split_first() else {
        usage();
        return ExitCode::from(2);
    };
    if command != "run" {
        usage();
        return ExitCode::from(2);
    }
    match run::command(options, &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("agent eval failed: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!(
        "usage: scripts/run-agent-eval.sh run --adapter executable [--run-root path] [--scenario id] [--report-dir path --report-name name]"
    );
}
