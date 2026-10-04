# logsh - Logship CLI

A command-line interface for interacting with [Logship](https://logship.io). Query logs, upload data, manage connections and accounts — designed for both human users and CI/automation pipelines.

## Quick Start

```bash
# Start guided setup (interactive terminal)
logsh

# Or add a context directly, then choose an account when prompted
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
logsh ctx add https://logship.example.com --pat --account <account-name-or-id>
# Token comes from LOGSH_PAT_TOKEN; --token-stdin also accepts a piped token.

# Add with OAuth device flow
logsh ctx add https://logship.example.com --sso

# List contexts
logsh ctx ls
logsh ctx ls -o json   # Machine-readable

# Switch context
logsh ctx use prod

# Show current context
logsh ctx current

# Show all contexts/accounts and common actions
logsh ctx

# Re-authenticate
logsh ctx login

# Re-authenticate in a script using a password piped to stdin
logsh ctx login prod --password-stdin

# Remove a context
logsh ctx rm old-ctx
```

Login never chooses an account automatically, even when only one is available.
Interactive setup asks you to choose; scripts supply `--account <name-or-id>`.
Re-login retains a saved selection while it remains accessible. Authentication with
no accessible accounts saves the context and explains why account commands are unavailable.
Canceling setup leaves existing configuration intact.

Without a terminal, bare `logsh` prints setup instructions instead of prompting.
With a connection, it shows status, server, user, account, and useful next commands.
Use `--help` for compact help and `--help-all` for advanced options. `--config-path`
and `LOGSH_CONFIG_PATH` may point to a configuration file that does not exist yet.

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
logsh acc use <account-id>
logsh acc current
logsh acc show
logsh acc permissions
logsh acc create <name>              # Generates an ID; --id supplies one
logsh acc update --name <new-name>
logsh acc delete <account-id>
```

Account names match without case sensitivity; ambiguous names require a UUID.
An invalid `--account` or `--context` fails rather than falling back to another
account or server. Command overrides do not change the saved selection.

### Account Users and Permissions

```bash
logsh account users list
logsh account users list -o json
logsh account users show <user-id>
logsh account users add <username> --permission <permission-name>
logsh account users remove <user-id>

# Preserve unrelated permissions
logsh account users permissions grant <user-id> --permission <permission-name>
logsh account users permissions revoke <user-id> --permission <permission-name>

# Replace the full list; repeat --permission as needed
logsh account users permissions set <user-id> --permission <permission-name>
logsh account users permissions set <user-id> --clear

logsh account users schema-permissions list <user-id>
logsh account users schema-permissions set <user-id> --file permissions.json
logsh account users schema-permissions set <user-id> --file -
logsh account users schema-permissions set <user-id> --clear
```

`users add` may create a user invitation and send email through the backend.
Grant/revoke reads the current permission list and writes a replacement; concurrent
permission edits are not atomic. The backend validates permission names and access.
`account permissions` displays the caller's permissions; `users show` displays a
specific user's permissions.

Schema-permission input is an array matching the API, for example:

```json
[
  {
    "schemaName": "Logs",
    "canRead": true,
    "canWrite": false,
    "columns": [
      { "columnName": "Message", "canRead": true, "canWrite": false }
    ]
  }
]
```

Use `--clear` to replace permissions with an empty list.

### Account Settings and Groups

```bash
logsh account settings list
logsh account settings set <setting-name> --value-json 'true'
logsh account settings set <setting-name> --value-json '"a string"'

logsh account groups list
logsh account groups create <name> --description "Operations team"
logsh account groups update <group-id> --name <new-name>
logsh account groups delete <group-id>
logsh account groups members list <group-id>
logsh account groups members add <group-id> <user-id>
logsh account groups members remove <group-id> <user-id>
```

New group IDs are generated unless `--id` is supplied. Settings and membership
commands use the selected account or the temporary `--account` override. All new
administration commands support table, JSON, pretty JSON, CSV, and Markdown output.

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
| `--account <name-or-id>` | Override the account for a command; setup persists this choice |
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

With no contexts, `logsht` opens a login form. Enter the server and context name,
choose password, PAT, or SSO using Left/Right on the authentication field, and press
Enter to log in. Tab/Shift+Tab moves between fields; passwords and tokens are masked.
SSO displays the browser URL and device code while the UI stays responsive.
Choose an account to complete setup. Esc cancels without saving the pending login.

The bottom row always shows the main controls: **F1 Commands**, **F2 Account**,
**F3 Context**, and **F4 Login**. F1 opens a searchable menu with descriptions;
type to filter, use Up/Down to select, and Enter to run a command. F2 opens the
account picker directly, including while editing a query.

Use F4 to re-authenticate or choose `context add` from the F1 menu to connect to another server.
In the context switcher, Ctrl+A adds a context and Ctrl+L opens login. Expired
authentication opens the login form. Account administration is available through
the `logsh account` commands.

### Layout

logsht has three main panes:

- **Schemas** — browse and filter tables on the connected server
- **Editor** — write and edit queries with full text editing
- **Results** — navigate query results with keyboard controls

### Keybindings

| Key | Action |
|-----|--------|
| `F1` | Browse and search commands |
| `F2` | Choose account |
| `F3` | Open context switcher |
| `F4` | Log in to the current context |
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

Press F1 to browse, search, and run commands. You can also enter them via the
command bar (`:` key):

| Command | Description |
|---------|-------------|
| `:quit` | Exit logsht |
| `:refresh` | Reload schemas from server |
| `:clear` | Clear query and results |
| `:ctx` | Open context switcher |
| `:login` | Log in to the current context |
| `:context add` | Add a context through the login form |
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
cd logsh && cargo test --workspace       # Run CLI, core API, and TUI tests
cd logsh && cargo clippy --workspace --all-targets -- -D warnings  # Lint
cd logsh && cargo fmt --all -- --check   # Formatting
```

### Tags

- **`latest`**: Latest stable release
- **`latest-pre`**: Latest development build (main/master)

