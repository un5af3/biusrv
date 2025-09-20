# 🚀 Biusrv - SSH Server Management Tool

A powerful Rust-based SSH server management tool for initializing, managing, and controlling multiple servers with ease.

## ✨ Features

### 🎯 Core Functionality

- **Server Initialization**: Automate user creation, SSH configuration, firewall setup, and fail2ban installation
- **Advanced Script System**: Execute complex deployment workflows with commands, file transfers, and multi-step operations
- **Intelligent Firewall Management**: Manage firewall rules with whitelist/blacklist policies and OS-aware operations
- **Robust File Transfer**: Smart file/directory upload/download with resume support and progress display
- **Command Execution**: Execute commands on single or multiple servers concurrently
- **Interactive Shell**: Connect to servers with interactive shell sessions

### 🚀 Advanced Features

- **Multi-Server Operations**: Manage multiple servers simultaneously
- **Concurrent Execution**: Parallel task execution with configurable thread pools
- **Retry Mechanism**: Intelligent retry with exponential backoff
- **Progress Display**: Beautiful progress bars for file transfers and operations
- **Resume Support**: Resume interrupted file transfers
- **Dual Format Support**: Both TOML and YAML configuration formats
- **Step-based Scripts**: Complex deployment workflows with multiple operation types
- **OS-aware Operations**: Automatic detection and adaptation to different operating systems

### 🔧 Script System

The new script system supports three operation types:

- **Command**: Execute shell commands with optional sudo privileges
- **Upload**: Transfer files and directories from local to remote
- **Download**: Transfer files and directories from remote to local

Each script can contain multiple steps, allowing for complex deployment workflows.

## 📦 Installation

### Prerequisites

- Rust 1.70+ (with Cargo)
- SSH access to target servers

### Build from Source

```bash
git clone https://github.com/un5af3/biusrv.git
cd biusrv
cargo build --release
```

### Install Binary

```bash
cargo install --path .
```

## 🚀 Quick Start

### 1. Configuration

Create a `config.toml` or `config.yaml` file:

#### TOML Format

```toml
[manage.server.pi]
host = "192.168.1.100"
port = 22
username = "root"
keypath = "/home/user/.ssh/id_rsa"

[init]
new_username = "admin"
new_password = "secure123"
packages = ["bash", "curl", "git", "vim"]
commands = ["timedatectl set-timezone Asia/Shanghai"]

[init.server.target_server]
host = "192.168.1.100"
port = 22
username = "root"
password = "initial-password"

[init.sshd]
new_port = 2222
public_key = "ssh-rsa your-public-key"

[init.firewall]
policy = "whitelist"
enable_icmp = false
allow_ping = true
allow_ports = ["2222/tcp", "80/tcp", "443/tcp"]
```

#### YAML Format

```yaml
manage:
  server:
    pi:
      host: 192.168.1.100
      port: 22
      username: root
      keypath: /home/user/.ssh/id_rsa

init:
  new_username: admin
  new_password: secure123
  packages: [bash, curl, git, vim]
  commands: [timedatectl set-timezone Asia/Shanghai]

  server:
    target_server:
      host: 192.168.1.100
      port: 22
      username: root
      password: initial-password

  sshd:
    new_port: 2222
    public_key: ssh-rsa your-public-key

  firewall:
    policy: whitelist
    enable_icmp: false
    allow_ping: true
    allow_ports: [2222/tcp, 80/tcp, 443/tcp]
```

### 2. Initialize Server

```bash
# Initialize a new server
biusrv init --server target_server

# Initialize multiple servers
biusrv init --server server1,server2

# Initialize all configured servers
biusrv init --all-servers

# List available servers for initialization
biusrv init --list-servers
```

### 3. Manage Servers

```bash
# List all configured servers
biusrv manage --list-servers

# Execute commands on multiple servers
biusrv manage --server pi,vps exec "systemctl status nginx"

# Execute scripts
biusrv manage --server pi script run scripts/docker.yaml --action uninstall,install

# Transfer files and directories
biusrv manage --server pi transfer --upload --local ./app/ --remote /opt/app/
biusrv manage --server pi transfer --download --remote /var/log/ --local ./logs/

# Transfer single file to specific path
biusrv manage --server pi transfer --upload --local ./config.conf --remote /etc/app/config.conf

# Transfer with resume support
biusrv manage --server pi transfer --upload --local ./large-file.zip --remote /tmp/large-file.zip --resume

# Force overwrite existing files
biusrv manage --server pi transfer --upload --local ./config.conf --remote /etc/app/config.conf --force

# Interactive shell
biusrv manage --server pi exec --shell bash
```

## 📖 Usage Examples

### Script Management

#### TOML Script Format

