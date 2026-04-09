mod api;
mod app_state;
mod models;
mod script_runner;
mod ui;

use crate::app_state::{AppMode, AppState, LoginField, ScriptEvent, View};
use crate::models::{build_full_url, parse_host_url, Configuration, Credentials};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io;
use std::sync::mpsc;

const SCRIPT_LONG_ABOUT: &str = "Get or set the on-completion script

Script Variables:

Following qBittorrent, the following variables are available in scripts, both as
argument substitution and environment variables:

  Argument            Environment Variable  Description
  %N                  TORRENT_NAME          Torrent name
  %L                  TORRENT_CATEGORY      Category
  %G                  TORRENT_TAGS          Tags (separated by comma)
  %F                  TORRENT_CONTENT_PATH  Content path (same as root path for
                                            multifile torrent)
  %R                  TORRENT_ROOT_PATH     Root path (first torrent subdirectory
                                            path)
  %D                  TORRENT_SAVE_PATH     Save path
  %C                  TORRENT_NUM_FILES     Number of files
  %Z                  TORRENT_SIZE          Torrent size (bytes)
  %T                  TORRENT_TRACKER       Current tracker
  %I                  TORRENT_INFOHASH      Info hash v1 (or '' if not set)
  %J                  TORRENT_INFOHASH2     Info hash v2 (or '' if not set)
  %K                  TORRENT_ID            Torrent ID (either sha-1 info hash for
                                            v1 torrent or truncated sha-256 info
                                            hash for v2/hybrid torrent)";

#[derive(Parser)]
#[command(name = "qb-och")]
#[command(about = "A CLI tool and TUI for re-running qBittorrent on-completion scripts.")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "config_dir",
        help = "Config directory override (env: QBITTORRENT_OCH_HOME)"
    )]
    config: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Launch the interactive TUI")]
    Tui,
    #[command(about = "Run the on-completion script for a specific torrent")]
    Run {
        #[arg(help = "Torrent ID or infohash")]
        torrent_id: String,
    },
    #[command(about = "Get or set the on-completion script", long_about = SCRIPT_LONG_ABOUT)]
    Script {
        #[arg(help = "Script path to set", index = 1)]
        path: Option<String>,
    },
    #[command(about = "Register/unregister the on-complete handler with qBittorrent")]
    Register {
        #[arg(long, help = "Unregister the handler")]
        unregister: bool,
        #[arg(long, help = "Config directory override for qBittorrent to use")]
        config: Option<String>,
        #[arg(
            long,
            help = "Parent directory for 'qb-och' executable for qBittorrent to use"
        )]
        parent_dir: Option<String>,
        #[arg(long, help = "Skip confirmation prompt")]
        force: bool,
    },
    #[command(about = "CLI login for non-interactive authentication")]
    Login {
        #[arg(
            help = "Hostname to connect to (optional - prints current if not set)",
            index = 1
        )]
        host: Option<String>,
        #[arg(long, help = "Username (env: QBITTORRENT_OCH_USERNAME)")]
        username: Option<String>,
        #[arg(
            long,
            help = "Password (env: QBITTORRENT_OCH_PASSWORD)",
            conflicts_with = "password_file"
        )]
        password: Option<String>,
        #[arg(
            long,
            help = "Read password from file (env: QBITTORRENT_OCH_PASSWORD_FILE)",
            conflicts_with = "password"
        )]
        password_file: Option<String>,
        #[arg(
            long,
            help = "Test if the connection is usable",
            default_missing_value = "true"
        )]
        test_connection: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    // Set config directory from --config flag if provided
    if let Some(config_dir) = &cli.config {
        std::env::set_var("QBITTORRENT_OCH_HOME", config_dir);
    }
    match cli.command {
        Commands::Tui => run_tui_main(),
        Commands::Run { torrent_id } => run_run_command(&torrent_id),
        Commands::Script { path } => run_script_command(path),
        Commands::Register {
            unregister,
            config,
            parent_dir,
            force,
        } => run_register_command(unregister, parent_dir, config, force),
        Commands::Login {
            host,
            username,
            password,
            password_file,
            test_connection,
        } => run_login_command(host, username, password, password_file, test_connection),
    }
}

