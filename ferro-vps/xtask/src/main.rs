//! Build and quality-task automation for the `Ferro-VPS` workspace.
//!
//! This is the `cargo-xtask` entry point. It wraps a small, fixed set of
//! `cargo` subcommands so the whole project can be built, formatted, linted and
//! tested with a single command. Run it with:
//!
//! ```text
//! cargo run -p xtask -- <build|check|fmt|fmt-check|lint|test|ci>
//! ```
//!
//! Only the hardcoded tasks below are ever executed; no arbitrary external
//! input is turned into a command.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]

use std::process::{Command, ExitCode};

#[derive(Clone, Copy)]
struct Step {
    name: &'static str,
    args: &'static [&'static str],
}

const BUILD: Step = Step {
    name: "build",
    args: &["build", "--workspace", "--all-targets"],
};

const CHECK: Step = Step {
    name: "check",
    args: &["check", "--workspace", "--all-targets"],
};

const FMT: Step = Step {
    name: "fmt",
    args: &["fmt", "--all"],
};

const FMT_CHECK: Step = Step {
    name: "fmt-check",
    args: &["fmt", "--all", "--", "--check"],
};

const LINT: Step = Step {
    name: "lint",
    args: &[
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ],
};

const TEST: Step = Step {
    name: "test",
    args: &["test", "--workspace"],
};

fn steps_for(task: &str) -> Option<Vec<Step>> {
    let steps = match task {
        "build" => vec![BUILD],
        "check" => vec![CHECK],
        "fmt" => vec![FMT],
        "fmt-check" => vec![FMT_CHECK],
        "lint" => vec![LINT],
        "test" => vec![TEST],
        "ci" => vec![FMT_CHECK, LINT, BUILD, TEST],
        _ => return None,
    };
    Some(steps)
}

fn run_step(step: Step) -> bool {
    println!("==> running `{}`", step.name);
    match Command::new("cargo").args(step.args).status() {
        Ok(status) if status.success() => true,
        Ok(status) => {
            eprintln!("step `{}` failed with {status}", step.name);
            false
        }
        Err(error) => {
            eprintln!("failed to launch cargo for step `{}`: {error}", step.name);
            false
        }
    }
}

fn print_usage() {
    eprintln!("usage: cargo run -p xtask -- <build|check|fmt|fmt-check|lint|test|ci>");
}

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let Some(steps) = steps_for(task.as_str()) else {
        eprintln!("unknown task: {task}");
        print_usage();
        return ExitCode::FAILURE;
    };
    for step in steps {
        if !run_step(step) {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::steps_for;

    #[test]
    fn ci_runs_four_steps() {
        let steps = steps_for("ci").expect("ci should be a known task");
        assert_eq!(steps.len(), 4);
    }

    #[test]
    fn unknown_task_has_no_steps() {
        assert!(steps_for("does-not-exist").is_none());
    }
}
