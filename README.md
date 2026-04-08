# qBittorrent On-Complete Handler (qb-och)

A CLI tool and TUI for re-running qBittorrent on-completion scripts.

---

This project came about due to the lack of an easy way to re-run the on-completion script for completed torrents in qBittorrent. If qBittorrent fixes this upstream, this project may no longer be needed.

## Features

- **Auto-Registration**: Can effortlessly register `qb-och` as qBittorrent's on-complete handler
- **Script Execution**: Re-run the on-completion script for completed torrents
- **State Tracking**: Tags torrents with execution status (`oc_ok`, `oc_fail`)
- **CLI Operation**: Run the on-completion script from command line for automation
- **TUI Application**: Interactive terminal UI for managing completed torrents

## Installation

### Download Pre-built Binary

Download the latest release for your platform from the [GitHub Releases](https://github.com/revam/qBittorrent-och/releases):

#### Linux (x86_64)
```bash
curl -L -o qb-och.zip https://github.com/revam/qBittorrent-och/releases/latest/download/qb-och-linux-x64.zip
unzip qb-och.zip && rm qb-och.zip && chmod +x qb-och
```

#### Linux (ARM64)
```bash
curl -L -o qb-och.zip https://github.com/revam/qBittorrent-och/releases/latest/download/qb-och-linux-arm64.zip
unzip qb-och.zip && rm qb-och.zip && chmod +x qb-och
```

#### macOS (Apple Silicon)
```bash
curl -L -o qb-och.zip https://github.com/revam/qBittorrent-och/releases/latest/download/qb-och-macos-arm64.zip
unzip qb-och.zip && rm qb-och.zip && chmod +x qb-och
```

#### macOS (Intel)
```bash
curl -L -o qb-och.zip https://github.com/revam/qBittorrent-och/releases/latest/download/qb-och-macos-x64.zip
unzip qb-och.zip && rm qb-och.zip && chmod +x qb-och
```

#### Windows
```bash
curl -L -o qb-och.zip https://github.com/revam/qBittorrent-och/releases/latest/download/qb-och-windows-x64.zip
Expand-Archive -Path qb-och.zip -DestinationPath . && rm qb-och.zip
```

### Build from Source

```bash
# Clone the repository
git clone https://github.com/revam/qBittorrent-och.git

# Enter the directory
cd qBittorrent-och

# Build the application
cargo build --release

# (Optional) Install the application
cargo install --path .
```

## Quick Start

> **Note**: For maximum compatibility when running `qb-och` outside of qBittorrent, it is recommended to run it with the same user or same user and group IDs as qBittorrent. This ensures the script can access torrent data and files the same way qBittorrent does.

```
A CLI tool and TUI for re-running qBittorrent on-completion scripts.

Usage: qb-och [OPTIONS] <COMMAND>

Commands:
  tui       Launch the interactive TUI
  run       Run the on-completion script for a specific torrent
  script    Get or set the on-completion script
  register  Register/unregister the on-complete handler with qBittorrent
  login     CLI login for non-interactive authentication
  help      Print this message or the help of the given subcommand(s)

Options:
      --config <config_dir>  Config directory override (env: QBITTORRENT_OCH_HOME)
  -h, --help                 Print help
  -V, --version              Print version
```

Run `qb-och <command> -h` to see options for each subcommand.

### Basic Usage

- Run `qb-och login <hostname>` to login to a qBittorrent instance
- Run `qb-och script <script>` to set the on-completion script
- Run `qb-och register` to configure qBittorrent to use the on-completion script
- When needed, run `qb-och tui` to launch the TUI to manage completed torrents

### Examples

```bash
# Login to a qBittorrent instance
qb-och login localhost
qb-och login admin@localhost
qb-och login http://localhost:8080
qb-och login https://localhost.example.org

# Set the on-completion script
qb-och script "/path/to/your-script.sh %N %L %G %F"

# Register qb-och as the on-complete handler
qb-och register

# Unregister (remove the handler)
qb-och register --unregister

# Force registration (skip confirmation if already registered)
qb-och register --force

# Run script for a specific torrent by ID
qb-och run dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c

# Launch the interactive TUI to manage completed torrents
qb-och tui
```


## Script Variables

Following qBittorrent, the following variables are available in scripts, both as argument substitution and environment variables:

| Argument | Environment Variable | Description |
|----------|---------------------|-------------|
| `%N` | `TORRENT_NAME` | Torrent name |
| `%L` | `TORRENT_CATEGORY` | Category |
| `%G` | `TORRENT_TAGS` | Tags (separated by comma) |
| `%F` | `TORRENT_CONTENT_PATH` | Content path (same as root path for multifile torrent) |
| `%R` | `TORRENT_ROOT_PATH` | Root path (first torrent subdirectory path) |
| `%D` | `TORRENT_SAVE_PATH` | Save path |
| `%C` | `TORRENT_NUM_FILES` | Number of files |
| `%Z` | `TORRENT_SIZE` | Torrent size (bytes) |
| `%T` | `TORRENT_TRACKER` | Current tracker |
| `%I` | `TORRENT_INFOHASH_V1` | Info hash v1 (or '-' if unavailable) |
| `%J` | `TORRENT_INFOHASH_V2` | Info hash v2 (or '-' if unavailable) |
| `%K` | `TORRENT_ID` | Torrent ID (either sha-1 info hash for v1 torrent or truncated sha-256 info hash for v2/hybrid torrent) |

## Environment Variables

The following environment variables can be used:

| Environment Variable | Command | Description |
|----------------------|---------|-------------|
| `QBITTORRENT_OCH_HOME` | all | Override config directory (creates if doesn't exist) |
| `QBITTORRENT_OCH_USERNAME` | `login` | Username for login |
| `QBITTORRENT_OCH_PASSWORD` | `login` | Password for login |
| `QBITTORRENT_OCH_PASSWORD_FILE` | `login` | Path to file containing password for login |

## Configuration

Configuration is stored in `${XDG_CONFIG_HOME:-$HOME/.config}/qb-och/config.toml` by default.

You can override the config directory using:
- CLI flag: `--config /path/to/config`
- Environment variable: `QBITTORRENT_OCH_HOME=/path/to/config`

The tool will create the directory if it doesn't exist (unless it points to a file).

### Script

The script path can be set via `qb-och script /path/to/script.sh` or directly in the config.

### Execution Logs

Script execution logs are stored in `${XDG_DATA_HOME:-~/.local/share}/qb-och/execution.log`:

```json
{"hash":"dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c","name":"Example","script":"echo \"%K: %N - %C\"","stdout":"\"dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c: Big Buck Bunny - 3\"","stderr":"","success":true,"exit_code":0,"started_at":"2024-01-01T12:00:00.100Z","completed_at":"2024-01-01T12:00:01.111Z"}
```

Each line is a separate JSON object, making it easy to parse with tools like `jq`.

## TUI Usage
Press `h` in the TUI to open the in-terminal help page.

### State Indicators

- **Blue `?`** - Script not yet ran
- **Green `✓`** - Script completed successfully
- **Red `!`** - Script execution failed

### Keyboard Shortcuts

#### Global

| Key | Action |
|-----|--------|
| `↑/↓` | Move up/down |
| `PgUp` / `PgDn` | Page up/down |
| `Home` / `End` | Jump to first/last |
| `h` | Open help page |
| `q` / `Esc` | Quit / Go back |

#### Torrent List

| Key | Action |
|-----|--------|
| `s` | Change sort field (Name/Completed/Added) |
| `S` | Toggle ascending/descending sort |
| `Tab` / `Shift+Tab` | Cycle filter (All/Not Ran/Ok/Fail) |
| `v` / `V` | Cycle view mode (Torrent/Log/Vertical Split/Horizontal Split) |
| `r` | Run on-complete script on selected torrent |
| `R` | Run on-complete script on all completed torrents |
| `t` | Toggle torrent details pane |
| `←/→` | Switch details sub-pane (Info/Paths/Transfer/Files) |
| `Enter` | Open torrent details |

#### Split Views

| Key | Action |
|-----|--------|
| `Shift+↑/↓` | Move log up/down |
| `Shift+PgUp/PgDn` | Log page up/down |
| `Shift+Home/End` | Jump to first/last log entry |

### View Modes

Press `v` or `V` to cycle through the available view modes:
- **Torrent List** - Full torrent list view
- **Log View** - Full script execution log view
- **Vertical Split** - Torrent list on left, log on right
- **Horizontal Split** - Torrent list on top, log on bottom

### Filter by State

Press `Tab` or `Shift+Tab` to cycle through script state filters:
- **All** - Show all completed torrents
- **Not ran** - Show torrents where script hasn't run
- **Ok** - Show torrents where script succeeded
- **Fail** - Show torrents where script failed

The current filter is displayed in the header.

## Roadmap

- Clean up the view modes, view focus and keyboard input handling to be less janky, and so we can move focus around the different views more easily.

- Improve the scrolling capabilities of the different views, maybe with a "fake" scroll bar.

- Improve the log handling, make it better virtualize the list, and maybe filter the view to the selected torrent in the split view modes.

- Add an action menu in the torrent list view, to select different actions to make, and add new actions such as deleting a torrent. Would be implemented after the view focus is improved, if at all.

- Add a `list` command, with filtering capabilities such as `--state`, `--sort`, `--limit`, `--offset`, etc.

- Add a `log` command, with filtering capabilities such as `--hash`, `--sort`, `--limit`, `--offset`, etc.

- Add a `delete` command, to delete torrents from the list, log, and qBittorrent.

## Troubleshooting

### Connection Issues

- Ensure qBittorrent WebUI is enabled: **Tools** > **Options** > **Web UI**
- Check that qBittorrent is running
- Verify credentials with `qb-och login`

### Authentication

- Run `qb-och login --test-connection` to test the connection.

### Script Not Executing

- Verify the script is set with `qb-och script`.
- Check script permissions. You can make the script executable with `chmod +x your-script.sh` on Unix-like systems.

## License

[MIT](./LICENSE)
