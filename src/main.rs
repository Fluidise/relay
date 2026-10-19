use lexopt::prelude::*;
use std::process::ExitCode;

const HELP: &str = "\
fluidise-relay

Standalone agent-first Git hook manager.

Usage: fluidise-relay [OPTIONS]

Options:
  -h, --help     Print help information
  -v, --version  Print version information
";

fn main() -> ExitCode {
    match parse_args() {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::from(2)
        }
    }
}

fn parse_args() -> Result<u8, lexopt::Error> {
    let mut parser = lexopt::Parser::from_env();
    let mut show_help = false;
    let mut show_version = false;

    while let Some(arg) = parser.next()? {
        match arg {
            Short('h') | Long("help") => {
                show_help = true;
            }
            Short('v') | Long("version") => {
                show_version = true;
            }
            _ => return Err(arg.unexpected()),
        }
    }

    if show_help {
        print!("{HELP}");
    } else if show_version {
        println!("fluidise-relay {}", env!("CARGO_PKG_VERSION"));
    } else {
        println!("fluidise-relay {}", env!("CARGO_PKG_VERSION"));
        println!("Run with --help for available options.");
    }

    Ok(0)
}