```toml
[info]
name = "nginx"
desc = "Web server and reverse proxy"

[script.install]
desc = "Install and configure nginx"

[[script.install.step]]
type = "command"
sudo = true
cmds = [
    "apt update",
    "apt install -y nginx",
    "systemctl enable nginx"
]

[[script.install.step]]
type = "upload"
local = "./configs/nginx.conf"
remote = "/etc/nginx/nginx.conf"
force = true

[[script.install.step]]
type = "command"
sudo = true
cmds = [
    "systemctl start nginx"
]
```

#### YAML Script Format

```yaml
info:
  name: nginx
  desc: Web server and reverse proxy

script:
  install:
    desc: Install and configure nginx
    step:
      - type: command
        sudo: true
        cmds:
          - apt update
          - apt install -y nginx
          - systemctl enable nginx

      - type: upload
        local: ./configs/nginx.conf
        remote: /etc/nginx/nginx.conf
        force: true

      - type: command
        sudo: true
        cmds:
          - systemctl start nginx
```

### Script Execution

```bash
# Execute specific script actions
biusrv manage --server pi script run scripts/nginx.yaml --action install

# Execute multiple actions
biusrv manage --server pi script run scripts/nginx.yaml --action install,configure

# List available actions in a script
biusrv manage script list scripts/nginx.yaml
```

### File Transfer

**Path Rules:**

- **File to File**: Specify complete file paths for both local and remote
- **Directory to Directory**: Specify directory paths for both local and remote
- **Mixed transfers are not supported**: Cannot upload a file to a directory path

```bash
# Upload single file to specific path
biusrv manage --server pi transfer --upload --local ./backup.tar.gz --remote /backup/backup.tar.gz
biusrv manage --server pi transfer --upload --local ./config.conf --remote /etc/app/config.conf

# Upload directory to directory
biusrv manage --server pi transfer --upload --local ./app/ --remote /opt/app/

# Download single file to specific path
biusrv manage --server pi transfer --download --remote /var/log/app.log --local ./logs/app.log --resume
biusrv manage --server pi transfer --download --remote /etc/nginx/nginx.conf --local ./configs/nginx.conf

# Download directory to directory
biusrv manage --server pi transfer --download --remote /var/log/ --local ./logs/

# Force overwrite existing files
biusrv manage --server pi transfer --upload --local ./config.conf --remote /etc/app/config.conf --force

# Resume interrupted transfers
biusrv manage --server pi transfer --upload --local ./large-file.zip --remote /tmp/large-file.zip --resume

# Hide progress display
biusrv manage --server pi transfer --upload --local ./large-file.zip --remote /tmp/large-file.zip --hide-progress

```

### Firewall Management

```bash
# Show firewall status
biusrv manage --server pi firewall --status

# Allow ports
biusrv manage --server pi firewall --allow-port 80,443,8080

# Deny ports
biusrv manage --server pi firewall --deny-port 23,135,445

# Delete allowed ports
biusrv manage --server pi firewall --delete-allow-port 8080

# Delete denied ports
biusrv manage --server pi firewall --delete-deny-port 23

# Save firewall rules permanently
biusrv manage --server pi firewall --allow-port 80,443 --save
```

## ⚙️ Configuration

### Server Management Configuration

```toml
[manage.server.server_name]
host = "server-ip-or-domain"     # Required: Server hostname or IP
username = "ssh-username"        # Required: SSH username
port = 22                        # Optional: SSH port (default: 22)
keypath = "/home/user/.ssh/id_rsa"        # Optional: Path to SSH private key
password = "ssh-password"        # Optional: SSH password
use_password = false             # Optional: Use password auth (default: false)
```

**Authentication Methods:**

- **Key-based (Recommended)**: Set `keypath` to your private key file
- **Password-based**: Set `password` and `use_password = true`
- **Both**: You can configure both methods for flexibility

### Server Initialization Configuration

```toml
[init]
# User creation
new_username = "admin"           # Username to create
new_password = "secure123"       # Password for new user
packages = ["bash", "curl", "git"]  # System packages to install
commands = [                     # Custom commands to run after setup
    "echo 'Welcome!' > /etc/motd",
    "timedatectl set-timezone UTC"
]

# Target server for initialization
[init.server.target_server]
host = "192.168.1.100"          # Server to initialize
port = 22                        # SSH port (default: 22)
username = "root"                # SSH username
password = "initial-password"    # Initial password for root

# SSH server configuration
[init.sshd]
new_port = 2222                  # Optional: Change SSH port
public_key = "ssh-rsa AAAAB..."  # Your public key for SSH access

[init.sshd.options]
PubkeyAuthentication = "yes"     # Enable key-based auth
PermitRootLogin = "no"           # Disable root login
PasswordAuthentication = "no"    # Disable password auth
PermitEmptyPasswords = "no"      # Disable empty passwords
MaxAuthTries = "5"               # Max authentication attempts
ClientAliveInterval = "300"      # Keep-alive interval (seconds)
X11Forwarding = "no"             # Disable X11 forwarding

# Firewall configuration
[init.firewall]
policy = "whitelist"             # Firewall policy: "whitelist" or "blacklist"
enable_icmp = false              # Enable ICMP protocol
allow_ping = true                # Allow ping (only used when enable_icmp is false)
allow_ports = ["2222/tcp", "80/tcp", "443/tcp"]  # Ports to allow

# Fail2ban configuration
[init.fail2ban]
backend = "systemd"              # Backend: "systemd" or "auto"

[init.fail2ban.jail.sshd]
enabled = true                   # Enable SSH protection
port = "2222"                    # SSH port to protect
filter = "sshd"                  # Filter to use
maxretry = 3                     # Max failed attempts
findtime = 600                   # Time window (seconds)
bantime = 3600                   # Ban duration (seconds)
```

