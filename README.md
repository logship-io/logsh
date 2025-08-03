# logsh - Logship CLI

A command-line interface for interacting with [logship](https://logship.io). The `logsh` CLI provides powerful tools for managing connections, querying logs, uploading data, and managing subscriptions.

## Quick Start

1. **Check Status**: Run `logsh` without arguments to see connection status
2. **Add Connection**: Configure your first Logship connection
3. **Verify Connection**: Use `logsh whoami` to verify your connection
4. **Query Logs**: Start querying your log data

## Usage

```bash
# Show help and available commands
logsh --help

# Check connection status and current user
logsh
logsh whoami

# Configure connections
logsh connection add <name> <url>
logsh connection list

# Query logs
logsh query -q "Logship.Agent.Uptime | limit 100"

# Upload CSV data
logsh upload data.csv

# Manage accounts
logsh account list
logsh account default <name>

# Version information and updates
logsh version
logsh version update
```

### Output Formats

All query and list commands support multiple output formats:

```bash
# JSON
logsh query -q "Logship.Agent.Uptime | limit 100" --output json
```

## Development

### Building and Testing

```bash
# Build the project
cd logsh && cargo build

# Run with development build
cd logsh && cargo run -- --help
```

**Release Tags:**
- **`latest`**: Latest stable release version (tagged releases)
- **`latest-pre`**: Latest development build (from main/master branch)
