use anyhow::Result;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
/// Shared module for detecting Claude Code binary installations
/// Supports NVM installations, aliased paths, and version-based selection
use std::path::PathBuf;
use std::process::Command;
use tauri::Manager;

/// Windows constant for CREATE_NO_WINDOW flag
/// This prevents console windows from flashing when running background commands
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Type of Claude installation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InstallationType {
    /// System-installed binary
    System,
    /// Custom path specified by user
    Custom,
}

/// Represents a Claude installation with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeInstallation {
    /// Full path to the Claude binary
    pub path: String,
    /// Version string if available
    pub version: Option<String>,
    /// Source of discovery (e.g., "nvm", "system", "homebrew", "which", "wsl")
    pub source: String,
    /// Type of installation
    pub installation_type: InstallationType,
    /// WSL distribution name (if this is a WSL installation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wsl_distro: Option<String>,
}

/// Main function to find the Claude binary
/// Checks database first for stored path and preference, then prioritizes accordingly
pub fn find_claude_binary(app_handle: &tauri::AppHandle) -> Result<String, String> {
    info!("Searching for claude binary...");

    // First check if we have a stored path and preference in the database
    if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
        let db_path = app_data_dir.join("agents.db");
        if db_path.exists() {
            if let Ok(conn) = rusqlite::Connection::open(&db_path) {
                // Check for stored path first
                if let Ok(stored_path) = conn.query_row(
                    "SELECT value FROM app_settings WHERE key = 'claude_binary_path'",
                    [],
                    |row| row.get::<_, String>(0),
                ) {
                    info!("Found stored claude path in database: {}", stored_path);

                    // Check if the path still exists
                    let path_buf = PathBuf::from(&stored_path);
                    if path_buf.exists() && path_buf.is_file() {
                        return Ok(stored_path);
                    } else {
                        warn!("Stored claude path no longer exists: {}", stored_path);
                    }
                }

                // Check user preference
                let preference = conn.query_row(
                    "SELECT value FROM app_settings WHERE key = 'claude_installation_preference'",
                    [],
                    |row| row.get::<_, String>(0),
                ).unwrap_or_else(|_| "system".to_string());

                info!("User preference for Claude installation: {}", preference);
            }
        }
    }

    // Discover all available system installations
    let installations = discover_system_installations();

    if installations.is_empty() {
        error!("Could not find claude binary in any location");
        return Err("Claude Code not found. Please ensure it's installed in one of these locations: PATH, /usr/local/bin, /opt/homebrew/bin, ~/.nvm/versions/node/*/bin, ~/.claude/local, ~/.local/bin".to_string());
    }

    // Log all found installations
    for installation in &installations {
        info!("Found Claude installation: {:?}", installation);
    }

    // Select the best installation (highest version)
    if let Some(best) = select_best_installation(installations) {
        info!(
            "Selected Claude installation: path={}, version={:?}, source={}",
            best.path, best.version, best.source
        );
        Ok(best.path)
    } else {
        Err("No valid Claude installation found".to_string())
    }
}

/// Discovers all available Claude installations and returns them for selection
/// This allows UI to show a version selector
pub fn discover_claude_installations() -> Vec<ClaudeInstallation> {
    info!("Discovering all Claude installations...");

    let mut installations = discover_system_installations();

    // Sort by version (highest first), then by source preference
    installations.sort_by(|a, b| {
        match (&a.version, &b.version) {
            (Some(v1), Some(v2)) => {
                // Compare versions in descending order (newest first)
                match compare_versions(v2, v1) {
                    Ordering::Equal => {
                        // If versions are equal, prefer by source
                        source_preference(a).cmp(&source_preference(b))
                    }
                    other => other,
                }
            }
            (Some(_), None) => Ordering::Less, // Version comes before no version
            (None, Some(_)) => Ordering::Greater,
            (None, None) => source_preference(a).cmp(&source_preference(b)),
        }
    });

    installations
}

/// Returns a preference score for installation sources (lower is better)
fn source_preference(installation: &ClaudeInstallation) -> u8 {
    match installation.source.as_str() {
        "which" => 1,
        "homebrew" => 2,
        "system" => 3,
        "nvm-active" => 4,
        source if source.starts_with("nvm") => 5,
        "local-bin" => 6,
        "claude-local" => 7,
        "npm-global" => 8,
        "yarn" | "yarn-global" => 9,
        "bun" => 10,
        "node-modules" => 11,
        "home-bin" => 12,
        "PATH" => 13,
        _ => 14,
    }
}