### Script Configuration

Scripts support three operation types:

#### Command Operations

```yaml
- type: command
  sudo: true # Optional, defaults to false
  cmds:
    - apt update
    - apt install -y nginx
```

#### Upload Operations

```yaml
- type: upload
  local: ./configs/nginx.conf
  remote: /etc/nginx/nginx.conf
  force: true # Optional, defaults to false
  resume: false # Optional, defaults to false
  max_retry: 3 # Optional, defaults to 0
```

#### Download Operations

```yaml
- type: download
  local: ./backups/
  remote: /var/log/nginx/
  force: false # Optional, defaults to false
  resume: true # Optional, defaults to false
  max_retry: 2 # Optional, defaults to 0
```

## 📚 Example Scripts

The project includes comprehensive example scripts in the `examples/scripts/` directory:

- **`nginx.toml/yaml`** - Nginx web server installation and configuration
- **`mysql.toml/yaml`** - MySQL database server setup
- **`nodejs.yaml`** - Node.js runtime environment and application deployment
- **`redis.yaml`** - Redis in-memory data store setup
- **`monitoring.yaml`** - Prometheus and Grafana monitoring system
- **`docker.toml/yaml`** - Docker container runtime installation

Each script includes multiple actions (install, configure, uninstall, backup) and demonstrates best practices for server management automation.

## 🔧 Command Reference

### Global Options

- `--config <FILE>`: Configuration file path (default: config.toml)
- `--log-level <LEVEL>`: Log level (trace, debug, info, warn, error)

### Init Command

```bash
biusrv init [OPTIONS]
```

Options:

- `--list-servers`: List all configured servers for initialization
- `--all-servers`: Initialize all configured servers
- `--server <SERVERS>`: Comma-separated list of server names to initialize
- `--threads <NUM>`: Number of threads for parallel initialization
- `--max-retry <NUM>`: Maximum retry attempts (default: 0)

### Manage Command

```bash
biusrv manage [OPTIONS] <SUBCOMMAND>
```

Global Options:

- `--list-servers`: List all configured servers
- `--all-servers`: Manage all servers
- `--server <SERVERS>`: Comma-separated list of server names
- `--threads <NUM>`: Number of threads for parallel operations
- `--max-retry <NUM>`: Maximum retry attempts (default: 0)

#### Subcommands:

**Script Management:**

```bash
biusrv manage script [OPTIONS]
```

- `list <SCRIPT_FILE>`: List available actions in a script
- `run <SCRIPT_FILE> --action <ACTIONS>`: Execute specific actions from a script (comma-separated)

**Command Execution:**

```bash
biusrv manage exec <COMMAND> [OPTIONS]
```

- `--sudo`: Execute with sudo privileges
- `--hide-output`: Hide command output
- `--shell`: Start interactive shell instead of executing command

**Firewall Management:**

```bash
biusrv manage firewall [OPTIONS]
```

- `--status`: Show firewall status and port information
- `--allow-port <PORTS>`: Allow ports (comma-separated)
- `--deny-port <PORTS>`: Deny ports (comma-separated)
- `--delete-allow-port <PORTS>`: Delete allowed ports (comma-separated)
- `--delete-deny-port <PORTS>`: Delete denied ports (comma-separated)
- `--save`: Save firewall rules permanently

**File Transfer:**

```bash
biusrv manage transfer [OPTIONS]
```

- `--upload`: Upload local file to remote server
- `--download`: Download remote file to local
- `--local <PATH>`: Local file path (required for upload/download)
- `--remote <PATH>`: Remote file path (required for upload/download)
- `--force`: Force overwrite existing files
- `--resume`: Resume interrupted transfers
- `--hide-progress`: Hide transfer progress display

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- SSH functionality powered by [russh](https://github.com/warp-tech/russh)
- CLI interface built with [clap](https://github.com/clap-rs/clap)
- Progress bars by [indicatif](https://github.com/console-rs/indicatif)
- YAML support by [serde_yaml](https://github.com/dtolnay/serde-yaml)

## 📞 Support

- 🐛 [Report Issues](https://github.com/yourusername/biusrv/issues)
- 💬 [Discussions](https://github.com/yourusername/biusrv/discussions)
- 📖 [Documentation](https://github.com/yourusername/biusrv/wiki)

---

**Made with ❤️ in Rust**
