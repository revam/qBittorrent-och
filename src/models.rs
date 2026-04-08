use anyhow::Error;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Deserialize)]
pub struct Torrent {
    pub hash: String,
    pub name: String,
    pub state: String,
    pub progress: f64,
    pub size: u64,
    pub size_total: u64,
    pub speed_down: u64,
    pub speed_up: u64,
    pub num_seeds: u32,
    pub num_leeches: u32,
    pub num_complete: u32,
    pub num_incomplete: u32,
    pub category: String,
    pub tags: String,
    pub content_path: String,
    pub root_path: String,
    pub save_path: String,
    pub ratio: f64,
    pub added_on: u64,
    pub completion_on: Option<i64>,
    pub tracker: String,
    pub uploaded: u64,
    pub downloaded: u64,
    pub time_active: u64,
    pub seeding_time: Option<u64>,
    pub infohash_v1: Option<String>,
    pub infohash_v2: Option<String>,
}

impl Torrent {
    pub fn has_oc_ok_tag(&self) -> bool {
        self.tags.split(',').any(|t| t.trim() == "oc_ok")
    }

    pub fn has_oc_fail_tag(&self) -> bool {
        self.tags.split(',').any(|t| t.trim() == "oc_fail")
    }

    pub fn script_state(&self) -> ScriptState {
        if self.has_oc_ok_tag() {
            ScriptState::Ok
        } else if self.has_oc_fail_tag() {
            ScriptState::Fail
        } else {
            ScriptState::NotRan
        }
    }
}