/// Discovers all Claude installations on the system
fn discover_system_installations() -> Vec<ClaudeInstallation> {
    let mut installations = Vec::new();

    // 1. Try 'which' command first (now works in production)
    if let Some(installation) = try_which_command() {
        installations.push(installation);
    }

    // 2. Check NVM paths (includes current active NVM)
    installations.extend(find_nvm_installations());

    // 3. Check standard paths
    installations.extend(find_standard_installations());

    // Remove duplicates by path
    let mut unique_paths = std::collections::HashSet::new();
    installations.retain(|install| unique_paths.insert(install.path.clone()));

    installations
}

/// Try using the 'which' command to find Claude
#[cfg(unix)]
fn try_which_command() -> Option<ClaudeInstallation> {
    debug!("Trying 'which claude' to find binary...");

    match Command::new("which").arg("claude").output() {
        Ok(output) if output.status.success() => {
            let output_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

            if output_str.is_empty() {
                return None;
            }

            // Parse aliased output: "claude: aliased to /path/to/claude"
            let path = if output_str.starts_with("claude:") && output_str.contains("aliased to") {
                output_str
                    .split("aliased to")
                    .nth(1)
                    .map(|s| s.trim().to_string())
            } else {
                Some(output_str)
            }?;

            debug!("'which' found claude at: {}", path);

            // Verify the path exists
            if !PathBuf::from(&path).exists() {
                warn!("Path from 'which' does not exist: {}", path);
                return None;
            }

            // Get version
            let version = get_claude_version(&path).ok().flatten();

            Some(ClaudeInstallation {
                path,
                version,
                source: "which".to_string(),
                installation_type: InstallationType::System,
                wsl_distro: None,
            })
        }
        _ => None,
    }
}

#[cfg(windows)]
fn try_which_command() -> Option<ClaudeInstallation> {
    debug!("Trying 'where claude' to find binary...");

    match Command::new("where").arg("claude").output() {
        Ok(output) if output.status.success() => {
            let output_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

            if output_str.is_empty() {
                return None;
            }

            // On Windows, `where` can return multiple paths, newline-separated. We take the first one.
            let path = output_str.lines().next().unwrap_or("").trim().to_string();

            if path.is_empty() {
                return None;
            }

            debug!("'where' found claude at: {}", path);

            // Verify the path exists
            if !PathBuf::from(&path).exists() {
                warn!("Path from 'where' does not exist: {}", path);
                return None;
            }

            // Get version
            let version = get_claude_version(&path).ok().flatten();

            Some(ClaudeInstallation {
                path,
                version,
                source: "where".to_string(),
                installation_type: InstallationType::System,
                wsl_distro: None,
            })
        }
        _ => None,
    }
}

/// Find Claude installations in NVM directories
#[cfg(unix)]
fn find_nvm_installations() -> Vec<ClaudeInstallation> {
    let mut installations = Vec::new();

    // First check NVM_BIN environment variable (current active NVM)
    if let Ok(nvm_bin) = std::env::var("NVM_BIN") {
        let claude_path = PathBuf::from(&nvm_bin).join("claude");
        if claude_path.exists() && claude_path.is_file() {
            debug!("Found Claude via NVM_BIN: {:?}", claude_path);
            let version = get_claude_version(&claude_path.to_string_lossy())
                .ok()
                .flatten();
            installations.push(ClaudeInstallation {
                path: claude_path.to_string_lossy().to_string(),
                version,
                source: "nvm-active".to_string(),
                installation_type: InstallationType::System,
                wsl_distro: None,
            });
        }
    }

    // Then check all NVM directories
    if let Ok(home) = std::env::var("HOME") {
        let nvm_dir = PathBuf::from(&home)
            .join(".nvm")
            .join("versions")
            .join("node");

        debug!("Checking NVM directory: {:?}", nvm_dir);

        if let Ok(entries) = std::fs::read_dir(&nvm_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let claude_path = entry.path().join("bin").join("claude");

                    if claude_path.exists() && claude_path.is_file() {
                        let path_str = claude_path.to_string_lossy().to_string();
                        let node_version = entry.file_name().to_string_lossy().to_string();

                        debug!("Found Claude in NVM node {}: {}", node_version, path_str);

                        // Get Claude version
                        let version = get_claude_version(&path_str).ok().flatten();

                        installations.push(ClaudeInstallation {
                            path: path_str,
                            version,
                            source: format!("nvm ({})", node_version),
                            installation_type: InstallationType::System,
                            wsl_distro: None,
                        });
                    }
                }
            }
        }
    }

    installations
}

