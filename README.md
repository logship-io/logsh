# logsh - Logship CLI

A command-line interface for interacting with [Logship](https://logship.io). Query logs, upload data, manage connections and accounts — designed for both human users and CI/automation pipelines.

## Quick Start

```bash
# Add a context
logsh ctx add https://my.logship.server

# Check status
logsh whoami

# Query logs
logsh query -q 'MyTable | take 10'

# Upload CSV data
logsh upload my_table data.csv
```

## Installation

### Pre-built Binaries

Download from [Releases](https://github.com/logship-io/logsh/releases) for your platform:

| Platform | Architecture |
|----------|-------------|
| Linux | x86_64, aarch64, armv7 (RPi), arm, musl (static) |
| macOS | x86_64 (Intel), aarch64 (Apple Silicon) |
| Windows | x86_64, aarch64 |
| FreeBSD | x86_64 |

### Container

```bash
docker run --rm ghcr.io/logship-io/logsh:latest --help
```

Multi-arch images available: `linux/amd64`, `linux/arm64`, `linux/arm/v7`

### Self-Update

```bash
logsh version --update       # Latest stable
logsh version --update-prerelease  # Latest dev build
```

## Usage

### Contexts

```bash
# Add with username (password will be prompted securely)
logsh ctx add https://logship.example.com --name prod

# Add with Personal Access Token (CI/automation)
logsh ctx add https://logship.example.com --pat --token $LOGSH_PAT_TOKEN

# Add with OAuth device flow
logsh ctx add https://logship.example.com --sso

# List contexts
logsh ctx ls
logsh ctx ls -o json   # Machine-readable

# Switch context
logsh ctx use prod

# Show current context
logsh ctx current

# Re-authenticate
logsh ctx login

# Remove a context
logsh ctx rm old-ctx
```

### Queries

```bash
# Inline query
logsh query -q 'MyTable | take 10'

# Read query from a file
logsh query -f query.kql

# From stdin (pipe-friendly)
echo 'MyTable | count' | logsh query

# Output formats: table (default), json, json-pretty, csv, markdown
logsh query -q 'MyTable | take 5' -o json
logsh query -q 'MyTable | take 5' -o csv > output.csv

# Custom timeout
logsh query -q 'BigTable | summarize count() by bin(timestamp, 1h)' -t 5m
```

### Data Upload

```bash
# Upload CSV
logsh upload my_schema data.csv

# Upload TSV with progress
logsh upload my_schema data.tsv --progress
```

### Schema Inspection

```bash
# List all tables
logsh schema ls

# Describe columns in a table
logsh schema describe MyTable
```

### Accounts

```bash
logsh acc ls
logsh acc ls --include-all
logsh acc use <account-name>
logsh acc current
logsh acc delete <account-id>
```

### Shell Completions

```bash
# Bash
logsh completions bash > ~/.local/share/bash-completion/completions/logsh

# Zsh
logsh completions zsh > ~/.zfunc/_logsh

# Fish
logsh completions fish > ~/.config/fish/completions/logsh.fish

# PowerShell
logsh completions powershell >> $PROFILE
```

### Global Flags

| Flag | Description |
|------|-------------|
| `-v` / `-vvvv` | Increase verbosity (error → warn → info → debug → trace) |
| `--no-color` | Disable colored output |
| `--context <name>` | Use a specific named context |
| `--account <name>` | Override the account for a command (by name) |
| `--config-path <path>` | Override config file location |
| `-o <format>` | Output format: `table`, `json`, `json-pretty`, `csv`, `markdown` |
| `--quiet` | Suppress non-essential output |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `LOGSH_CONFIG_PATH` | Override config file path |
| `LOGSH_PAT_TOKEN` | Personal Access Token for `context add pat` |
| `LOGSH_UPDATE_REPOSITORY` | Custom GitHub repo for self-update (format: `owner/repo`) |
| `NO_COLOR` | Disable color output ([no-color.org](https://no-color.org)) |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Authentication failure |
| 3 | No connection configured |

## Container Usage

### With Configuration

```bash
# Mount config directory
docker run --rm \
  -v ~/.logsh:/config:Z \
  ghcr.io/logship-io/logsh:latest query -q 'MyTable | take 10'

# Mount specific config file
docker run --rm \
  -v ~/.logsh/config.json:/config/config.json:Z \
  ghcr.io/logship-io/logsh:latest whoami
```

### Docker Compose

```yaml
services:
  logsh:
    image: ghcr.io/logship-io/logsh:latest
    volumes:
      - ~/.logsh:/config:ro
    command: ["query", "-q", "MyTable | take 5", "-o", "json"]
```

## logsht — Terminal UI

`logsht` is an interactive terminal UI (TUI) for querying and exploring your Logship data, built with [Ratatui](https://ratatui.rs).

```bash
logsht
```

### Layout

logsht has three main panes:

- **Schemas** — browse and filter tables on the connected server
- **Editor** — write and edit queries with full text editing
- **Results** — navigate query results with keyboard controls

### Keybindings

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle focus: Schemas → Editor → Results |
| `Alt+Enter` / `Ctrl+R` | Execute query |
| `Ctrl+K` | Open context switcher |
| `Ctrl+S` | Open saved queries |
| `Alt+Up` / `Alt+Down` | Navigate query history |
| `:` | Open command bar |
| `?` / `Ctrl+H` | Show help |
| `Ctrl+Q` / `Ctrl+C` | Quit |

#### Schemas Pane

| Key | Action |
|-----|--------|
| `j` / `k` / `↑` / `↓` | Navigate tables |
| `Enter` | Select table → editor |
| `r` | Refresh schemas |
| Type | Filter tables by name |

#### Editor Pane

| Key | Action |
|-----|--------|
| Type | Enter query text (full tui-textarea) |
| `Esc` | Move focus to schemas |

#### Results Pane

| Key | Action |
|-----|--------|
| `j` / `k` / `↑` / `↓` | Move cursor up/down rows |
| `h` / `l` / `←` / `→` | Navigate columns |
| `PgUp` / `PgDn` | Scroll 20 rows |
| `g` / `G` | Jump to top / bottom |

#### Overlay Navigation

| Key | Action |
|-----|--------|
| Type | Filter items in any overlay |
| `Backspace` | Clear filter character |
| `Ctrl+D` | Delete saved query (in saved overlay) |
| `Esc` | Close overlay |

### Commands

Commands are entered via the command bar (`:` key):

| Command | Description |
|---------|-------------|
| `:quit` | Exit logsht |
| `:refresh` | Reload schemas from server |
| `:clear` | Clear query and results |
| `:ctx` | Open context switcher |
| `:help` | Show keybindings |
| `:account` | Open account picker |
| `:saved` | Open saved queries |
| `:save` | Save current query |
| `:cell` | Fullscreen focused cell value |
| `:row` | Expand focused row as key-value pairs |
| `:copy cell` | Copy focused cell to clipboard |
| `:copy row` | Copy focused row as JSON |
| `:copy json` | Copy all results as JSON |

## Development

```bash
cd logsh && cargo build          # Debug build
cd logsh && cargo build --release  # Release build
cd logsh && cargo test           # Run tests
cd logsh && cargo clippy -- -D warnings  # Lint
```

### Tags

- **`latest`**: Latest stable release
- **`latest-pre`**: Latest development build (main/master)

