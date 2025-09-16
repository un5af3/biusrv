# 🚀 Biusrv - SSH Server Management Tool

A powerful Rust-based SSH server management tool for initializing, managing, and controlling multiple servers with ease.

## ✨ Features

### 🎯 Core Functionality

- **Server Initialization**: Automate user creation, SSH configuration, firewall setup, and fail2ban installation
- **Component Management**: Install, uninstall, and manage server components (Docker, Node.js, etc.)
- **Firewall Management**: Manage firewall rules with port allow/deny operations
- **File Transfer**: Robust file upload/download with resume support and progress display
- **Command Execution**: Execute commands on single or multiple servers concurrently
- **Interactive Shell**: Connect to servers with interactive shell sessions

### 🚀 Advanced Features

- **Multi-Server Operations**: Manage multiple servers simultaneously
- **Concurrent Execution**: Parallel task execution with configurable thread pools
- **Retry Mechanism**: Intelligent retry with exponential backoff
- **Progress Display**: Beautiful progress bars for file transfers and operations
- **Resume Support**: Resume interrupted file transfers
- **Configuration-Driven**: TOML-based configuration for easy setup

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

Create a `config.toml` file:

```toml
# ===========================================
# Server Management Configuration
# ===========================================

[manage.server.pi]
host = "192.168.1.100"
port = 22                    # Optional, defaults to 22
username = "root"
keypath = "~/.ssh/id_rsa"    # Optional, for key-based auth
password = "your-password"   # Optional, for password auth
use_password = false         # Optional, defaults to false

[manage.server.vps]
host = "your-vps.com"
port = 2222                  # Custom SSH port
username = "admin"
keypath = "~/.ssh/vps_key"   # SSH key path
# password = "secret"        # Uncomment for password auth
# use_password = true        # Uncomment to enable password auth

[manage.server.production]
host = "prod.example.com"
username = "deploy"
keypath = "~/.ssh/prod_key"
port = 22

# ===========================================
# Server Initialization Configuration
# ===========================================

[init]
# User creation settings
new_username = "admin"       # Username to create
new_password = "secure123"   # Password for new user
packages = ["bash", "curl", "git", "vim", "htop"]  # Packages to install
commands = [                 # Custom commands to run
    "echo 'Welcome to the server!' > /etc/motd",
    "timedatectl set-timezone Asia/Shanghai"
]

# Target server for initialization
[init.server.target_server]
host = "192.168.1.100"       # Server to initialize
port = 22                     # SSH port (default: 22)
username = "root"             # SSH username
password = "initial-password" # Initial password for root
# keypath = "~/.ssh/id_rsa"   # Optional: SSH key path

# SSH server configuration
[init.sshd]
new_port = 2222              # Optional, change SSH port
public_key = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABgQC... your-public-key"

[init.sshd.options]
PubkeyAuthentication = "yes"
PermitRootLogin = "no"
PasswordAuthentication = "no"
PermitEmptyPasswords = "no"
MaxAuthTries = "5"
ClientAliveInterval = "300"
X11Forwarding = "no"

# Firewall configuration
[init.firewall]
allow_ports = ["2222/tcp", "80/tcp", "443/tcp", "3306/tcp"]
deny_ports = ["22/tcp"]      # Block default SSH port

# Fail2ban configuration
[init.fail2ban]
backend = "systemd"          # or "auto"

[init.fail2ban.jail.sshd]
enabled = true
port = "2222"                # SSH port to protect
filter = "sshd"
maxretry = 3                 # Max failed attempts
findtime = 600               # Time window (seconds)
bantime = 3600               # Ban duration (seconds)

# ===========================================
# Component Configuration (Optional)
# ===========================================

# Components are defined in separate TOML files in components/ directory
# Example: components/docker.toml, components/nodejs.toml, etc.
```

### 2. Initialize Server

```bash
# Initialize a new server
biusrv init --server 192.168.1.100

# Initialize with custom settings
biusrv init --server 192.168.1.100 --users admin,deploy --ssh-port 2222
```

### 3. Manage Servers

```bash
# List all configured servers
biusrv manage --list-servers

# Execute commands on multiple servers
biusrv manage --server pi,vps exec "systemctl status nginx"

# Install components
biusrv manage --server pi,vps component --install docker,nodejs

# Transfer files
biusrv manage --server pi transfer --upload ./app.tar.gz /opt/app.tar.gz

# Interactive shell
biusrv manage --server pi exec bash --shell
```

## 📖 Usage Examples

### Server Management

```bash
# Manage all servers
biusrv manage --all-servers component --install docker

# Manage specific servers
biusrv manage --server web1,web2,db1 firewall --allow-port 3306,5432

# Execute with sudo
biusrv manage --server pi exec "systemctl restart nginx" --sudo

# Hide command output
biusrv manage --server pi exec "rm /tmp/old.log" --hide-output
```