#[cfg(windows)]
fn find_nvm_installations() -> Vec<ClaudeInstallation> {
    let mut installations = Vec::new();

    if let Ok(nvm_home) = std::env::var("NVM_HOME") {
        debug!("Checking NVM_HOME directory: {:?}", nvm_home);

        if let Ok(entries) = std::fs::read_dir(&nvm_home) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let claude_path = entry.path().join("claude.exe");

                    if claude_path.exists() && claude_path.is_file() {
                        let path_str = claude_path.to_string_lossy().to_string();
                        let node_version = entry.file_name().to_string_lossy().to_string();

                        debug!("Found Claude in NVM node {}: {}", node_version, path_str);

                        // Get Claude version
                        let version = get_claude_version(&path_str).ok().flatten();

                        installations.push(ClaudeInstallation {
                            path: path_str,
                            version,
                            source: format!("nvm ({})", node_version),
                            installation_type: InstallationType::System,
                            wsl_distro: None,
                        });
                    }
                }
            }
        }
    }

    installations
}

/// Check standard installation paths
#[cfg(unix)]
fn find_standard_installations() -> Vec<ClaudeInstallation> {
    let mut installations = Vec::new();

    // Common installation paths for claude
    let mut paths_to_check: Vec<(String, String)> = vec![
        ("/usr/local/bin/claude".to_string(), "system".to_string()),
        (
            "/opt/homebrew/bin/claude".to_string(),
            "homebrew".to_string(),
        ),
        ("/usr/bin/claude".to_string(), "system".to_string()),
        ("/bin/claude".to_string(), "system".to_string()),
    ];

    // Also check user-specific paths
    if let Ok(home) = std::env::var("HOME") {
        paths_to_check.extend(vec![
            (
                format!("{}/.claude/local/claude", home),
                "claude-local".to_string(),
            ),
            (
                format!("{}/.local/bin/claude", home),
                "local-bin".to_string(),
            ),
            (
                format!("{}/.npm-global/bin/claude", home),
                "npm-global".to_string(),
            ),
            (format!("{}/.yarn/bin/claude", home), "yarn".to_string()),
            (format!("{}/.bun/bin/claude", home), "bun".to_string()),
            (format!("{}/bin/claude", home), "home-bin".to_string()),
            // Check common node_modules locations
            (
                format!("{}/node_modules/.bin/claude", home),
                "node-modules".to_string(),
            ),
            (
                format!("{}/.config/yarn/global/node_modules/.bin/claude", home),
                "yarn-global".to_string(),
            ),
        ]);
    }

    // Check each path
    for (path, source) in paths_to_check {
        let path_buf = PathBuf::from(&path);
        if path_buf.exists() && path_buf.is_file() {
            debug!("Found claude at standard path: {} ({})", path, source);

            // Get version
            let version = get_claude_version(&path).ok().flatten();

            installations.push(ClaudeInstallation {
                path,
                version,
                source,
                installation_type: InstallationType::System,
                wsl_distro: None,
            });
        }
    }

    // Also check if claude is available in PATH (without full path)
    if let Ok(output) = Command::new("claude").arg("--version").output() {
        if output.status.success() {
            debug!("claude is available in PATH");
            let version = extract_version_from_output(&output.stdout);

            installations.push(ClaudeInstallation {
                path: "claude".to_string(),
                version,
                source: "PATH".to_string(),
                installation_type: InstallationType::System,
                wsl_distro: None,
            });
        }
    }

    installations
}