fn run_tui_main() -> Result<()> {
    use crossterm::execute;
    use crossterm::terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    };
    let mut state = AppState::default();
    if let Some(configuration) = Configuration::load() {
        state = AppState::with_configuration(&configuration);
        let rt = tokio::runtime::Runtime::new()?;
        if let Err(e) = rt.block_on(state.test_connection()) {
            eprintln!("Saved credentials failed: {}", e);
            state.enter_login_mode();
        }
    } else {
        state.enter_login_mode();
    }
    if state.mode != AppMode::Login {
        let rt = tokio::runtime::Runtime::new()?;
        if let Err(e) = rt.block_on(state.refresh()) {
            eprintln!("Warning: Failed to load torrents: {}", e);
            state.enter_login_mode();
        }
    }
    // Set up channel for background script results
    let (script_tx, script_rx) = mpsc::channel();
    state.script_result_tx = Some(script_tx);
    enable_raw_mode()?;
    execute!(io::stderr(), EnterAlternateScreen)?;
    let result = run_tui(&mut state, script_rx);
    execute!(io::stderr(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    result
}

fn run_tui(state: &mut AppState, script_rx: mpsc::Receiver<ScriptEvent>) -> Result<()> {
    let backend = ratatui::backend::CrosstermBackend::new(io::stderr());
    let mut terminal = ratatui::Terminal::new(backend)?;
    terminal.clear()?;
    let mut last_sync = std::time::Instant::now();
    let sync_interval = std::time::Duration::from_secs(2);
    let mut last_log_poll = std::time::Instant::now();
    let log_poll_interval = std::time::Duration::from_secs(5);
    let rt = tokio::runtime::Runtime::new()?;
    state.init_logs_from_file();
    loop {
        let size = terminal.size()?;
        terminal.draw(|f| {
            ui::render_app(f, state);
            state.notifications.render(f, f.area());
        })?;
        state.tick_notifications();
        // Drain background script results, update logs, and show toasts.
        // Adding to state.logs here ensures poll_log_file sees them in existing_keys
        // and won't fire a second toast for the same entry.
        while let Ok(event) = script_rx.try_recv() {
            match event {
                ScriptEvent::Started {
                    torrent_name,
                    torrent_hash,
                } => {
                    state.show_script_started_toast(&torrent_name, &torrent_hash);
                }
                ScriptEvent::Completed(result) => {
                    state.show_script_toast(&result);
                    let at_end = state.is_log_at_end();
                    state.logs.push(result);
                    if state.logs.len() > 1000 {
                        state.logs.drain(..state.logs.len() - 1000);
                    }
                    if at_end {
                        state.scroll_logs_to_end();
                    }
                }
            }
        }
        if state.pending_login.is_some() {
            if let Err(e) = rt.block_on(state.process_login()) {
                state.show_toast_error("Login Failed", &e.to_string());
            }
        } else if state.mode == AppMode::Normal && last_sync.elapsed() >= sync_interval {
            if let Err(_e) = rt.block_on(state.sync()) {
                // On sync failure fall back to a full refresh and reset rid
                state.sync_rid = 0;
                if let Err(e2) = rt.block_on(state.refresh()) {
                    state.show_toast_error("Refresh Failed", &e2.to_string());
                }
            }
            last_sync = std::time::Instant::now();
        }
        // Lazy-load torrent files when Files pane is active
        if state.mode == AppMode::Normal {
            if let Some(hash) = state.needs_file_load() {
                if let Err(e) = rt.block_on(state.load_torrent_files(hash)) {
                    state.show_toast_error("File Load Failed", &e.to_string());
                    state.files_loading = None;
                }
            }
        }
        if last_log_poll.elapsed() >= log_poll_interval {
            let new_entries = state.poll_log_file();
            for result in new_entries {
                state.show_script_toast(&result);
            }
            last_log_poll = std::time::Instant::now();
        }
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            let event = crossterm::event::read()?;
            if let Some(handled) = handle_event(state, &event, size.into()) {
                if !handled {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn handle_event(
    state: &mut AppState,
    event: &crossterm::event::Event,
    _area: ratatui::layout::Rect,
) -> Option<bool> {
    if let crossterm::event::Event::Key(key) = event {
        if state.mode == AppMode::Login {
            return handle_login_event(state, &key);
        }
        match key.code {
            crossterm::event::KeyCode::Char('h') => {
                state.enter_help();
            }
            crossterm::event::KeyCode::Up => {
                let shift = key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT);
                if state.view == View::Help {
                    state.scroll_help(-1);
                } else if state.view == View::LogView {
                    state.scroll_logs(-1);
                } else if shift && matches!(state.view, View::VerticalSplit | View::HorizontalSplit)
                {
                    state.scroll_logs(-1);
                } else {
                    state.move_selection(-1);
                }
            }
            crossterm::event::KeyCode::Down => {
                let shift = key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT);
                if state.view == View::Help {
                    state.scroll_help(1);
                } else if state.view == View::LogView {
                    state.scroll_logs(1);
                } else if shift && matches!(state.view, View::VerticalSplit | View::HorizontalSplit)
                {
                    state.scroll_logs(1);
                } else {
                    state.move_selection(1);
                }
            }
            crossterm::event::KeyCode::Char('s') => {
                if state.view != View::Help {
                    state.cycle_sort_field(1);
                }
            }
            crossterm::event::KeyCode::Char('S') => {
                if state.view != View::Help {
                    state.toggle_sort_direction();
                }
            }
            crossterm::event::KeyCode::Tab => {
                if state.view != View::Help {
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::SHIFT)
                    {
                        state.cycle_filter_backward();
                    } else {
                        state.cycle_filter_forward();
                    }
                }
            }
            crossterm::event::KeyCode::Char('t') => {
                if matches!(
                    state.view,
                    View::TorrentList | View::VerticalSplit | View::HorizontalSplit
                ) {
                    state.show_details = !state.show_details;
                }
            }
            crossterm::event::KeyCode::Left => {
                if state.show_details
                    && matches!(
                        state.view,
                        View::TorrentList | View::VerticalSplit | View::HorizontalSplit
                    )
                {
                    state.detail_pane = state.detail_pane.prev();
                }
            }
            crossterm::event::KeyCode::Right => {
                if state.show_details
                    && matches!(
                        state.view,
                        View::TorrentList | View::VerticalSplit | View::HorizontalSplit
                    )
                {
                    state.detail_pane = state.detail_pane.next();
                }
            }
            crossterm::event::KeyCode::Char('v') => {
                if state.view != View::Help {
                    state.cycle_view_forward();
                }
            }
            crossterm::event::KeyCode::Char('V') => {
                if state.view != View::Help {
                    state.cycle_view_backward();
                }
            }
            crossterm::event::KeyCode::Char('r') => {
                let selected = state.selected_torrent().cloned();
                if let Some(torrent) = selected {
                    let configuration = Configuration::load();
                    let tx = state.script_result_tx.clone();
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let mut temp_state = match configuration {
                            Some(ref c) => AppState::with_configuration(c),
                            None => AppState::default(),
                        };
                        temp_state.torrents.push(torrent);
                        temp_state.script_result_tx = tx;
                        if let Err(e) = rt.block_on(temp_state.run_script_on_selected()) {
                            eprintln!("Script execution error: {}", e);
                        }
                    });
                }
            }
            crossterm::event::KeyCode::Char('R') => {
                let torrents = state.torrents.clone();
                let configuration = Configuration::load();
                let tx = state.script_result_tx.clone();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let mut temp_state = match configuration {
                        Some(ref c) => AppState::with_configuration(c),
                        None => AppState::default(),
                    };
                    temp_state.torrents = torrents;
                    temp_state.script_result_tx = tx;
                    if let Err(e) = rt.block_on(temp_state.run_script_on_all_completed()) {
                        eprintln!("Script execution error: {}", e);
                    }
                });
            }
            crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Esc => {
                if state.view == View::Help {
                    state.exit_help();
                } else {
                    return Some(false);
                }
            }
            crossterm::event::KeyCode::Home => {
                let shift = key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT);
                if state.view == View::Help {
                    state.help_scroll = 0;
                } else if state.view == View::LogView
                    || (shift && matches!(state.view, View::VerticalSplit | View::HorizontalSplit))
                {
                    state.log_scroll = 0;
                } else {
                    let count = state.filtered_torrents().len();
                    state.jump_to(0, count);
                }
            }
            crossterm::event::KeyCode::End => {
                let shift = key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT);
                if state.view == View::Help {
                    state.help_scroll = 1000;
                } else if state.view == View::LogView
                    || (shift && matches!(state.view, View::VerticalSplit | View::HorizontalSplit))
                {
                    state.log_scroll = state.logs.len().saturating_sub(1);
                } else {
                    let count = state.filtered_torrents().len();
                    state.jump_to(count.saturating_sub(1), count);
                }
            }
            crossterm::event::KeyCode::PageUp => {
                let shift = key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT);
                if state.view == View::Help {
                    state.scroll_help(-10);
                } else if state.view == View::LogView
                    || (shift && matches!(state.view, View::VerticalSplit | View::HorizontalSplit))
                {
                    state.scroll_logs(-10);
                } else {
                    let count = state.filtered_torrents().len();
                    state.jump_to(state.selected_index.saturating_sub(10), count);
                }
            }
            crossterm::event::KeyCode::PageDown => {
                let shift = key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT);
                if state.view == View::Help {
                    state.scroll_help(10);
                } else if state.view == View::LogView
                    || (shift && matches!(state.view, View::VerticalSplit | View::HorizontalSplit))
                {
                    state.scroll_logs(10);
                } else {
                    let count = state.filtered_torrents().len();
                    state.jump_to(state.selected_index + 10, count);
                }
            }
            _ => {}
        }
    }
    Some(true)
}

