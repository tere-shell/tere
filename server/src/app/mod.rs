use std::ffi::OsString;
use structopt::StructOpt;
use thiserror::Error;

/// Errors that subcommands may return.
#[derive(Error, Debug)]
enum Error {
    #[error(transparent)]
    Pty(#[from] crate::services::pty::CommandError),
}

/// Internal error type returned by `try_run`.
#[derive(Error, Debug)]
enum RunError {
    #[error(transparent)]
    Arguments(structopt::clap::Error),

    #[error(transparent)]
    Subcommand(Error),
}

fn try_run(args: &[OsString]) -> Result<(), RunError> {
    let opt = Opt::from_iter_safe(args).map_err(RunError::Arguments)?;
    opt.command.run(&opt.global).map_err(RunError::Subcommand)?;
    Ok(())
}

pub fn run() {
    let args: Vec<OsString> = std::env::args_os().collect();
    match try_run(&args) {
        Ok(()) => {}
        Err(RunError::Arguments(error)) => error.exit(),
        Err(error) => {
            eprintln!("{}", error);
            std::process::exit(1);
        }
    };
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Server for web-based shell sessions")]
#[structopt(global_settings(&[
    // Spamming every subcommand usage with `--version` and `help` is just silly.
    structopt::clap::AppSettings::VersionlessSubcommands,
    structopt::clap::AppSettings::DisableHelpSubcommand
]))]
struct Opt {
    #[structopt(flatten)]
    global: GlobalFlags,

    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
pub struct GlobalFlags {
    /// Activate debug mode
    #[structopt(long)]
    pub debug: bool,
}
#[derive(StructOpt, Debug)]
enum Command {
    Serve(Serve),
}

/// Run as server.
/// Not intended for interactive use.
#[derive(StructOpt, Debug)]
enum Serve {
    Pty(crate::services::pty::Command),
}

impl Command {
    fn run(&self, global: &GlobalFlags) -> Result<(), Error> {
        match self {
            Command::Serve(run) => match run {
                Serve::Pty(run) => run.run(global).map_err(Error::from),
            },
        }
    }
}