#[cfg(windows)]
fn find_standard_installations() -> Vec<ClaudeInstallation> {
    let mut installations = Vec::new();

    // Common installation paths for claude on Windows
    let mut paths_to_check: Vec<(String, String)> = vec![];

    // Check user-specific paths
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        paths_to_check.extend(vec![
            (
                format!("{}\\.claude\\local\\claude.exe", user_profile),
                "claude-local".to_string(),
            ),
            (
                format!("{}\\.local\\bin\\claude.exe", user_profile),
                "local-bin".to_string(),
            ),
            (
                format!("{}\\AppData\\Roaming\\npm\\claude.cmd", user_profile),
                "npm-global".to_string(),
            ),
            (
                format!("{}\\.yarn\\bin\\claude.cmd", user_profile),
                "yarn".to_string(),
            ),
            (
                format!("{}\\.bun\\bin\\claude.exe", user_profile),
                "bun".to_string(),
            ),
        ]);
    }

    // Check each path
    for (path, source) in paths_to_check {
        let path_buf = PathBuf::from(&path);
        if path_buf.exists() && path_buf.is_file() {
            debug!("Found claude at standard path: {} ({})", path, source);

            // Get version
            let version = get_claude_version(&path).ok().flatten();

            installations.push(ClaudeInstallation {
                path,
                version,
                source,
                installation_type: InstallationType::System,
                wsl_distro: None,
            });
        }
    }

    // Also check if claude is available in PATH (without full path)
    if let Ok(output) = Command::new("claude.exe").arg("--version").output() {
        if output.status.success() {
            debug!("claude.exe is available in PATH");
            let version = extract_version_from_output(&output.stdout);

            installations.push(ClaudeInstallation {
                path: "claude.exe".to_string(),
                version,
                source: "PATH".to_string(),
                installation_type: InstallationType::System,
                wsl_distro: None,
            });
        }
    }

    // Also check WSL installations
    installations.extend(find_wsl_installations());

    installations
}

/// Find Claude installations in WSL distributions (Windows only)
#[cfg(windows)]
fn find_wsl_installations() -> Vec<ClaudeInstallation> {
    let mut installations = Vec::new();

    debug!("Checking for Claude installations in WSL...");

    // Get list of WSL distributions
    let distros = match get_wsl_distributions() {
        Ok(d) => d,
        Err(e) => {
            debug!("Failed to get WSL distributions: {}", e);
            return installations;
        }
    };

    for distro in distros {
        debug!("Checking WSL distribution: {}", distro);

        // Try to find claude in this distribution
        if let Some(claude_path) = find_claude_in_wsl(&distro) {
            debug!("Found Claude in WSL {}: {}", distro, claude_path);

            // Get version
            let version = get_claude_version_in_wsl(&distro, &claude_path);

            installations.push(ClaudeInstallation {
                path: claude_path,
                version,
                source: format!("wsl ({})", distro),
                installation_type: InstallationType::System,
                wsl_distro: Some(distro),
            });
        }
    }

    installations
}

/// Creates a WSL command with CREATE_NO_WINDOW flag to prevent terminal flashing
#[cfg(windows)]
fn wsl_command() -> Command {
    let mut cmd = Command::new("wsl");
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Get list of WSL distributions
#[cfg(windows)]
fn get_wsl_distributions() -> Result<Vec<String>, String> {
    // Run: wsl -l -q (quiet mode, just names)
    let output = wsl_command()
        .args(["-l", "-q"])
        .output()
        .map_err(|e| format!("Failed to run wsl -l -q: {}", e))?;

    if !output.status.success() {
        return Err("wsl -l -q failed".to_string());
    }

    // WSL output is UTF-16 LE encoded on Windows
    let output_str = String::from_utf16_lossy(
        &output
            .stdout
            .chunks(2)
            .filter_map(|chunk| {
                if chunk.len() == 2 {
                    Some(u16::from_le_bytes([chunk[0], chunk[1]]))
                } else {
                    None
                }
            })
            .collect::<Vec<u16>>(),
    );

    let distros: Vec<String> = output_str
        .lines()
        .map(|s| s.trim().trim_matches('\0').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    debug!("Found WSL distributions: {:?}", distros);
    Ok(distros)
}

/// Find Claude binary path in a WSL distribution
#[cfg(windows)]
fn find_claude_in_wsl(distro: &str) -> Option<String> {
    // Try 'which claude' in the WSL distribution
    let output = wsl_command()
        .args(["-d", distro, "--", "which", "claude"])
        .output()
        .ok()?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() && !path.contains("not found") {
            return Some(path);
        }
    }

    // Try common paths if 'which' doesn't work
    let common_paths = ["/usr/local/bin/claude", "/usr/bin/claude"];

    // Also check NVM paths - first get home directory
    if let Some(home) = get_wsl_home_dir(distro) {
        // Check if there's an NVM installation
        let nvm_base = format!("{}/.nvm/versions/node", home);

        // List node versions and check for claude
        let output = wsl_command()
            .args(["-d", distro, "--", "ls", "-1", &nvm_base])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let versions = String::from_utf8_lossy(&output.stdout);
                for version in versions.lines() {
                    let version = version.trim();
                    if !version.is_empty() {
                        let claude_path = format!("{}/{}/bin/claude", nvm_base, version);
                        // Check if claude exists at this path
                        let check = wsl_command()
                            .args(["-d", distro, "--", "test", "-f", &claude_path])
                            .output();

                        if let Ok(check) = check {
                            if check.status.success() {
                                return Some(claude_path);
                            }
                        }
                    }
                }
            }
        }

        // Check ~/.local/bin/claude
        let local_claude = format!("{}/.local/bin/claude", home);
        let check = wsl_command()
            .args(["-d", distro, "--", "test", "-f", &local_claude])
            .output();

        if let Ok(check) = check {
            if check.status.success() {
                return Some(local_claude);
            }
        }
    }

    // Check common system paths
    for path in common_paths {
        let check = wsl_command()
            .args(["-d", distro, "--", "test", "-f", path])
            .output();

        if let Ok(check) = check {
            if check.status.success() {
                return Some(path.to_string());
            }
        }
    }

    None
}

