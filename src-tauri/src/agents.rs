//! Lets coding agents on this Mac read Redrule's notes. Started with `--mcp`, Redrule serves
//! the meeting store over MCP on stdin and stdout instead of opening its window.
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;
use serde_json::Value;

use crate::core::mcp;
use crate::core::store::MeetingStore;
use crate::core::{Error, Result};

pub const MCP_ARG: &str = "--mcp";
/// The name each agent lists the server under.
const SERVER_NAME: &str = "redrule";

pub fn started_for_mcp() -> bool {
    std::env::args().any(|arg| arg == MCP_ARG)
}

/// Answers newline-delimited JSON-RPC until the agent closes stdin. Read-only.
pub fn serve_mcp() {
    let store = match MeetingStore::default_root().and_then(MeetingStore::new) {
        Ok(store) => store,
        Err(error) => return eprintln!("Redrule's meetings could not be opened. {error}"),
    };
    let mut stdout = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => mcp::handle(&store, &message),
            Err(_) => Some(mcp::parse_error()),
        };
        if let Some(reply) = reply
            && writeln!(stdout, "{reply}").and_then(|()| stdout.flush()).is_err()
        {
            break;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Agent {
    Claude,
    Codex,
    Grok,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub agent: Agent,
    pub connected: bool,
    /// What to run in Terminal when connecting from Redrule doesn't work.
    pub command: String,
}

const AGENTS: [Agent; 3] = [Agent::Claude, Agent::Codex, Agent::Grok];

impl Agent {
    fn add_command(self, server: &str) -> String {
        let server = quote(server);
        match self {
            Agent::Claude => format!("claude mcp add --scope user {SERVER_NAME} -- {server} {MCP_ARG}"),
            Agent::Codex => format!("codex mcp add {SERVER_NAME} -- {server} {MCP_ARG}"),
            Agent::Grok => format!("grok mcp add --scope user {SERVER_NAME} {server} -- {MCP_ARG}"),
        }
    }

    fn cli(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
            Agent::Grok => "Grok Build",
        }
    }

    /// Read from each agent's own config, so connecting from Terminal shows up too.
    fn connected(self) -> bool {
        let Some(home) = dirs::home_dir() else { return false };
        let read = |path: PathBuf| std::fs::read_to_string(path).unwrap_or_default();
        match self {
            Agent::Claude => serde_json::from_str::<Value>(&read(home.join(".claude.json")))
                .is_ok_and(|config| config["mcpServers"].get(SERVER_NAME).is_some()),
            Agent::Codex => toml_has_server(&read(home.join(".codex/config.toml"))),
            Agent::Grok => toml_has_server(&read(home.join(".grok/config.toml"))),
        }
    }
}

fn toml_has_server(config: &str) -> bool {
    config.lines().any(|line| line.trim() == format!("[mcp_servers.{SERVER_NAME}]"))
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn this_executable() -> Result<String> {
    let path = std::env::current_exe().map_err(|e| Error::message(format!("Redrule could not find itself. {e}")))?;
    Ok(path.to_string_lossy().into_owned())
}

pub fn statuses() -> Result<Vec<AgentStatus>> {
    let server = this_executable()?;
    Ok(AGENTS.iter().map(|&agent| AgentStatus { agent, connected: agent.connected(), command: agent.add_command(&server) }).collect())
}

/// Runs the agent's own `mcp add` in a login shell, since apps opened from Finder don't get the Terminal's PATH.
pub fn connect(agent: Agent) -> Result<()> {
    let command = agent.add_command(&this_executable()?);
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let output = Command::new(shell)
        .args(["-lic", &command])
        .output()
        .map_err(|e| Error::message(format!("{} could not be set up. {e}", agent.cli())))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    let reason = if detail.contains("not found") { format!("{} isn't installed, or isn't on your PATH.", agent.cli()) } else { detail.trim().to_string() };
    Err(Error::message(format!("{reason} You can run the command shown in Terminal instead.")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_quote_the_app_path() {
        let server = "/Applications/My Apps/Redrule.app/Contents/MacOS/Redrule";
        assert_eq!(Agent::Claude.add_command(server), format!("claude mcp add --scope user redrule -- '{server}' --mcp"));
        assert_eq!(Agent::Codex.add_command(server), format!("codex mcp add redrule -- '{server}' --mcp"));
        assert_eq!(Agent::Grok.add_command(server), format!("grok mcp add --scope user redrule '{server}' -- --mcp"));
        assert_eq!(quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn finds_the_server_in_toml_configs() {
        assert!(toml_has_server("model = \"x\"\n\n[mcp_servers.redrule]\ncommand = \"/a\"\n"));
        assert!(!toml_has_server("[mcp_servers.redrule.env]\n[mcp_servers.other]\n"));
    }
}