impl From<qbit_rs::model::Torrent> for Torrent {
    fn from(lib_torrent: qbit_rs::model::Torrent) -> Self {
        let state_str = lib_torrent
            .state
            .map(|s| format!("{:?}", s))
            .unwrap_or_default();
        Torrent {
            hash: lib_torrent.hash.unwrap_or_default(),
            name: lib_torrent.name.unwrap_or_default(),
            state: state_str,
            progress: lib_torrent.progress.unwrap_or(0.0),
            size: lib_torrent.size.unwrap_or(0) as u64,
            size_total: lib_torrent.total_size.unwrap_or(0) as u64,
            speed_down: lib_torrent.dlspeed.unwrap_or(0) as u64,
            speed_up: lib_torrent.upspeed.unwrap_or(0) as u64,
            num_seeds: lib_torrent.num_seeds.unwrap_or(0) as u32,
            num_leeches: lib_torrent.num_leechs.unwrap_or(0) as u32,
            num_complete: lib_torrent.num_complete.unwrap_or(0) as u32,
            num_incomplete: lib_torrent.num_incomplete.unwrap_or(0) as u32,
            category: lib_torrent.category.unwrap_or_default(),
            tags: lib_torrent.tags.unwrap_or_default(),
            content_path: lib_torrent.content_path.unwrap_or_default(),
            root_path: lib_torrent.root_path.unwrap_or_default(),
            save_path: lib_torrent.save_path.unwrap_or_default(),
            ratio: lib_torrent.ratio.unwrap_or(0.0),
            added_on: lib_torrent.added_on.unwrap_or(0) as u64,
            completion_on: lib_torrent.completion_on,
            tracker: lib_torrent.tracker.unwrap_or_default(),
            uploaded: lib_torrent.uploaded.unwrap_or(0) as u64,
            downloaded: lib_torrent.downloaded.unwrap_or(0) as u64,
            time_active: lib_torrent.time_active.unwrap_or(0) as u64,
            seeding_time: lib_torrent.seeding_time.map(|s| s as u64),
            infohash_v1: match lib_torrent.infohash_v1 {
                Some(s) => {
                    if s.len() != 0 {
                        Some(s)
                    } else {
                        None
                    }
                }
                None => None,
            },
            infohash_v2: match lib_torrent.infohash_v2 {
                Some(s) => {
                    if s.len() != 0 {
                        Some(s)
                    } else {
                        None
                    }
                }
                None => None,
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TorrentFile {
    pub name: String,
    pub size: u64,
    pub progress: f64,
}

impl From<qbit_rs::model::TorrentContent> for TorrentFile {
    fn from(content: qbit_rs::model::TorrentContent) -> Self {
        TorrentFile {
            name: content.name,
            size: content.size as u64,
            progress: content.progress,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Preferences {
    /// Script enabled
    #[serde(rename = "autorun_enabled")]
    pub script_enabled: bool,
    /// Script to run
    #[serde(rename = "autorun_program")]
    pub script: String,
    /// Log directory
    #[serde(rename = "file_log_path")]
    pub log_dir: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PreferencesForUpdate {
    /// Enable or disable the script
    pub script_enabled: Option<bool>,
    /// Set the script to run
    pub script: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptState {
    NotRan,
    Ok,
    Fail,
}

impl ScriptState {
    pub fn symbol(&self) -> &'static str {
        match self {
            ScriptState::NotRan => "?",
            ScriptState::Ok => "✓",
            ScriptState::Fail => "!",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        match self {
            ScriptState::NotRan => ratatui::style::Color::Blue,
            ScriptState::Ok => ratatui::style::Color::Green,
            ScriptState::Fail => ratatui::style::Color::Red,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptResult {
    pub hash: String,
    pub name: String,
    pub script: String,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

impl Default for ScriptResult {
    fn default() -> Self {
        let started_at = chrono::Utc::now();
        Self {
            name: String::new(),
            hash: String::new(),
            script: String::new(),
            stdout: String::new(),
            stderr: String::new(),
            success: false,
            exit_code: None,
            started_at: started_at,
            completed_at: started_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub enum SortField {
    Name,
    AddedAt,
    CompletedAt,
}

impl Default for SortField {
    fn default() -> Self {
        SortField::Name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl Default for SortDirection {
    fn default() -> Self {
        SortDirection::Desc
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SortConfig {
    pub field: SortField,
    pub direction: SortDirection,
}

impl Default for SortConfig {
    fn default() -> Self {
        SortConfig {
            field: SortField::AddedAt,
            direction: SortDirection::Desc,
        }
    }
}

pub fn get_config_dir() -> std::path::PathBuf {
    if let Ok(custom) = std::env::var("QBITTORRENT_OCH_HOME") {
        let path = std::path::PathBuf::from(&custom);
        if path.is_file() {
            eprintln!("Error: QBITTORRENT_OCH_HOME points to a file, not a directory");
            std::process::exit(1);
        }
        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&path) {
            eprintln!("Error: Failed to create config directory: {}", e);
            std::process::exit(1);
        }
        return path;
    }
    std::env::var("XDG_CONFIG_HOME")
        .map(|p| std::path::PathBuf::from(p).join("qb-och"))
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .map(|p| p.join(".config").join("qb-och"))
                .unwrap_or_else(|| std::path::PathBuf::from("~/.config/qb-och"))
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptStateFilter {
    All,
    NotRan,
    Ok,
    Fail,
}

impl ScriptStateFilter {
    pub fn label(&self) -> &'static str {
        match self {
            ScriptStateFilter::All => "All",
            ScriptStateFilter::NotRan => "Not ran",
            ScriptStateFilter::Ok => "Ok",
            ScriptStateFilter::Fail => "Fail",
        }
    }

    pub fn next(self) -> Self {
        match self {
            ScriptStateFilter::All => ScriptStateFilter::NotRan,
            ScriptStateFilter::NotRan => ScriptStateFilter::Ok,
            ScriptStateFilter::Ok => ScriptStateFilter::Fail,
            ScriptStateFilter::Fail => ScriptStateFilter::All,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            ScriptStateFilter::All => ScriptStateFilter::Fail,
            ScriptStateFilter::NotRan => ScriptStateFilter::All,
            ScriptStateFilter::Ok => ScriptStateFilter::NotRan,
            ScriptStateFilter::Fail => ScriptStateFilter::Ok,
        }
    }
    pub fn to_str(self) -> &'static str {
        match self {
            ScriptStateFilter::All => "all",
            ScriptStateFilter::NotRan => "not ran",
            ScriptStateFilter::Ok => "ok",
            ScriptStateFilter::Fail => "fail",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "not ran" => ScriptStateFilter::NotRan,
            "ok" => ScriptStateFilter::Ok,
            "fail" => ScriptStateFilter::Fail,
            _ => ScriptStateFilter::All,
        }
    }
}

impl Default for ScriptStateFilter {
    fn default() -> Self {
        ScriptStateFilter::All
    }
}

#[derive(Debug, Clone)]
pub struct ParsedHost {
    pub protocol: String,
    pub host: String,
    pub port: Option<u16>,
    pub pathname: String,
    pub username: String,
    pub password: String,
}

impl ParsedHost {
    pub fn to_string(&self) -> String {
        build_full_url(&self.protocol, &self.host, self.port, &self.pathname, None)
    }

    pub fn display(&self) -> String {
        build_full_url(
            &self.protocol,
            &self.host,
            self.port,
            &self.pathname,
            Some(&self.username),
        )
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Configuration {
    pub credentials: Credentials,
    pub sort_config: SortConfig,
    #[serde(default)]
    pub last_view: Option<String>,
    #[serde(default)]
    pub last_filter: Option<String>,
    #[serde(default)]
    pub script: Option<String>,
}

impl Configuration {
    pub fn config_path() -> std::path::PathBuf {
        get_config_dir().join("config.toml")
    }

    pub fn log_path() -> std::path::PathBuf {
        get_config_dir().join("execution.log")
    }

    pub fn new() -> Self {
        Self {
            credentials: Credentials::empty(),
            sort_config: SortConfig::default(),
            last_view: None,
            last_filter: None,
            script: None,
        }
    }

    pub fn load() -> Option<Self> {
        let path = Self::config_path();
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        let mut configuration: Configuration = toml::from_str(&content).ok()?;
        let parsed = parse_host_url(&configuration.credentials.host).ok()?;
        if parsed.port.is_none() {
            configuration.credentials.host = build_full_url(
                &parsed.protocol,
                &parsed.host,
                Some(8080),
                &parsed.pathname,
                None,
            );
        } else {
            configuration.credentials.host = parsed.to_string();
        }
        Some(configuration)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn display_url(&self) -> String {
        let parsed = match parse_host_url(&self.credentials.host) {
            Ok(p) => p,
            Err(_) => return self.credentials.host.clone(),
        };
        parsed.display()
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Credentials {
    pub host: String,
    pub username: String,
    pub password: String,
}

impl Credentials {
    pub fn new(host: String, username: String, password: String) -> Self {
        Self {
            host,
            username,
            password,
        }
    }

    pub fn empty() -> Self {
        Self {
            host: String::new(),
            username: String::new(),
            password: String::new(),
        }
    }
}

pub fn parse_host_url(input: &str) -> Result<ParsedHost, Error> {
    let input = input.trim();
    let url = if input.contains("://") {
        Url::parse(input)?
    } else {
        Url::parse(&format!("http://{}", input))?
    };
    if let Some(query) = url.query() {
        anyhow::bail!("URL query string not supported: '?{}'", query);
    }
    if let Some(fragment) = url.fragment() {
        anyhow::bail!("URL fragment not supported: '#{}'", fragment);
    }
    let pathname = url.path().to_string();
    let protocol = url.scheme();
    let host = url.host_str().unwrap_or("");
    let username = url.username().to_string();
    let password = url.password().map(|s| s.to_string()).unwrap_or_default();
    Ok(ParsedHost {
        protocol: protocol.to_string(),
        host: host.to_string(),
        port: url.port(),
        pathname,
        username,
        password,
    })
}

pub fn build_full_url(
    protocol: &str,
    host: &str,
    port: Option<u16>,
    pathname: &str,
    username: Option<&str>,
) -> String {
    let mut url = String::new();
    url.push_str(protocol);
    if url.ends_with("://") {
        // good
    } else if !url.is_empty() {
        url.push_str("://");
    }
    if username.is_some_and(|u| !u.is_empty()) {
        url.push_str(username.unwrap());
        url.push('@');
    }
    url.push_str(host);
    if let Some(p) = port {
        let default_http = protocol == "http://" && p == 80;
        let default_https = protocol == "https://" && p == 443;
        if !default_http && !default_https {
            url.push_str(&format!(":{}", p));
        }
    }
    if !pathname.is_empty() {
        if !pathname.starts_with('/') {
            url.push('/');
        }
        url.push_str(pathname);
    }
    url
}
