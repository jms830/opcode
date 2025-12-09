//! Shell environment detection and configuration for Windows
//!
//! This module provides support for running Claude Code through different shell
//! environments on Windows, including:
//! - PowerShell (default Windows shell)
//! - WSL (Windows Subsystem for Linux)
//! - Git Bash
//!
//! For WSL users who have Claude installed in their Linux environment, this allows
//! opcode to bridge the Windows GUI with the WSL-installed Claude CLI.

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Command;

/// Windows constant for CREATE_NO_WINDOW flag
/// This prevents console windows from flashing when running background commands
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Creates a WSL command with CREATE_NO_WINDOW flag to prevent terminal flashing
#[cfg(windows)]
fn wsl_command() -> Command {
    let mut cmd = Command::new("wsl");
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Available shell environments for Claude execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ShellEnvironment {
    /// Native Windows (PowerShell/CMD) - default
    #[default]
    Native,
    /// Windows Subsystem for Linux
    Wsl,
    /// Git Bash (MSYS2/MinGW)
    GitBash,
}

impl std::fmt::Display for ShellEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellEnvironment::Native => write!(f, "native"),
            ShellEnvironment::Wsl => write!(f, "wsl"),
            ShellEnvironment::GitBash => write!(f, "gitbash"),
        }
    }
}

impl std::str::FromStr for ShellEnvironment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "native" | "powershell" | "cmd" => Ok(ShellEnvironment::Native),
            "wsl" | "wsl2" => Ok(ShellEnvironment::Wsl),
            "gitbash" | "git-bash" | "git_bash" | "bash" => Ok(ShellEnvironment::GitBash),
            _ => Err(format!("Unknown shell environment: {}", s)),
        }
    }
}

/// Information about an available WSL distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WslDistribution {
    /// Name of the distribution (e.g., "Ubuntu", "Debian")
    pub name: String,
    /// Whether this is the default distribution
    pub is_default: bool,
    /// WSL version (1 or 2)
    pub version: Option<u8>,
}

/// Detected shell environments available on the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableShells {
    /// Native Windows is always available on Windows
    pub native: bool,
    /// WSL distributions if available
    pub wsl_distributions: Vec<WslDistribution>,
    /// Git Bash path if available
    pub git_bash_path: Option<String>,
}

/// Detect available shell environments on the current system
#[cfg(windows)]
pub fn detect_available_shells() -> AvailableShells {
    info!("Detecting available shell environments on Windows...");

    AvailableShells {
        native: true,
        wsl_distributions: detect_wsl_distributions(),
        git_bash_path: detect_git_bash(),
    }
}

#[cfg(not(windows))]
pub fn detect_available_shells() -> AvailableShells {
    // On non-Windows, only native shell is relevant
    AvailableShells {
        native: true,
        wsl_distributions: vec![],
        git_bash_path: None,
    }
}

/// Detect installed WSL distributions
#[cfg(windows)]
fn detect_wsl_distributions() -> Vec<WslDistribution> {
    debug!("Detecting WSL distributions...");

    let mut distributions = Vec::new();

    // Run `wsl --list --verbose` to get distributions
    match wsl_command().args(["--list", "--verbose"]).output() {
        Ok(output) if output.status.success() => {
            // WSL outputs UTF-16LE on Windows, need to handle that
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Also try UTF-16LE decoding
            let stdout_utf16: String = if stdout.chars().filter(|c| *c == '\0').count() > 5 {
                // Likely UTF-16LE encoded
                let bytes: Vec<u16> = output
                    .stdout
                    .chunks_exact(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();
                String::from_utf16_lossy(&bytes)
            } else {
                stdout.to_string()
            };

            debug!("WSL list output: {:?}", stdout_utf16);

            // Parse the output (skip header line)
            for line in stdout_utf16.lines().skip(1) {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                // Format: "* Ubuntu    Running    2" or "  Debian    Stopped    1"
                let is_default = line.starts_with('*');
                let parts: Vec<&str> = line.trim_start_matches('*').split_whitespace().collect();

                if let Some(name) = parts.first() {
                    let name = name.to_string();
                    // Skip if it looks like a header
                    if name == "NAME" || name.is_empty() {
                        continue;
                    }

                    let version = parts.get(2).and_then(|v| v.parse().ok());

                    debug!(
                        "Found WSL distribution: {} (default: {}, version: {:?})",
                        name, is_default, version
                    );

                    distributions.push(WslDistribution {
                        name,
                        is_default,
                        version,
                    });
                }
            }
        }
        Ok(output) => {
            debug!(
                "WSL command failed: {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            debug!("WSL not available: {}", e);
        }
    }

    distributions
}

#[cfg(not(windows))]
fn detect_wsl_distributions() -> Vec<WslDistribution> {
    vec![]
}

/// Detect Git Bash installation
#[cfg(windows)]
fn detect_git_bash() -> Option<String> {
    debug!("Detecting Git Bash...");

    // Common Git Bash locations
    let paths = [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ];

    for path in &paths {
        if std::path::Path::new(path).exists() {
            info!("Found Git Bash at: {}", path);
            return Some(path.to_string());
        }
    }

    // Also check if git bash is in PATH
    if let Ok(output) = Command::new("where").arg("bash.exe").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .map(|s| s.trim().to_string());
            if let Some(ref p) = path {
                if p.to_lowercase().contains("git") {
                    info!("Found Git Bash in PATH: {}", p);
                    return path;
                }
            }
        }
    }

    None
}

#[cfg(not(windows))]
fn detect_git_bash() -> Option<String> {
    None
}