/// Get home directory in WSL
#[cfg(windows)]
fn get_wsl_home_dir(distro: &str) -> Option<String> {
    let output = wsl_command()
        .args(["-d", distro, "--", "echo", "$HOME"])
        .output()
        .ok()?;

    if output.status.success() {
        let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !home.is_empty() {
            return Some(home);
        }
    }

    None
}

/// Get Claude version in WSL
#[cfg(windows)]
fn get_claude_version_in_wsl(distro: &str, claude_path: &str) -> Option<String> {
    let output = wsl_command()
        .args(["-d", distro, "--", claude_path, "--version"])
        .output()
        .ok()?;

    if output.status.success() {
        extract_version_from_output(&output.stdout)
    } else {
        None
    }
}

/// Get Claude version by running --version command
fn get_claude_version(path: &str) -> Result<Option<String>, String> {
    match Command::new(path).arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                Ok(extract_version_from_output(&output.stdout))
            } else {
                Ok(None)
            }
        }
        Err(e) => {
            warn!("Failed to get version for {}: {}", path, e);
            Ok(None)
        }
    }
}

/// Extract version string from command output
fn extract_version_from_output(stdout: &[u8]) -> Option<String> {
    let output_str = String::from_utf8_lossy(stdout);

    // Debug log the raw output
    debug!("Raw version output: {:?}", output_str);

    // Use regex to directly extract version pattern (e.g., "1.0.41")
    // This pattern matches:
    // - One or more digits, followed by
    // - A dot, followed by
    // - One or more digits, followed by
    // - A dot, followed by
    // - One or more digits
    // - Optionally followed by pre-release/build metadata
    let version_regex =
        regex::Regex::new(r"(\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?(?:\+[a-zA-Z0-9.-]+)?)").ok()?;

    if let Some(captures) = version_regex.captures(&output_str) {
        if let Some(version_match) = captures.get(1) {
            let version = version_match.as_str().to_string();
            debug!("Extracted version: {:?}", version);
            return Some(version);
        }
    }

    debug!("No version found in output");
    None
}