fn handle_login_event(state: &mut AppState, key: &crossterm::event::KeyEvent) -> Option<bool> {
    use crossterm::event::KeyModifiers;
    match key.code {
        crossterm::event::KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                state.retreat_login_field();
            } else {
                state.advance_login_field();
            }
        }
        crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Left => {
            state.retreat_login_field();
        }
        crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Right => {
            state.advance_login_field();
        }
        crossterm::event::KeyCode::Enter => {
            state.queue_login();
        }
        crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('q') => {
            return Some(false);
        }
        crossterm::event::KeyCode::Backspace | crossterm::event::KeyCode::Delete => {
            match state.login_field {
                LoginField::Host => {
                    state.login_host.pop();
                }
                LoginField::Username => {
                    state.login_username.pop();
                }
                LoginField::Password => {
                    state.login_password.pop();
                }
            }
        }
        crossterm::event::KeyCode::Char(c) => {
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT)
            {
                match state.login_field {
                    LoginField::Host => {
                        state.login_host.push(c);
                    }
                    LoginField::Username => {
                        state.login_username.push(c);
                    }
                    LoginField::Password => {
                        state.login_password.push(c);
                    }
                }
            }
        }
        _ => {}
    }
    Some(true)
}

fn run_run_command(torrent_id: &str) -> anyhow::Result<()> {
    let torrent_id = torrent_id.trim_matches('"');
    let rt = tokio::runtime::Runtime::new()?;
    let configuration = Configuration::load()
        .ok_or_else(|| anyhow::anyhow!("No saved credentials. Run 'qb-och login' first."))?;
    let state = AppState::with_configuration(&configuration);
    rt.block_on(async { state.api.test_connection().await })
        .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;
    let torrents: Vec<models::Torrent> = rt
        .block_on(async { state.api.get_torrents().await })
        .map_err(|e| anyhow::anyhow!("Failed to fetch torrents: {}", e))?;
    let torrent = torrents
        .iter()
        .find(|t| t.hash.eq(torrent_id))
        .ok_or_else(|| anyhow::anyhow!("Torrent not found: {}", torrent_id))?
        .clone();
    let script = state.script.unwrap_or_default();
    if script.is_empty() {
        anyhow::bail!("No auto-run script configured. Run 'qb-och script' first.");
    }
    // Run the script
    let runner = script_runner::ScriptRunner::new(script);
    let result = rt
        .block_on(runner.run(&torrent, &state.api))
        .map_err(|e| anyhow::anyhow!("Script execution failed: {}", e))?;
    // Add tag on the torrent
    let tag_to_add = if result.success { "oc_ok" } else { "oc_fail" };
    if !torrent.tags.split(",").any(|t| t.trim() == tag_to_add) {
        _ = rt.block_on(async {
            state
                .api
                .add_tags(&[torrent.hash.clone()], &[tag_to_add.to_string()])
                .await
        });
    }
    // Remove tag on the torrent
    let tag_to_remove = if result.success { "oc_fail" } else { "oc_ok" };
    if torrent.tags.split(",").any(|t| t.trim() == tag_to_remove) {
        _ = rt.block_on(async {
            state
                .api
                .remove_tags(&[torrent.hash.clone()], &[tag_to_remove.to_string()])
                .await
        });
    }
    // Write to log file so TUI can pick it up via poll_log_file
    let log_path = models::Configuration::log_path();
    if let Some(parent) = log_path.parent() {
        _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(&result) {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            _ = writeln!(file, "{}", json);
        }
    }
    // Print result and exit
    if !result.stdout.is_empty() {
        print!("{}", result.stdout);
    }
    if !result.stderr.is_empty() {
        eprint!("{}", result.stderr);
    }
    if !result.success {
        std::process::exit(result.exit_code.unwrap());
    }
    Ok(())
}