/// Check if Claude is installed in WSL
#[cfg(windows)]
pub fn check_claude_in_wsl(distro: Option<&str>) -> Option<String> {
    debug!("Checking for Claude in WSL (distro: {:?})...", distro);

    let mut cmd = wsl_command();

    if let Some(d) = distro {
        cmd.args(["-d", d]);
    }

    // Check common Claude installation paths in WSL
    cmd.args(["bash", "-lc", "command -v claude || echo ''"]);

    match cmd.output() {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && path != "''" {
                info!("Found Claude in WSL: {}", path);
                return Some(path);
            }
        }
        Ok(output) => {
            debug!(
                "WSL claude check failed: {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(e) => {
            warn!("Failed to check Claude in WSL: {}", e);
        }
    }

    None
}

#[cfg(not(windows))]
pub fn check_claude_in_wsl(_distro: Option<&str>) -> Option<String> {
    None
}

/// Convert a Windows path to WSL path format
/// e.g., C:\Users\user\project -> /mnt/c/Users/user/project
/// Also handles WSL UNC paths: \\wsl.localhost\Ubuntu\home\user -> /home/user
#[cfg(windows)]
pub fn windows_to_wsl_path(windows_path: &str) -> String {
    // Handle UNC paths and standard paths
    let path = windows_path.replace('\\', "/");

    // Check for WSL UNC paths first: //wsl.localhost/Distro/path or //wsl$/Distro/path
    if let Some(rest) = path.strip_prefix("//wsl.localhost/") {
        // Format: //wsl.localhost/Ubuntu/home/user/... -> /home/user/...
        // Skip the distro name (first path component)
        if let Some(slash_pos) = rest.find('/') {
            return rest[slash_pos..].to_string();
        }
        return format!("/{}", rest);
    }
    
    if let Some(rest) = path.strip_prefix("//wsl$/") {
        // Format: //wsl$/Ubuntu/home/user/... -> /home/user/...
        // Skip the distro name (first path component)
        if let Some(slash_pos) = rest.find('/') {
            return rest[slash_pos..].to_string();
        }
        return format!("/{}", rest);
    }

    if let Some(rest) = path.strip_prefix("//") {
        // Other UNC paths: \\server\share -> /mnt/server/share (approximate)
        format!("/mnt/{}", rest)
    } else if path.len() >= 2 && path.chars().nth(1) == Some(':') {
        // Drive letter path: C:/... -> /mnt/c/...
        let drive = path.chars().next().unwrap().to_lowercase().next().unwrap();
        let rest = &path[2..];
        format!("/mnt/{}{}", drive, rest)
    } else {
        path
    }
}

#[cfg(not(windows))]
pub fn windows_to_wsl_path(path: &str) -> String {
    path.to_string()
}

/// Create a command that runs through WSL
/// Uses CREATE_NO_WINDOW flag to prevent terminal flashing
#[cfg(windows)]
pub fn create_wsl_command(
    distro: Option<&str>,
    claude_path: &str,
    args: &[String],
    working_dir: &str,
) -> Command {
    let mut cmd = wsl_command();

    // Specify distribution if provided
    if let Some(d) = distro {
        cmd.args(["-d", d]);
    }

    // Convert working directory to WSL path
    let wsl_working_dir = windows_to_wsl_path(working_dir);

    // Build the full command to run in bash
    // Use bash -lc to get a login shell with proper PATH
    let claude_args: String = args
        .iter()
        .map(|arg| {
            // Escape special characters for bash
            let escaped = arg
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('$', "\\$")
                .replace('`', "\\`");
            format!("\"{}\"", escaped)
        })
        .collect::<Vec<_>>()
        .join(" ");

    let bash_command = format!(
        "cd '{}' && {} {}",
        wsl_working_dir.replace('\'', "'\\''"),
        claude_path,
        claude_args
    );

    debug!("WSL bash command: {}", bash_command);

    cmd.args(["bash", "-lc", &bash_command]);

    cmd
}

#[cfg(not(windows))]
pub fn create_wsl_command(
    _distro: Option<&str>,
    claude_path: &str,
    args: &[String],
    working_dir: &str,
) -> Command {
    // On non-Windows, just create a regular command
    let mut cmd = Command::new(claude_path);
    cmd.args(args);
    cmd.current_dir(working_dir);
    cmd
}

/// Shell configuration stored in settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShellConfig {
    /// The preferred shell environment
    pub environment: ShellEnvironment,
    /// WSL distribution name (if using WSL)
    pub wsl_distro: Option<String>,
    /// Path to Claude in WSL (if using WSL)
    pub wsl_claude_path: Option<String>,
    /// Path to Git Bash (if using Git Bash)
    pub git_bash_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_environment_parsing() {
        assert_eq!(
            "wsl".parse::<ShellEnvironment>().unwrap(),
            ShellEnvironment::Wsl
        );
        assert_eq!(
            "native".parse::<ShellEnvironment>().unwrap(),
            ShellEnvironment::Native
        );
        assert_eq!(
            "gitbash".parse::<ShellEnvironment>().unwrap(),
            ShellEnvironment::GitBash
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_to_wsl_path() {
        // Standard Windows drive paths
        assert_eq!(
            windows_to_wsl_path(r"C:\Users\test\project"),
            "/mnt/c/Users/test/project"
        );
        assert_eq!(windows_to_wsl_path(r"D:\dev\myapp"), "/mnt/d/dev/myapp");
        
        // WSL UNC paths - these should extract the Linux path
        assert_eq!(
            windows_to_wsl_path(r"\\wsl.localhost\Ubuntu\home\jordan\project"),
            "/home/jordan/project"
        );
        assert_eq!(
            windows_to_wsl_path(r"\\wsl$\Ubuntu\home\user\code"),
            "/home/user/code"
        );
        
        // Already a Linux path (passthrough)
        assert_eq!(
            windows_to_wsl_path("/home/jordan/project"),
            "/home/jordan/project"
        );
    }
}