/// Select the best installation based on version
fn select_best_installation(installations: Vec<ClaudeInstallation>) -> Option<ClaudeInstallation> {
    // In production builds, version information may not be retrievable because
    // spawning external processes can be restricted. We therefore no longer
    // discard installations that lack a detected version – the mere presence
    // of a readable binary on disk is enough to consider it valid. We still
    // prefer binaries with version information when it is available so that
    // in development builds we keep the previous behaviour of picking the
    // most recent version.
    installations.into_iter().max_by(|a, b| {
        match (&a.version, &b.version) {
            // If both have versions, compare them semantically.
            (Some(v1), Some(v2)) => compare_versions(v1, v2),
            // Prefer the entry that actually has version information.
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            // Neither have version info: prefer the one that is not just
            // the bare "claude" lookup from PATH, because that may fail
            // at runtime if PATH is modified.
            (None, None) => {
                if a.path == "claude" && b.path != "claude" {
                    Ordering::Less
                } else if a.path != "claude" && b.path == "claude" {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            }
        }
    })
}

/// Compare two version strings
fn compare_versions(a: &str, b: &str) -> Ordering {
    // Simple semantic version comparison
    let a_parts: Vec<u32> = a
        .split('.')
        .filter_map(|s| {
            // Handle versions like "1.0.17-beta" by taking only numeric part
            s.chars()
                .take_while(|c| c.is_numeric())
                .collect::<String>()
                .parse()
                .ok()
        })
        .collect();

    let b_parts: Vec<u32> = b
        .split('.')
        .filter_map(|s| {
            s.chars()
                .take_while(|c| c.is_numeric())
                .collect::<String>()
                .parse()
                .ok()
        })
        .collect();

    // Compare each part
    for i in 0..std::cmp::max(a_parts.len(), b_parts.len()) {
        let a_val = a_parts.get(i).unwrap_or(&0);
        let b_val = b_parts.get(i).unwrap_or(&0);
        match a_val.cmp(b_val) {
            Ordering::Equal => continue,
            other => return other,
        }
    }

    Ordering::Equal
}

/// Helper function to create a Command with proper environment variables
/// This ensures commands like Claude can find Node.js and other dependencies
pub fn create_command_with_env(program: &str) -> Command {
    let mut cmd = Command::new(program);

    info!("Creating command for: {}", program);

    // Inherit essential environment variables from parent process
    for (key, value) in std::env::vars() {
        // Pass through PATH and other essential environment variables
        if key == "PATH"
            || key == "HOME"
            || key == "USER"
            || key == "SHELL"
            || key == "LANG"
            || key == "LC_ALL"
            || key.starts_with("LC_")
            || key == "NODE_PATH"
            || key == "NVM_DIR"
            || key == "NVM_BIN"
            || key == "HOMEBREW_PREFIX"
            || key == "HOMEBREW_CELLAR"
            // Add proxy environment variables (only uppercase)
            || key == "HTTP_PROXY"
            || key == "HTTPS_PROXY"
            || key == "NO_PROXY"
            || key == "ALL_PROXY"
        {
            debug!("Inheriting env var: {}={}", key, value);
            cmd.env(&key, &value);
        }
    }

    // Log proxy-related environment variables for debugging
    info!("Command will use proxy settings:");
    if let Ok(http_proxy) = std::env::var("HTTP_PROXY") {
        info!("  HTTP_PROXY={}", http_proxy);
    }
    if let Ok(https_proxy) = std::env::var("HTTPS_PROXY") {
        info!("  HTTPS_PROXY={}", https_proxy);
    }

    // Add NVM support if the program is in an NVM directory
    if program.contains("/.nvm/versions/node/") {
        if let Some(node_bin_dir) = std::path::Path::new(program).parent() {
            // Ensure the Node.js bin directory is in PATH
            let current_path = std::env::var("PATH").unwrap_or_default();
            let node_bin_str = node_bin_dir.to_string_lossy();
            if !current_path.contains(&node_bin_str.as_ref()) {
                let new_path = format!("{}:{}", node_bin_str, current_path);
                debug!("Adding NVM bin directory to PATH: {}", node_bin_str);
                cmd.env("PATH", new_path);
            }
        }
    }

    // Add Homebrew support if the program is in a Homebrew directory
    if program.contains("/homebrew/") || program.contains("/opt/homebrew/") {
        if let Some(program_dir) = std::path::Path::new(program).parent() {
            // Ensure the Homebrew bin directory is in PATH
            let current_path = std::env::var("PATH").unwrap_or_default();
            let homebrew_bin_str = program_dir.to_string_lossy();
            if !current_path.contains(&homebrew_bin_str.as_ref()) {
                let new_path = format!("{}:{}", homebrew_bin_str, current_path);
                debug!(
                    "Adding Homebrew bin directory to PATH: {}",
                    homebrew_bin_str
                );
                cmd.env("PATH", new_path);
            }
        }
    }

    cmd
}
