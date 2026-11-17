//! The `weaver` command line.
//!
//! One command registry ([`registry`], filled by [`commands`]) drives the parsing of the
//! arguments, the validation of the input, the dispatch, the help, the JSON Schemas and the
//! command reference of the installed skills. The domain logic lives in `weaver-core`, never here.
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`spec`] | What a command is: words, arguments, input and output fields, access, place |
//! | [`registry`] | The commands, found by words; the command reference |
//! | [`commands`] | The commands, one file per object |
//! | [`validate`] | The input checked whole against its fields, every problem with its JSON path |
//! | [`schema`] | JSON Schemas (2020-12) for `--help --json` |
//! | [`help`] | Help texts and the Markdown command reference |
//! | [`exec`] | Running one command, in the CLI or in the app |
//! | [`cli`] | One invocation: arguments, root, input, where it runs, what is printed |
//! | [`client`] | The client of the app socket |
//! | [`round`] | `question new`: submits a round to the app and waits for the developer |
//! | [`open`] | `weaver open [<folder>]`: asks the running app to open a folder or show its welcome window |
//! | [`interrupts`] | What stops a command that waits: signals, a closed standard output |

pub mod cli;
pub mod client;
pub mod commands;
pub mod exec;
pub mod help;
pub mod interrupts;
pub mod open;
pub mod registry;
pub mod round;
pub mod schema;
pub mod spec;
pub mod validate;

use std::process::ExitCode;

use weaver_core::paths::AppDirs;

/// Entry point of the binary.
pub fn run() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let env = cli::Environment::from_process();
    let dirs = AppDirs::resolve().ok();
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            eprintln!("error: cannot find the current folder: {e}");
            return ExitCode::from(1);
        }
    };
    let (stdin, stdout, stderr) = (std::io::stdin(), std::io::stdout(), std::io::stderr());
    let (mut stdin, mut stdout, mut stderr) = (stdin.lock(), stdout.lock(), stderr.lock());
    let mut streams = cli::Streams {
        stdin: &mut stdin,
        out: &mut stdout,
        err: &mut stderr,
        interrupts: &interrupts::ProcessInterrupts,
    };
    ExitCode::from(cli::run_with(
        &args,
        &env,
        dirs.as_ref(),
        &cwd,
        &mut streams,
    ))
}
