# logsh - Logship CLI

A command-line interface for interacting with [logship](https://logship.io). The `logsh` CLI provides powerful tools for managing connections, querying logs, uploading data, and managing subscriptions.

## Quick Start

1. **Check Status**: `logsh` without arguments or `logsh whoami`
2. **Add Connection**: `logsh connection add basic <name> <url>`
3. **Query Logs**: `logsh query -q "Logship.Agent.Uptime | limit 100"`

## Usage

```bash
# Show help and available commands
logsh --help

# Check connection status and current user
logsh
logsh whoami

# Configure connections
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

## Container Quick Start

### Basic Usage
```bash
# Show help
podman run --rm ghcr.io/logship-io/logsh:latest

# Show version
podman run --rm ghcr.io/logship-io/logsh:latest version
```

## Configuration Management

The container is designed to work with your local logsh configuration files.

### Environment Variables

- `LOGSH_CONFIG_PATH` - Path to the configuration file inside the container (default: `/config/logsh-config.json`)

### Mounting Your Local Configuration

#### Option 1: Mount entire config directory
```bash
# Mount your local ~/.logsh directory to /config in the container
podman run --rm \
  -v ~/.logsh:/config:Z \
  logsh:latest query -q "SELECT * FROM logs LIMIT 10"
```

#### Option 2: Mount specific config file
```bash
# Mount just the config file
podman run --rm \
  -v ~/.logsh/logsh-config.json:/config/logsh-config.json:Z \
  logsh:latest whoami
```

### Using with Docker Compose

Create a `docker-compose.yml`:

```yaml
version: '3.8'
services:
  logsh:
    image: logsh:latest
    volumes:
      - ~/.logsh:/config:ro
    environment:
      - LOGSH_CONFIG_PATH=/config/logsh-config.json
    command: ["query", "-q", "SELECT * FROM logs LIMIT 5"]
```

Run with:
```bash
docker-compose run --rm logsh
```

## Common Usage Patterns

### Interactive Query
```bash
podman run --rm -it \
  -v ~/.logsh:/config:Z \
  logsh:latest query -q "SELECT timestamp, message FROM logs WHERE level='ERROR'"
```

### Upload Logs
```bash
podman run --rm \
  -v ~/.logsh:/config:Z \
  -v /path/to/logs:/logs:Z \
  logsh:latest upload /logs/app.log
```

### Check Connection Status
```bash
podman run --rm \
  -v ~/.logsh:/config:Z \
  logsh:latest whoami
```

### Account Management
```bash
# List accounts
podman run --rm \
  -v ~/.logsh:/config:Z \
  logsh:latest account ls

# Set default account
podman run --rm \
  -v ~/.logsh:/config:Z \
  logsh:latest account default 00000000-0000-0000-0000-000000000000
```

## Troubleshooting

### Configuration File Not Found
Ensure your local configuration file exists and is properly mounted:
```bash
# Check if config exists locally
ls -la ~/.logsh/logsh-config.json

# Check if mounted correctly in container
podman run --rm \
  -v ~/.logsh:/config:Z \
  logsh:latest ls -la /config/
```

### Permission Issues
If you encounter permission issues, ensure the mounted volume has correct SELinux context (`:Z` flag) or permissions.

### Network Connectivity
The container may need network access to connect:
```bash
# Ensure network access
podman run --rm --network=host \
  -v ~/.logsh:/config:Z \
  logsh:latest whoami
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

