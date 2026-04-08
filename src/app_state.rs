use crate::api::QBitApi;
use crate::models::{
    Configuration, Credentials, Preferences, ScriptResult, ScriptStateFilter, SortConfig,
    SortDirection, SortField, Torrent, TorrentFile,
};
use crate::script_runner::ScriptRunner;
use anyhow::Result;
use ratatui_notifications::{
    Anchor, Animation, AutoDismiss, Level, Notification, Notifications, SizeConstraint, Timing,
};
use std::collections::HashMap;
use std::fs;
use std::sync::mpsc;
use std::time::Duration;

pub enum ScriptEvent {
    Started {
        torrent_name: String,
        torrent_hash: String,
    },
    Completed(ScriptResult),
}

const MAX_LOGS: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetailPane {
    Info,
    Paths,
    Transfer,
    Files,
}

impl DetailPane {
    pub fn next(self) -> Self {
        match self {
            DetailPane::Info => DetailPane::Paths,
            DetailPane::Paths => DetailPane::Transfer,
            DetailPane::Transfer => DetailPane::Files,
            DetailPane::Files => DetailPane::Info,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            DetailPane::Info => DetailPane::Files,
            DetailPane::Paths => DetailPane::Info,
            DetailPane::Transfer => DetailPane::Paths,
            DetailPane::Files => DetailPane::Transfer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    TorrentList,
    LogView,
    VerticalSplit,
    HorizontalSplit,
    Help,
}

impl View {
    pub fn next(self) -> Self {
        match self {
            View::TorrentList => View::VerticalSplit,
            View::LogView => View::HorizontalSplit,
            View::VerticalSplit => View::LogView,
            View::HorizontalSplit => View::TorrentList,
            View::Help => View::Help,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            View::TorrentList => View::HorizontalSplit,
            View::LogView => View::VerticalSplit,
            View::VerticalSplit => View::TorrentList,
            View::HorizontalSplit => View::LogView,
            View::Help => View::Help,
        }
    }

    pub fn to_str(self) -> &'static str {
        match self {
            View::TorrentList => "torrent_list",
            View::LogView => "log_view",
            View::VerticalSplit => "vertical_split",
            View::HorizontalSplit => "horizontal_split",
            View::Help => "torrent_list",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "log_view" => View::LogView,
            "vertical_split" => View::VerticalSplit,
            "horizontal_split" => View::HorizontalSplit,
            _ => View::TorrentList,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    Login,
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoginField {
    Host,
    Username,
    Password,
}

pub struct AppState {
    pub api: QBitApi,
    pub torrents: Vec<Torrent>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub view: View,
    pub previous_view: Option<View>,
    pub help_scroll: usize,
    pub mode: AppMode,
    pub error_message: Option<String>,
    pub preferences: Option<Preferences>,
    pub logs: Vec<ScriptResult>,
    pub log_scroll: usize,
    pub qbit_version: Option<String>,
    pub login_host: String,
    pub login_username: String,
    pub login_password: String,
    pub login_field: LoginField,
    pub pending_login: Option<(String, String, String)>,
    pub sort_config: SortConfig,
    pub notifications: Notifications,
    pub filter: ScriptStateFilter,
    pub last_log_mtime: Option<std::time::SystemTime>,
    pub sync_rid: u64,
    pub script_result_tx: Option<mpsc::Sender<ScriptEvent>>,
    pub show_details: bool,
    pub detail_pane: DetailPane,
    pub file_cache: HashMap<String, Vec<TorrentFile>>,
    pub files_loading: Option<String>,
    pub script: Option<String>,
}

impl AppState {
    pub fn default() -> Self {
        Self {
            api: QBitApi::new(""),
            torrents: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            view: View::TorrentList,
            previous_view: None,
            help_scroll: 0,
            mode: AppMode::Normal,
            error_message: None,
            preferences: None,
            logs: Vec::new(),
            log_scroll: 0,
            qbit_version: None,
            login_host: String::new(),
            login_username: String::new(),
            login_password: String::new(),
            login_field: LoginField::Host,
            pending_login: None,
            sort_config: SortConfig::default(),
            notifications: Notifications::new(),
            filter: ScriptStateFilter::All,
            last_log_mtime: None,
            sync_rid: 0,
            script_result_tx: None,
            show_details: false,
            detail_pane: DetailPane::Info,
            file_cache: HashMap::new(),
            files_loading: None,
            script: None,
        }
    }

    pub fn with_configuration(configuration: &Configuration) -> Self {
        let mut sort_directions = HashMap::new();
        sort_directions.insert(SortField::Name, SortDirection::Asc);
        sort_directions.insert(SortField::CompletedAt, SortDirection::Desc);
        sort_directions.insert(SortField::AddedAt, SortDirection::Desc);
        Self {
            api: QBitApi::with_credentials(&configuration.credentials),
            torrents: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            view: configuration
                .last_view
                .as_deref()
                .map(View::from_str)
                .unwrap_or(View::TorrentList),
            previous_view: None,
            help_scroll: 0,
            mode: AppMode::Normal,
            error_message: None,
            preferences: None,
            logs: Vec::new(),
            log_scroll: 0,
            qbit_version: None,
            login_host: configuration.credentials.host.clone(),
            login_username: configuration.credentials.username.clone(),
            login_password: configuration.credentials.password.clone(),
            login_field: LoginField::Host,
            pending_login: None,
            sort_config: configuration.sort_config.clone(),
            notifications: Notifications::new(),
            filter: configuration
                .last_filter
                .as_deref()
                .map(ScriptStateFilter::from_str)
                .unwrap_or_default(),
            last_log_mtime: None,
            sync_rid: 0,
            script_result_tx: None,
            show_details: false,
            detail_pane: DetailPane::Info,
            file_cache: HashMap::new(),
            files_loading: None,
            script: configuration.script.clone(),
        }
    }

    pub fn get_sort_key(&self) -> impl FnMut(&Torrent, &Torrent) -> std::cmp::Ordering {
        let field = self.sort_config.field;
        let direction = self.sort_config.direction;
        move |a: &Torrent, b: &Torrent| {
            let ord = match field {
                SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortField::AddedAt => a.added_on.cmp(&b.added_on),
                SortField::CompletedAt => a
                    .completion_on
                    .unwrap_or(0)
                    .cmp(&b.completion_on.unwrap_or(0)),
            };
            match direction {
                SortDirection::Asc => ord,
                SortDirection::Desc => ord.reverse(),
            }
        }
    }

    pub fn toggle_sort_direction(&mut self) {
        self.sort_config.direction = match self.sort_config.direction {
            SortDirection::Asc => SortDirection::Desc,
            SortDirection::Desc => SortDirection::Asc,
        };
        self.sort_torrents();
        self.save_sort_config();
    }

    pub fn cycle_sort_field(&mut self, direction: isize) {
        let fields = &[SortField::Name, SortField::CompletedAt, SortField::AddedAt];
        let current = self.sort_config.field;
        let current_idx = fields.iter().position(|&f| f == current).unwrap_or(0);
        let new_idx = ((current_idx as isize + direction + fields.len() as isize)
            % fields.len() as isize) as usize;
        let new_field = fields[new_idx];

        self.sort_config.field = new_field;
        self.sort_torrents();
        self.save_sort_config();
    }

    pub fn cycle_filter_forward(&mut self) {
        self.filter = self.filter.next();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.save_view_filter();
    }

    pub fn cycle_filter_backward(&mut self) {
        self.filter = self.filter.prev();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.save_view_filter();
    }

    pub fn filtered_torrents(&self) -> Vec<&Torrent> {
        self.torrents
            .iter()
            .filter(|t| (t.progress - 1.0).abs() < f64::EPSILON || t.progress >= 1.0)
            .filter(|t| match self.filter {
                ScriptStateFilter::All => true,
                ScriptStateFilter::NotRan => t.script_state() == crate::models::ScriptState::NotRan,
                ScriptStateFilter::Ok => t.script_state() == crate::models::ScriptState::Ok,
                ScriptStateFilter::Fail => t.script_state() == crate::models::ScriptState::Fail,
            })
            .collect()
    }

    pub fn sort_torrents(&mut self) {
        let key_fn = self.get_sort_key();
        self.torrents.sort_by(key_fn);
    }

    fn save_sort_config(&self) {
        if let Some(mut configuration) = Configuration::load() {
            configuration.sort_config = self.sort_config.clone();
            _ = configuration.save();
        }
    }

    fn save_view_filter(&self) {
        if let Some(mut configuration) = Configuration::load() {
            configuration.last_view = Some(self.view.to_str().to_string());
            configuration.last_filter = Some(self.filter.to_str().to_string());
            _ = configuration.save();
        }
    }

    pub fn completed_torrents(&self) -> Vec<Torrent> {
        let mut completed: Vec<Torrent> = self
            .torrents
            .iter()
            .filter(|t| (t.progress - 1.0).abs() < f64::EPSILON || t.progress >= 1.0)
            .cloned()
            .collect();
        let key_fn = self.get_sort_key();
        completed.sort_by(key_fn);
        completed
    }

    pub fn enter_login_mode(&mut self) {
        self.mode = AppMode::Login;
        self.login_field = LoginField::Host;
    }

    pub fn advance_login_field(&mut self) {
        self.login_field = match self.login_field {
            LoginField::Host => LoginField::Username,
            LoginField::Username => LoginField::Password,
            LoginField::Password => LoginField::Host,
        };
    }

    pub fn retreat_login_field(&mut self) {
        self.login_field = match self.login_field {
            LoginField::Host => LoginField::Password,
            LoginField::Username => LoginField::Host,
            LoginField::Password => LoginField::Username,
        };
    }

    pub fn queue_login(&mut self) {
        self.pending_login = Some((
            self.login_host.trim().to_string(),
            self.login_username.trim().to_string(),
            self.login_password.clone(),
        ));
    }

    pub async fn process_login(&mut self) -> Result<()> {
        let (host, username, password) = self
            .pending_login
            .take()
            .ok_or_else(|| anyhow::anyhow!("No pending login"))?;
        let credentials = Credentials::new(host.clone(), username.clone(), password.clone());
        let configuration = Configuration {
            credentials: credentials.clone(),
            sort_config: self.sort_config.clone(),
            last_view: Some(self.view.to_str().to_string()),
            last_filter: Some(self.filter.to_str().to_string()),
            script: self.script.clone(),
        };
        configuration.save()?;
        self.api = QBitApi::with_credentials(&credentials);
        match self.api.test_connection().await {
            Ok(_) => {
                self.login_host = host;
                self.login_username = username;
                self.login_password = password;
                self.mode = AppMode::Normal;
                self.error_message = None;
                self.load_preferences().await?;
                self.refresh().await?;
                Ok(())
            }
            Err(e) => {
                self.show_toast_error("Login Failed", &e.to_string());
                self.mode = AppMode::Login;
                Err(e)
            }
        }
    }

    pub async fn refresh(&mut self) -> Result<()> {
        self.torrents = self.api.get_torrents().await?;
        self.sort_torrents();
        let count = self.filtered_torrents().len();
        if self.selected_index >= count {
            self.selected_index = count.saturating_sub(1);
        }
        Ok(())
    }

    pub async fn sync(&mut self) -> Result<()> {
        let data = self.api.sync(self.sync_rid).await?;
        self.sync_rid = data.rid;
        if data.full_update {
            self.torrents = self.api.get_torrents().await?;
            self.sort_torrents();
        } else {
            // Remove deleted torrents
            if !data.torrents_removed.is_empty() {
                self.torrents
                    .retain(|t| !data.torrents_removed.contains(&t.hash));
            }
            // Apply partial updates (only changed fields are present)
            for (hash, changes) in &data.torrents {
                if let Some(torrent) = self.torrents.iter_mut().find(|t| &t.hash == hash) {
                    if let Some(tags) = changes.get("tags").and_then(|v| v.as_str()) {
                        torrent.tags = tags.to_string();
                    }
                    if let Some(state) = changes.get("state").and_then(|v| v.as_str()) {
                        torrent.state = state.to_string();
                    }
                    if let Some(progress) = changes.get("progress").and_then(|v| v.as_f64()) {
                        torrent.progress = progress;
                    }
                    if let Some(completion_on) =
                        changes.get("completion_on").and_then(|v| v.as_i64())
                    {
                        torrent.completion_on = if completion_on > 0 {
                            Some(completion_on)
                        } else {
                            None
                        };
                    }
                } else if !changes.is_null() {
                    // New torrent appeared — do a full refresh to get complete data
                    self.torrents = self.api.get_torrents().await?;
                    self.sort_torrents();
                    break;
                }
            }
        }
        let count = self.filtered_torrents().len();
        if self.selected_index >= count {
            self.selected_index = count.saturating_sub(1);
        }
        Ok(())
    }

    pub async fn load_preferences(&mut self) -> Result<()> {
        self.preferences = Some(self.api.get_preferences().await?);
        Ok(())
    }

    /// Returns the hash that needs file loading, if any.
    pub fn needs_file_load(&self) -> Option<String> {
        if !self.show_details || self.detail_pane != DetailPane::Files {
            return None;
        }
        let hash = self
            .filtered_torrents()
            .into_iter()
            .nth(self.selected_index)
            .map(|t| t.hash.clone())?;
        if self.file_cache.contains_key(&hash) {
            return None;
        }
        if self.files_loading.as_deref() == Some(hash.as_str()) {
            return None;
        }
        Some(hash)
    }

    pub async fn load_torrent_files(&mut self, hash: String) -> Result<()> {
        self.files_loading = Some(hash.clone());
        match self.api.get_torrent_files(&hash).await {
            Ok(files) => {
                self.file_cache.insert(hash, files);
                self.files_loading = None;
                Ok(())
            }
            Err(e) => {
                self.files_loading = None;
                Err(e)
            }
        }
    }

    pub async fn test_connection(&mut self) -> Result<()> {
        return match self.api.test_connection().await {
            Ok(version) => {
                self.qbit_version = Some(version);
                Ok(())
            }
            Err(e) => Err(e),
        };
    }

    pub fn selected_torrent(&self) -> Option<&Torrent> {
        self.filtered_torrents()
            .into_iter()
            .nth(self.selected_index)
    }

    pub fn move_selection(&mut self, delta: isize) {
        let count = self.filtered_torrents().len();
        let new_index = self.selected_index as isize + delta;
        self.selected_index = new_index.max(0).min(count.saturating_sub(1) as isize) as usize;
        self.adjust_scroll(count);
    }

    pub fn adjust_scroll(&mut self, count: usize) {
        let max_visible = 20;
        let max_scroll = count.saturating_sub(max_visible);
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + max_visible {
            self.scroll_offset = self.selected_index - max_visible + 1;
        }
        self.scroll_offset = self.scroll_offset.min(max_scroll);
    }

    pub fn jump_to(&mut self, position: usize, count: usize) {
        self.selected_index = position.min(count.saturating_sub(1));
        self.adjust_scroll(count);
    }

    pub async fn run_script_on_selected(&mut self) -> Result<()> {
        let torrent = match self.selected_torrent() {
            Some(t) => t.clone(),
            None => return Ok(()),
        };
        let script = self.script.clone().unwrap_or_default();
        if script.is_empty() {
            self.show_toast_error(
                "Error",
                "No auto-run script configured. Run 'qb-och script' first.",
            );
            return Ok(());
        }
        let runner = ScriptRunner::new(script.clone());
        // Run the script; if it takes more than 500ms, send a Started event first
        let api = &self.api.clone();
        let run_fut = runner.run(&torrent, api);
        tokio::pin!(run_fut);
        let script_output = tokio::select! {
            result = &mut run_fut => {
                // Finished within 500ms — no started toast needed
                result
            },
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                // Still running — send started event then wait for completion
                if let Some(ref tx) = self.script_result_tx {
                    _ = tx.send(ScriptEvent::Started {
                        torrent_name: torrent.name.clone(),
                        torrent_hash: torrent.hash.clone(),
                    });
                }
                run_fut.await
            },
        };
        let result = match script_output {
            Ok(r) => r,
            Err(e) => {
                let mut r = ScriptResult::default();
                r.name = torrent.name.clone();
                r.hash = torrent.hash.clone();
                r.script = script;
                r.stderr = e.to_string();
                r
            }
        };
        let tag = if result.success { "oc_ok" } else { "oc_fail" };
        _ = self
            .api
            .add_tags(&[torrent.hash.clone()], &[tag.to_string()])
            .await;
        self.update_torrent_tag(&torrent.hash, tag);
        _ = self.save_log_to_file(&result);

        if let Some(ref tx) = self.script_result_tx {
            _ = tx.send(ScriptEvent::Completed(result));
        }
        Ok(())
    }

    pub async fn run_script_on_all_completed(&mut self) -> Result<()> {
        let completed = self.completed_torrents();
        if completed.is_empty() {
            self.show_toast_error("Error", "No completed/seeding torrents found");
            return Ok(());
        }
        let script = self.script.clone().unwrap_or_default();
        if script.is_empty() {
            self.show_toast_error(
                "Error",
                "No auto-run script configured. Run 'qb-och script' first.",
            );
            return Ok(());
        }
        let runner = ScriptRunner::new(script.clone());
        let api = &self.api.clone();
        for torrent in completed {
            let result = match runner.run(&torrent, api).await {
                Ok(r) => r,
                Err(e) => {
                    let mut r = ScriptResult::default();
                    r.name = torrent.name.clone();
                    r.hash = torrent.hash.clone();
                    r.script = script.clone();
                    r.stderr = e.to_string();
                    r
                }
            };
            let tag = if result.success { "oc_ok" } else { "oc_fail" };
            _ = self
                .api
                .add_tags(&[torrent.hash.clone()], &[tag.to_string()])
                .await;
            self.update_torrent_tag(&torrent.hash, tag);
            _ = self.save_log_to_file(&result);

            if let Some(ref tx) = self.script_result_tx {
                _ = tx.send(ScriptEvent::Completed(result));
            }
        }
        Ok(())
    }

    pub fn cycle_view_forward(&mut self) {
        self.view = self.view.next();
        self.save_view_filter();
    }

    pub fn cycle_view_backward(&mut self) {
        self.view = self.view.prev();
        self.save_view_filter();
    }

    pub fn enter_help(&mut self) {
        self.previous_view = Some(self.view);
        self.view = View::Help;
        self.help_scroll = 0;
    }

    pub fn exit_help(&mut self) {
        if let Some(prev) = self.previous_view {
            self.view = prev;
        } else {
            self.view = View::TorrentList;
        }
        self.previous_view = None;
        self.help_scroll = 0;
    }

    pub fn scroll_help(&mut self, delta: isize) {
        let max_scroll = 30;
        let new_scroll = self.help_scroll as isize + delta;
        self.help_scroll = new_scroll.max(0).min(max_scroll) as usize;
    }

    pub fn update_torrent_tag(&mut self, hash: &str, tag: &str) {
        let remove_tag = if tag == "oc_ok" { "oc_fail" } else { "oc_ok" };
        if let Some(torrent) = self.torrents.iter_mut().find(|t| t.hash == hash) {
            let existing: Vec<&str> = torrent
                .tags
                .split(',')
                .map(|t| t.trim())
                .filter(|t| !t.is_empty() && *t != remove_tag && *t != tag)
                .collect();
            let mut new_tags = existing.join(", ");
            if !new_tags.is_empty() {
                new_tags.push_str(", ");
            }
            new_tags.push_str(tag);
            torrent.tags = new_tags;
        }
    }

    pub fn scroll_logs(&mut self, delta: isize) {
        let max_scroll = self.logs.len().saturating_sub(1);
        let new_scroll = self.log_scroll as isize + delta;
        let bounded = new_scroll.max(0).min(max_scroll as isize);
        self.log_scroll = bounded as usize;
    }

    pub fn scroll_logs_to_end(&mut self) {
        self.log_scroll = self.logs.len().saturating_sub(1);
    }

    pub fn is_log_at_end(&self) -> bool {
        self.logs.is_empty() || self.log_scroll >= self.logs.len().saturating_sub(1)
    }

    fn load_logs_from_file(&mut self) -> Result<Vec<ScriptResult>> {
        use std::io::{BufRead, BufReader};
        let path = Configuration::log_path();
        if !path.exists() {
            return Ok(vec![]);
        }
        let file = fs::File::open(&path)?;
        let reader = BufReader::new(file);
        let existing_keys: std::collections::HashSet<String> = self
            .logs
            .iter()
            .map(|l| format!("{}:{}", l.started_at, l.hash))
            .collect();
        let new_logs: Vec<ScriptResult> = reader
            .lines()
            .filter_map(|line| line.ok())
            .filter_map(|line| serde_json::from_str::<ScriptResult>(&line).ok())
            .filter(|log| !existing_keys.contains(&format!("{}:{}", log.started_at, log.hash)))
            .collect();
        if !new_logs.is_empty() {
            self.logs.extend(new_logs.clone());
            if self.logs.len() > MAX_LOGS {
                self.logs.drain(..self.logs.len() - MAX_LOGS);
            }
            // don't touch log_scroll here — callers decide
        }
        if let Ok(mtime) = fs::metadata(&path)?.modified() {
            self.last_log_mtime = Some(mtime);
        }
        Ok(new_logs)
    }

    pub fn poll_log_file(&mut self) -> Vec<ScriptResult> {
        let path = Configuration::log_path();
        if !path.exists() {
            self.last_log_mtime = None;
            return vec![];
        }
        let current_mtime = match fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(m) => m,
            Err(_) => {
                self.last_log_mtime = None;
                return vec![];
            }
        };
        if let Some(last_mtime) = self.last_log_mtime {
            if current_mtime <= last_mtime {
                return vec![];
            }
        }
        let at_end = self.is_log_at_end();
        match self.load_logs_from_file() {
            Ok(new_entries) => {
                if !new_entries.is_empty() && at_end {
                    self.scroll_logs_to_end();
                }
                new_entries
            }
            Err(e) => {
                self.show_toast_error("Error loading logs from file", &e.to_string());
                vec![]
            }
        }
    }

    pub fn init_logs_from_file(&mut self) {
        if let Err(e) = self.load_logs_from_file() {
            self.show_toast_error("Error initializing logs from file: {}", &e.to_string());
            self.logs.clear();
            self.last_log_mtime = None;
        }
        // Always start at the end
        self.scroll_logs_to_end();
    }

    pub fn show_toast_error(&mut self, title: &str, message: &str) {
        let notification = Notification::new(message.to_string())
            .title(title.to_string())
            .level(Level::Error)
            .anchor(Anchor::TopRight)
            .animation(Animation::Fade)
            .timing(
                Timing::Auto,
                Timing::Fixed(Duration::from_secs(3)),
                Timing::Auto,
            )
            .max_size(SizeConstraint::Absolute(52), SizeConstraint::Absolute(4))
            .auto_dismiss(AutoDismiss::After(Duration::from_secs(1)))
            .build()
            .unwrap();
        _ = self.notifications.add(notification);
    }

    pub fn show_toast_info(&mut self, title: &str, message: &str) {
        let notification = Notification::new(message.to_string())
            .title(title.to_string())
            .level(Level::Info)
            .anchor(Anchor::TopRight)
            .animation(Animation::Fade)
            .timing(
                Timing::Auto,
                Timing::Fixed(Duration::from_secs(3)),
                Timing::Auto,
            )
            .max_size(SizeConstraint::Absolute(52), SizeConstraint::Absolute(4))
            .auto_dismiss(AutoDismiss::After(Duration::from_secs(1)))
            .build()
            .unwrap();
        _ = self.notifications.add(notification);
    }

    pub fn show_toast_success(&mut self, title: &str, message: &str) {
        let notification = Notification::new(message.to_string())
            .title(title.to_string())
            .level(Level::Info)
            .anchor(Anchor::TopRight)
            .animation(Animation::Fade)
            .timing(
                Timing::Auto,
                Timing::Fixed(Duration::from_secs(3)),
                Timing::Auto,
            )
            .max_size(SizeConstraint::Absolute(52), SizeConstraint::Absolute(4))
            .auto_dismiss(AutoDismiss::After(Duration::from_secs(1)))
            .build()
            .unwrap();
        _ = self.notifications.add(notification);
    }

    pub fn show_script_started_toast(&mut self, torrent_name: &str, torrent_hash: &str) {
        let name = if torrent_name.len() > 52 {
            format!("{}…", &torrent_name[..51])
        } else {
            torrent_name.to_string()
        };
        let message = format!("{}\n{}", name, torrent_hash);
        self.show_toast_info("Script started", &message);
    }

    pub fn show_script_toast(&mut self, result: &ScriptResult) {
        let name = if result.name.len() > 52 {
            format!("{}…", &result.name[..51])
        } else {
            result.name.clone()
        };
        let message = &format!("{}\n{}", name, result.hash);
        if result.success {
            self.show_toast_success("Script complete", message);
        } else {
            self.show_toast_error("Script failed", message);
        }
    }

    pub fn tick_notifications(&mut self) {
        self.notifications.tick(Duration::from_millis(60));
    }

    fn save_log_to_file(&self, result: &ScriptResult) -> Result<()> {
        let path = Configuration::log_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(result)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        use std::io::Write;
        writeln!(file, "{}", json)?;
        Ok(())
    }
}
