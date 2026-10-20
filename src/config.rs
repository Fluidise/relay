use crate::support::*;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct CommandSpec {
    pub id: String,
    pub program: String,
    pub args: Vec<String>,
    pub modifies: bool,
}
pub struct Config {
    pub source: PathBuf,
    pub timeout: Duration,
    pub commands: Vec<CommandSpec>,
    pub sections: Vec<String>,
}
impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = read_bounded(path, CONFIG_LIMIT)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| Failure::new("config.encoding", "Configuration must be UTF-8"))?;
        let document =
            toml::de::DeTable::parse(text).map_err(|e| Failure::new("config.syntax", e))?;
        let doc = document.get_ref();
        let hooks = doc
            .get("hooks")
            .and_then(|v| v.get_ref().as_table())
            .ok_or_else(|| Failure::new("config.hooks_missing", "A hooks table is required"))?;
        let mut diagnostics = Vec::new();
        let mut complete = true;
        let mut add = |message: String| {
            if diagnostics.len() < 20 {
                diagnostics.push(json!({"code":"config.invalid","message":message.chars().take(512).collect::<String>()}));
                if diagnostics.len() == 20 {
                    complete = false;
                }
            } else {
                complete = false;
            }
        };
        for k in hooks.keys() {
            if !["version", "timeout-seconds", "commit-msg"].contains(&k.get_ref().as_ref()) {
                add(format!("Unknown hooks key: {k:?}"));
            }
        }
        if hooks.get("version").and_then(|v| {
            v.get_ref()
                .as_integer()
                .and_then(|n| i64::from_str_radix(n.as_str(), n.radix()).ok())
        }) != Some(1)
        {
            add("hooks.version must be integer 1".into());
        }
        let seconds = match hooks.get("timeout-seconds") {
            None => 60,
            Some(v) => match v
                .get_ref()
                .as_integer()
                .and_then(|n| i64::from_str_radix(n.as_str(), n.radix()).ok())
                .filter(|n| *n > 0)
            {
                Some(n) => n as u64,
                None => {
                    add("timeout-seconds must be a positive integer".into());
                    60
                }
            },
        };
        let timeout = Duration::from_secs(seconds);
        if std::time::Instant::now().checked_add(timeout).is_none() {
            add("timeout-seconds cannot be represented by the monotonic clock".into());
        }
        let list = hooks.get("commit-msg").and_then(|v| v.get_ref().as_array());
        if list.is_none_or(|a| a.is_empty() || a.len() > 64) {
            add("commit-msg requires between 1 and 64 commands".into());
        }
        let mut commands = Vec::new();
        let mut ids = HashSet::new();
        for entry in list.into_iter().flatten().take(64) {
            let Some(t) = entry.get_ref().as_table() else {
                add("Each command must be a table".into());
                continue;
            };
            for k in t.keys() {
                if !["id", "program", "args", "modifies-message"].contains(&k.get_ref().as_ref()) {
                    add(format!("Unknown command key: {k:?}"));
                }
            }
            let id = t.get("id").and_then(|v| v.get_ref().as_str()).unwrap_or("");
            if id.is_empty()
                || id.len() > 64
                || !id.as_bytes()[0].is_ascii_lowercase()
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                || !ids.insert(id.to_owned())
            {
                add(
                    "Command ids must be unique ASCII [a-z][a-z0-9-]* of at most 64 characters"
                        .into(),
                );
            }
            let program = t
                .get("program")
                .and_then(|v| v.get_ref().as_str())
                .unwrap_or("");
            if program.is_empty() || program.contains('\0') {
                add(format!("Invalid program for command {id}"));
            }
            let mut args = Vec::new();
            match t.get("args").and_then(|v| v.get_ref().as_array()) {
                None => add(format!("Explicit args array required for command {id}")),
                Some(a) => {
                    for v in a {
                        match v.get_ref().as_str().filter(|s| !s.contains('\0')) {
                            Some(s) => args.push(s.to_owned()),
                            None => add(format!(
                                "Arguments must be NUL-free strings for command {id}"
                            )),
                        }
                    }
                }
            }
            let modifies = match t.get("modifies-message") {
                None => false,
                Some(v) => match v.get_ref().as_bool() {
                    Some(b) => b,
                    None => {
                        add(format!("modifies-message must be boolean for command {id}"));
                        false
                    }
                },
            };
            commands.push(CommandSpec {
                id: id.into(),
                program: program.into(),
                args,
                modifies,
            });
        }
        if !diagnostics.is_empty() {
            return Err(Failure {
                diagnostics,
                complete,
            });
        }
        Ok(Self {
            source: path.to_path_buf(),
            timeout,
            commands,
            sections: doc.keys().map(|k| k.get_ref().to_string()).collect(),
        })
    }
    pub fn effective(&self) -> Value {
        let dir = self
            .source
            .parent()
            .expect("absolute configuration has a parent");
        json!({"version":1,"timeout_seconds":self.timeout.as_secs(),"commands":self.commands.iter().map(|c| {
            let p=Path::new(&c.program);
            let program=if c.program.contains('/') && p.is_relative() {path_value(&dir.join(p))} else {bytes_value(c.program.as_bytes())};
            json!({"id":c.id,"program":program,"args":c.args,"modifies_message":c.modifies})
        }).collect::<Vec<_>>()})
    }
    pub fn approval(&self) -> String {
        digest("fluidise-relay.approval.v1", &self.effective())
    }
    pub fn check_programs(&self, work_dir: &Path) -> Result<()> {
        for c in &self.commands {
            resolve_program(
                &c.program,
                self.source.parent().expect("config parent"),
                work_dir,
            )?;
        }
        Ok(())
    }
}