fn run_script_command(path: Option<String>) -> anyhow::Result<()> {
    let mut configuration = Configuration::load()
        .or_else(|| Some(Configuration::new()))
        .unwrap();
    if let Some(script_path) = path {
        configuration.script = Some(script_path.clone());
        configuration.save()?;
        println!("Script path set to: {}", script_path);
    } else {
        if let Some(script) = configuration.script {
            println!("{}", script);
            return Ok(());
        }
        eprintln!("No script configured. Use 'qb-och script /path/to/script' to set one.");
        std::process::exit(1);
    }
    Ok(())
}

fn run_register_command(
    unregister: bool,
    parent_dir: Option<String>,
    config_dir: Option<String>,
    force: bool,
) -> anyhow::Result<()> {
    let credentials = Configuration::load()
        .ok_or_else(|| anyhow::anyhow!("No saved credentials. Run 'qb-och login' first."))?;
    let state = AppState::with_configuration(&credentials);
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { state.api.test_connection().await })
        .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;
    let preferences: models::Preferences = rt
        .block_on(async { state.api.get_preferences().await })
        .map_err(|e| anyhow::anyhow!("Failed to get preferences: {}", e))?;
    if unregister {
        let update = models::PreferencesForUpdate {
            script_enabled: Some(false),
            script: Some(String::new()),
        };
        rt.block_on(async { state.api.set_preferences(&update).await })
            .map_err(|e| anyhow::anyhow!("Failed to unregister: {}", e))?;
        println!("Unregistered successfully.");
        return Ok(());
    }
    let script_path = if let Some(dir) = parent_dir {
        let dir = std::path::PathBuf::from(dir);
        if !dir.is_dir() {
            anyhow::bail!("Specified directory does not exist: {}", dir.display());
        }
        std::path::PathBuf::from(dir.join("qb-och"))
    } else {
        let log_path = preferences.log_dir.ok_or_else(|| {
            anyhow::anyhow!(
                "Could not determine qBittorrent config directory. Provide --config-dir."
            )
        })?;
        match std::path::PathBuf::from(&log_path).parent() {
            Some(parent_dir) => {
                let is_qbit_dir = parent_dir.is_dir()
                    && parent_dir
                        .file_name()
                        .map(|n| n.to_string_lossy() == "qBittorrent")
                        .unwrap_or(false);
                if !is_qbit_dir {
                    anyhow::bail!("Auto-detection failed: save_path does not appear to be in a 'qBittorrent' directory. Provide --config-dir manually.");
                }
                std::path::PathBuf::from(parent_dir.join("qb-och"))
            }
            None => {
                anyhow::bail!(
                    "Could not determine qBittorrent config directory. Provide --config-dir."
                )
            }
        }
    };
    let current_script = &preferences.script;
    let target_script = match config_dir {
        Some(c) => format!("{} --config {} run \"%K\"", script_path.display(), c),
        None => format!("{} run \"%K\"", script_path.display()),
    };
    if !preferences.script_enabled && current_script.contains("qb-och run \"%K\"") {
        let update = models::PreferencesForUpdate {
            script_enabled: Some(true),
            script: None,
        };
        rt.block_on(async { state.api.set_preferences(&update).await })
            .map_err(|e| anyhow::anyhow!("Failed to enable: {}", e))?;
        println!("Enabled (was already set): {}", current_script);
        return Ok(());
    }
    if preferences.script_enabled && current_script.contains("qb-och run \"%K\"") {
        if force || current_script == &target_script {
            // Continue to allow force override or exact match
        } else if current_script.ends_with("run \"%K\"") {
            println!("Already registered (partial match): {}", current_script);
            return Ok(());
        } else {
            println!("Already registered with: {}", current_script);
            return Ok(());
        }
    }
    if !current_script.is_empty() && !force {
        eprintln!("Warning: different script already set: {}", current_script);
        eprintln!("Use --force to override, or run with --unregister to clear it first.");
        std::process::exit(1);
    }
    let update = models::PreferencesForUpdate {
        script_enabled: Some(true),
        script: Some(target_script.clone()),
    };
    rt.block_on(async { state.api.set_preferences(&update).await })
        .map_err(|e| anyhow::anyhow!("Failed to register: {}", e))?;
    println!("Registered successfully: {}", target_script);
    Ok(())
}

