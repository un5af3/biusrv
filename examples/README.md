# Configuration Examples

This directory contains configuration examples for the server management tool, demonstrating how to use both TOML and YAML formats.

## Main Configuration Files

### config.toml / config.yaml

Main configuration files for server initialization and daily management, including:

- **Server connection info**: SSH host, port, username, key path
- **Initialization config**: New user creation, package installation, SSH configuration
- **Security config**: Firewall rules, fail2ban settings

### Usage examples:

```bash
# Use TOML configuration
cargo run -- init -c examples/config.toml

# Use YAML configuration
cargo run -- init -c examples/config.yaml
```

## Script Configuration Files

Script configuration files for automated deployment and management of various services.

### Supported script types:

#### 1. nginx.toml / nginx.yaml

Web server and reverse proxy installation, configuration and backup

#### 2. mysql.toml / mysql.yaml

MySQL database server installation, configuration and backup

#### 3. nodejs.yaml

Node.js runtime environment installation and application deployment

#### 4. redis.yaml

Redis in-memory database installation, configuration and backup

#### 5. monitoring.yaml

Prometheus and Grafana monitoring system installation and configuration

### Script operation types:

#### Command (Command execution)

```yaml
- type: command
  sudo: true
  cmds:
    - apt update
    - apt install -y nginx
```

#### Upload (File upload)

```yaml
- type: upload
  local: ./configs/nginx.conf
  remote: /etc/nginx/nginx.conf
  force: true
```

#### Download (File download)

```yaml
- type: download
  local: ./backups/
  remote: /var/log/nginx/
  resume: true
```

### Usage examples:

```bash
# Execute script
cargo run -- manage script run examples/scripts/nginx.yaml install

# List available scripts
cargo run -- manage script list examples/scripts/

# Show script information
cargo run -- manage script info examples/scripts/nginx.yaml
```

## Configuration Format Comparison

### TOML format features:

- Suitable for simple configurations
- Clear types (string, number, boolean)
- Windows path friendly

### YAML format features:

- More natural multi-line strings
- Clearer nested structure
- More concise array format
- Can omit quotes in most cases

## Security Notes

⚠️ **Important**: Passwords and keys in examples are for demonstration only. In actual use, please:

1. Use strong passwords
2. Use SSH key authentication
3. Change passwords regularly
4. Don't commit real passwords to version control

## Custom Configuration

You can create your own configuration files based on these examples:

1. Copy example files
2. Modify server information
3. Adjust script steps
4. Test configuration syntax
5. Execute deployment

## Troubleshooting

If you encounter configuration issues:

1. Check YAML/TOML syntax
2. Verify file paths
3. Confirm server connection
4. Check error logs
