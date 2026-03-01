# logsh - Logship CLI

A command-line interface for interacting with [Logship](https://logship.io). Query logs, upload data, manage connections and accounts — designed for both human users and CI/automation pipelines.

## Quick Start

```bash
# Add a context
logsh context add basic myctx https://my.logship.server

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
# Add with username/password
logsh context add basic prod https://logship.example.com

# Add with Personal Access Token (CI/automation)
logsh context add pat ci-ctx https://logship.example.com -t $LOGSH_PAT_TOKEN

# Add with OAuth device flow
logsh context add oauth myctx https://logship.example.com

# List contexts
logsh context list
logsh context list -o json   # Machine-readable

# Switch context
logsh context use prod

# Re-authenticate
logsh context login
```

### Queries

```bash
# Interactive query
logsh query -q 'MyTable | take 10'

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

### Accounts

```bash
logsh account list
logsh account list --include-all
logsh account default <account-id>
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
| `-v` / `-vvvv` | Increase verbosity (warn → trace) |
| `--no-color` | Disable colored output |
| `--context <name>` | Use a specific named context |
| `--config-path <path>` | Override config file location |
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
  -v ~/.logsh/logsh-config.json:/config/logsh-config.json:Z \
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