### File Transfer

```bash
# Upload with progress
biusrv manage --server pi transfer --upload ./backup.tar.gz /backup/

# Download with resume
biusrv manage --server pi transfer --download /var/log/app.log ./logs/

# Force overwrite
biusrv manage --server pi transfer --upload ./config.conf /etc/app/ --force

# Hide progress
biusrv manage --server pi transfer --upload ./large-file.zip /tmp/ --hide-progress
```

### Component Management

```bash
# List available components
biusrv manage component --list

# Install multiple components
biusrv manage --server pi component --install docker,nodejs,nginx

# Uninstall components
biusrv manage --server pi component --uninstall old-service
```

### Firewall Management

```bash
# Show firewall status
biusrv manage --server pi firewall --status-port

# Allow ports
biusrv manage --server pi firewall --allow-port 80,443,8080

# Deny ports
biusrv manage --server pi firewall --deny-port 23,135,445

# Delete rules
biusrv manage --server pi firewall --delete-allow-port 8080
```

## ⚙️ Configuration

### Server Management Configuration

```toml
[manage.server.server_name]
host = "server-ip-or-domain"     # Required: Server hostname or IP
username = "ssh-username"        # Required: SSH username
port = 22                        # Optional: SSH port (default: 22)
keypath = "~/.ssh/id_rsa"        # Optional: Path to SSH private key
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
# keypath = "~/.ssh/id_rsa"      # Optional: SSH key path

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
allow_ports = ["2222/tcp", "80/tcp", "443/tcp"]  # Ports to allow
deny_ports = ["22/tcp"]          # Ports to deny

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

### Component Configuration

Components are defined in TOML files in the `components/` directory:

```toml
# components/docker.toml
name = "docker"
description = "Docker container runtime"
install_commands = [
    "curl -fsSL https://get.docker.com -o get-docker.sh",
    "sh get-docker.sh",
    "systemctl enable docker",
    "systemctl start docker"
]
uninstall_commands = [
    "systemctl stop docker",
    "apt-get remove -y docker-ce docker-ce-cli containerd.io"
]
```

## 🔧 Command Reference

### Global Options

- `--config <FILE>`: Configuration file path (default: config.toml)
- `--log-level <LEVEL>`: Log level (trace, debug, info, warn, error)
- `--comp-dir <DIR>`: Component directory path (default: components)

### Init Command

```bash
biusrv init [OPTIONS] --server <SERVER>
```

Options:

- `--users <USERS>`: Comma-separated list of users to create
- `--ssh-port <PORT>`: SSH port to configure
- `--firewall-ports <PORTS>`: Comma-separated list of ports to allow

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

**Component Management:**

```bash
biusrv manage component [OPTIONS]
```

- `--list`: List all available components
- `--install <COMPONENTS>`: Install components
- `--uninstall <COMPONENTS>`: Uninstall components

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

- `--status-port`: Show firewall status and port information
- `--allow-port <PORTS>`: Allow ports
- `--deny-port <PORTS>`: Deny ports
- `--delete-allow-port <PORTS>`: Delete allowed ports
- `--delete-deny-port <PORTS>`: Delete denied ports

**File Transfer:**

```bash
biusrv manage transfer [OPTIONS]
```

- `--upload <LOCAL> <REMOTE>`: Upload file to remote server
- `--download <REMOTE> <LOCAL>`: Download file from remote server
- `--force`: Force overwrite existing files
- `--resume`: Resume interrupted transfers
- `--hide-progress`: Hide transfer progress

## 🏗️ Architecture

### Project Structure

```
src/
├── cli/                    # CLI interface
│   ├── manage/            # Management subcommands
│   │   ├── component.rs   # Component management
│   │   ├── exec.rs        # Command execution & shell
│   │   ├── firewall.rs    # Firewall management
│   │   └── transfer.rs    # File transfer
│   ├── executor.rs        # Parallel task execution
│   └── multishell.rs      # Multi-server shell sessions
├── component/             # Component management
├── config.rs              # Configuration handling
├── ssh.rs                 # SSH client implementation
├── transfer.rs            # File transfer core
└── lib.rs                 # Library exports
```

### Key Design Patterns

- **Unified Execution Model**: All commands follow `local_execute`/`remote_execute` pattern
- **Concurrent Task Execution**: Built-in parallel execution framework
- **Retry Mechanism**: Exponential backoff with configurable retry counts
- **Progress Display**: Real-time progress bars for long-running operations

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

## 📞 Support

- 🐛 [Report Issues](https://github.com/yourusername/biusrv/issues)
- 💬 [Discussions](https://github.com/yourusername/biusrv/discussions)
- 📖 [Documentation](https://github.com/yourusername/biusrv/wiki)

---

**Made with ❤️ in Rust**
