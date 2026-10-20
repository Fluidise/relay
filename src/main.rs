mod config;
mod lifecycle;
mod repository;
mod runner;
mod support;

use lexopt::prelude::*;
use serde_json::{Value as Json, json};
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use support::*;

pub const OPERATIONS: &[&str] = &[
    "prepare",
    "install",
    "approve",
    "uninstall",
    "unprepare",
    "run",
    "inspect",
];
#[derive(Default)]
pub struct Options {
    pub operation: String,
    pub target: Option<String>,
    pub repo: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub binary: Option<PathBuf>,
    pub file: Option<PathBuf>,
    pub plan: Option<String>,
    pub human: bool,
    pub display: Option<String>,
}
fn value(parser: &mut lexopt::Parser) -> Result<OsString> {
    let attached = parser.optional_value();
    let out = match attached {
        Some(v) => v,
        None => {
            let v = parser
                .value()
                .map_err(|e| Failure::new("invocation.invalid", e))?;
            if v.to_string_lossy().starts_with('-') {
                return Err(Failure::new(
                    "invocation.invalid",
                    "Use = for option values starting with '-'",
                ));
            }
            v
        }
    };
    if out.is_empty() {
        return Err(Failure::new("invocation.invalid", "Empty option value"));
    }
    Ok(out)
}
fn parse() -> Result<Options> {
    let mut parser = lexopt::Parser::from_env();
    let mut options = Options::default();
    let mut seen = std::collections::HashSet::new();
    while let Some(arg) = parser
        .next()
        .map_err(|e| Failure::new("invocation.invalid", e))?
    {
        match arg {
            Value(v) if options.operation.is_empty() => {
                options.operation = v
                    .into_string()
                    .map_err(|_| Failure::new("invocation.invalid", "Operation must be UTF-8"))?
            }
            Long(k) => {
                if !seen.insert(k.to_owned()) {
                    return Err(Failure::new("invocation.invalid", "Duplicate option"));
                }
                match k {
                    "human" => options.human = true,
                    "help" | "version" => {
                        if options.display.is_some() {
                            return Err(Failure::new(
                                "invocation.invalid",
                                "Help/version must be used alone",
                            ));
                        }
                        options.display = Some(k.into());
                    }
                    "repo" => options.repo = Some(value(&mut parser)?.into()),
                    "config" => options.config = Some(value(&mut parser)?.into()),
                    "binary" => options.binary = Some(value(&mut parser)?.into()),
                    "file" => options.file = Some(value(&mut parser)?.into()),
                    "plan" => {
                        options.plan = Some(value(&mut parser)?.into_string().map_err(|_| {
                            Failure::new("invocation.invalid", "Plan must be UTF-8")
                        })?)
                    }
                    "operation" => {
                        options.target = Some(value(&mut parser)?.into_string().map_err(|_| {
                            Failure::new("invocation.invalid", "Operation must be UTF-8")
                        })?)
                    }
                    _ => {
                        return Err(Failure::new(
                            "invocation.invalid",
                            format!("Unknown option: {k}"),
                        ));
                    }
                }
            }
            Short(c @ ('h' | 'v')) => {
                if options.display.is_some() {
                    return Err(Failure::new("invocation.invalid", "Duplicate display flag"));
                }
                options.display = Some(if c == 'h' { "help" } else { "version" }.into());
            }
            _ => {
                return Err(Failure::new(
                    "invocation.invalid",
                    "Unexpected positional argument or flag",
                ));
            }
        }
    }
    if options.display.is_some() {
        if !options.operation.is_empty()
            || seen.len() > 1
            || options.human
            || options.repo.is_some()
            || options.config.is_some()
            || options.file.is_some()
            || options.plan.is_some()
            || options.binary.is_some()
            || options.target.is_some()
        {
            return Err(Failure::new(
                "invocation.invalid",
                "Help/version must be used alone",
            ));
        }
        return Ok(options);
    }
    if !OPERATIONS.contains(&options.operation.as_str()) {
        return Err(Failure::new(
            "invocation.invalid",
            "An operation is required; use --help",
        ));
    }
    let target = if options.operation == "inspect" {
        options
            .target
            .as_deref()
            .ok_or_else(|| Failure::new("invocation.invalid", "inspect requires --operation"))?
    } else {
        options.operation.as_str()
    };
    if options.operation == "inspect"
        && !["prepare", "install", "approve", "uninstall", "unprepare"].contains(&target)
    {
        return Err(Failure::new(
            "invocation.invalid",
            "Invalid inspection operation",
        ));
    }
    if options.operation != "inspect" && options.target.is_some() {
        return Err(Failure::new(
            "invocation.invalid",
            "--operation is only valid for inspect",
        ));
    }
    if options.operation == "run" {
        if options.file.is_none() || options.plan.is_some() || options.binary.is_some() {
            return Err(Failure::new(
                "invocation.invalid",
                "run requires --file and rejects --plan/--binary",
            ));
        }
    } else if options.file.is_some() {
        return Err(Failure::new(
            "invocation.invalid",
            "--file is only valid for run",
        ));
    }
    if options.operation == "inspect" {
        if options.plan.is_some() {
            return Err(Failure::new(
                "invocation.invalid",
                "inspect does not consume a plan",
            ));
        }
    } else if options.operation != "run" && options.plan.is_none() {
        return Err(Failure::new(
            "invocation.invalid",
            "Mutation requires --plan",
        ));
    }
    if target == "install" {
        if options.binary.as_ref().is_none_or(|p| !p.is_absolute()) {
            return Err(Failure::new(
                "invocation.invalid",
                "install requires --binary with an absolute path",
            ));
        }
    } else if options.binary.is_some() {
        return Err(Failure::new(
            "invocation.invalid",
            "--binary is only valid for install",
        ));
    }
    if !["install", "approve", "run"].contains(&target) && options.config.is_some() {
        return Err(Failure::new(
            "invocation.invalid",
            "This operation does not consume --config",
        ));
    }
    Ok(options)
}
fn render(mut result: Json, mut code: u8, human: bool, hook: bool) -> u8 {
    let mut bytes = serde_json::to_vec(&result).expect("JSON values serialize");
    if bytes.len() + 1 > OUTPUT_LIMIT {
        result = Failure::new("output.limit_exceeded", "Result exceeds 2 MiB")
            .result(result["operation"].as_str().unwrap_or("unknown"));
        bytes = serde_json::to_vec(&result).expect("small JSON");
        code = 2;
    }
    if hook && code == 0 {
        return 0;
    }
    if human && !hook {
        bytes = format!(
            "{}: {}\n",
            result["operation"].as_str().unwrap_or("unknown"),
            result["status"].as_str().unwrap_or("error")
        )
        .into_bytes();
        if let Some(diags) = result["diagnostics"].as_array() {
            for d in diags {
                bytes.extend_from_slice(
                    format!(
                        "{}: {}\n",
                        d["code"].as_str().unwrap_or("error"),
                        d["message"].as_str().unwrap_or("")
                    )
                    .as_bytes(),
                );
            }
        }
        if let Some(id) = result["plan"]["id"].as_str() {
            bytes.extend_from_slice(format!("plan: {id}\n").as_bytes());
            bytes
                .extend_from_slice(&serde_json::to_vec_pretty(&result["plan"]).expect("plan JSON"));
            bytes.push(b'\n');
        }
    } else {
        bytes.push(b'\n');
    }
    let write = if hook {
        std::io::stderr().lock().write_all(&bytes)
    } else {
        std::io::stdout().lock().write_all(&bytes)
    };
    if write.is_err() { 2 } else { code }
}
fn main() -> std::process::ExitCode {
    if let Some(delegation) = lifecycle::delegated_hook() {
        let error = delegation.expect_err("successful delegation replaces the process");
        return std::process::ExitCode::from(render(error.result("delegation"), 2, false, true));
    }
    let hook =
        std::env::var_os("FLUIDISE_RELAY_HOOK").as_deref() == Some(std::ffi::OsStr::new("1"));
    let options = match parse() {
        Ok(o) => o,
        Err(e) => {
            return std::process::ExitCode::from(render(e.result("invocation"), 2, false, hook));
        }
    };
    if let Some(display) = &options.display {
        let text = if display == "version" {
            format!("fluidise-relay {}\n", env!("CARGO_PKG_VERSION"))
        } else {
            String::from(
                "fluidise-relay\n\nStandalone agent-first Git hook manager.\n\nOperations: inspect, prepare, install, approve, run, uninstall, unprepare\n\ninspect --operation <operation> [--repo <path>] [--config <path>] [--binary <absolute-path>]\n<mutation> --plan <id> [--repo <path>] [--config <path>] [--binary <absolute-path>]\nrun --file <path> [--repo <path>] [--config <path>]\n\nJSON by default; --human selects human output.\n--help and --version must be used alone.\n",
            )
        };
        return std::process::ExitCode::from(
            if std::io::stdout().lock().write_all(text.as_bytes()).is_ok() {
                0
            } else {
                2
            },
        );
    }
    let outcome = (|| -> Result<(Json, u8)> {
        let cwd = std::env::current_dir()?;
        let repo = repository::Repo::discover(options.repo.as_deref().unwrap_or(&cwd))?;
        if options.operation == "run" {
            runner::run(&repo, &options)
        } else {
            lifecycle::dispatch(&repo, &options)
        }
    })();
    let (result, code) = match outcome {
        Ok(v) => v,
        Err(e) => {
            let mut v = e.result(&options.operation);
            if options.operation != "inspect" && options.operation != "run" {
                v["changed"] = json!(false);
            }
            (v, 2)
        }
    };
    let mut result = result;
    let interrupted = result
        .as_object_mut()
        .and_then(|o| o.remove("interrupted_signal"))
        .and_then(|v| v.as_i64());
    let code = render(result, code, options.human, hook);
    if let Some(signal) = interrupted {
        let _ = signal_hook::low_level::emulate_default_handler(signal as i32);
        return std::process::ExitCode::from(2);
    }
    std::process::ExitCode::from(code)
}