fn run_login_command(
    host_arg: Option<String>,
    username: Option<String>,
    password: Option<String>,
    password_file: Option<String>,
    test_connection: bool,
) -> anyhow::Result<()> {
    if let Some(host_input) = host_arg {
        let parsed =
            parse_host_url(&host_input).map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;
        let url_username = if parsed.username.is_empty() {
            None
        } else {
            Some(parsed.username)
        };
        let url_password = if parsed.password.is_empty() {
            None
        } else {
            Some(parsed.password)
        };
        let username = username
            .or_else(|| std::env::var("QBITTORRENT_OCH_USERNAME").ok())
            .or(url_username)
            .unwrap_or_default();
        let password = if let Some(path) =
            password_file.or_else(|| std::env::var("QBITTORRENT_OCH_PASSWORD_FILE").ok())
        {
            let contents = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("Failed to read password file '{}': {}", path, e))?;
            contents
                .trim_end_matches('\n')
                .trim_end_matches('\r')
                .to_string()
        } else {
            password
                .or_else(|| std::env::var("QBITTORRENT_OCH_PASSWORD").ok())
                .or(url_password)
                .unwrap_or_default()
        };
        if !username.is_empty() && password.is_empty() {
            anyhow::bail!("Password required when username is set. Use --password, --password-file, set QBITTORRENT_OCH_PASSWORD, or include it in the URL");
        }
        let normalized_url =
            try_connect_and_normalize(&host_input, username.as_str(), password.as_str())
                .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;
        let credentials = Credentials::new(normalized_url.clone(), username, password);
        let api = api::QBitApi::with_credentials(&credentials.clone());
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async { api.login().await })
            .map_err(|e| anyhow::anyhow!("Login failed: {}", e))?;
        let mut configuration = Configuration::load()
            .or_else(|| Some(Configuration::new()))
            .unwrap();
        configuration.credentials = credentials;
        configuration.save()?;
        println!("Login successful! Credentials saved.");
    } else {
        if let Some(configuration) = Configuration::load() {
            // Default to not testing when just displaying existing connection
            let url = configuration.display_url();
            if !test_connection {
                println!("{}", url);
            } else {
                let state = AppState::with_configuration(&configuration);
                let rt = tokio::runtime::Runtime::new()?;
                match rt.block_on(state.api.test_connection()) {
                    Ok(version) => {
                        println!("{} (Connection OK - qBittorrent {})", url, version);
                    }
                    Err(e) => {
                        eprintln!("{} (Connection FAILED: {})", url, e);
                        std::process::exit(1);
                    }
                }
            }
        } else {
            eprintln!("No credentials configured. Use 'qb-och login <hostname>' to set up.");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn try_connect_and_normalize(host: &str, username: &str, password: &str) -> Result<String, String> {
    let parsed = parse_host_url(host).map_err(|e| e.to_string())?;
    let protocol = if parsed.protocol.is_empty() {
        "http://".to_string()
    } else {
        parsed.protocol.clone()
    };
    let host_part = parsed.host.clone();
    let port = parsed.port;
    let pathname = parsed.pathname.clone();
    let ports_to_try = if let Some(p) = port {
        vec![p]
    } else {
        vec![8080, 443, 80]
    };
    let user = if username.is_empty() {
        None
    } else {
        Some(username)
    };
    for p in ports_to_try {
        let url = build_full_url(&protocol, &host_part, Some(p), &pathname, None);
        let credentials = Credentials::new(url.clone(), username.to_string(), password.to_string());
        let api = api::QBitApi::with_credentials(&credentials);
        let rt = tokio::runtime::Runtime::new().unwrap();
        eprintln!(
            "Trying {}...",
            build_full_url(&protocol, &host_part, Some(p), &pathname, user)
        );
        match rt.block_on(api.test_connection()) {
            Ok(_) => {
                return Ok(url);
            }
            Err(e) => {
                eprintln!("  Failed: {}", e);
            }
        }
    }
    Err("Could not connect to any port".to_string())
}
