use crate::api::QBitApi;
use crate::models::{ScriptResult, Torrent};
use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub struct ScriptRunner {
    command: String,
}

impl ScriptRunner {
    pub fn new(command: String) -> Self {
        Self { command }
    }

    pub async fn run(&self, torrent: &Torrent, api: &QBitApi) -> Result<ScriptResult> {
        let mut result = ScriptResult::default();
        result.name = torrent.name.clone();
        result.hash = torrent.hash.clone();
        result.script = self.command.clone();
        if self.command.is_empty() {
            return Ok(result);
        }
        let files_len = api.get_torrent_files(&torrent.hash).await?.len();
        let mut parts = self.command.split_whitespace();
        let program = match parts.next() {
            Some(p) => p,
            None => anyhow::bail!("Empty command"),
        };
        let args: Vec<String> = parts
            .map(|p| Self::process_arg(p, torrent, &files_len))
            .collect();
        let mut cmd = Command::new(program);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .env("TORRENT_NAME", &torrent.name)
            .env("TORRENT_CATEGORY", &torrent.category)
            .env("TORRENT_TAGS", &torrent.tags)
            .env("TORRENT_CONTENT_PATH", &torrent.content_path)
            .env("TORRENT_ROOT_PATH", &torrent.root_path)
            .env("TORRENT_SAVE_PATH", &torrent.save_path)
            .env("TORRENT_NUM_FILES", files_len.to_string())
            .env("TORRENT_SIZE", torrent.size.to_string())
            .env("TORRENT_TRACKER", &torrent.tracker)
            .env(
                "TORRENT_INFOHASH_V1",
                torrent
                    .infohash_v1
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .env(
                "TORRENT_INFOHASH_V2",
                torrent
                    .infohash_v2
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            )
            .env("TORRENT_ID", &torrent.hash);
        for arg in &args {
            cmd.arg(arg);
        }
        let mut child = cmd.spawn().context("Failed to spawn script process")?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut stdout_handle = None;
        let mut stderr_handle = None;
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            stdout_handle = Some(tokio::spawn(async move {
                let mut output = String::new();
                while let Ok(Some(line)) = reader.next_line().await {
                    output.push_str(&line);
                    output.push('\n');
                }
                output
            }));
        }
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            stderr_handle = Some(tokio::spawn(async move {
                let mut output = String::new();
                while let Ok(Some(line)) = reader.next_line().await {
                    output.push_str(&line);
                    output.push('\n');
                }
                output
            }));
        }
        let status = child.wait().await.context("Script execution failed")?;
        if let Some(handle) = stdout_handle {
            if let Ok(output) = handle.await {
                result.stdout = output;
            }
        }
        if let Some(handle) = stderr_handle {
            if let Ok(output) = handle.await {
                result.stderr = output;
            }
        }
        result.exit_code = status.code();
        result.success = status.success();
        result.completed_at = chrono::Utc::now();
        Ok(result)
    }

    fn process_arg(arg: &str, torrent: &Torrent, files_len: &usize) -> String {
        let mut arg = arg.to_string();
        arg = arg.replace("%N", &torrent.name);
        arg = arg.replace("%L", &torrent.category);
        arg = arg.replace("%G", &torrent.tags);
        arg = arg.replace("%F", &torrent.content_path);
        arg = arg.replace("%R", &torrent.root_path);
        arg = arg.replace("%D", &torrent.save_path);
        arg = arg.replace("%C", files_len.to_string().as_str());
        arg = arg.replace("%Z", &torrent.size.to_string());
        arg = arg.replace("%T", &torrent.tracker);
        arg = arg.replace(
            "%I",
            &torrent
                .infohash_v1
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        );
        arg = arg.replace(
            "%J",
            &torrent
                .infohash_v2
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        );
        arg = arg.replace("%K", &torrent.hash);
        arg
    }
}
