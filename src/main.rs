mod contract;
mod domain;
mod engine;
mod paths;
mod runner;
mod serde_util;
mod storage;

fn main() -> std::process::ExitCode {
    runner::run_process()
}
