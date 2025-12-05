//! Shell environment Tauri commands
//!
//! These commands allow the frontend to:
//! - Detect available shell environments (Native, WSL, Git Bash)
//! - Get/set the preferred shell environment
//! - Check if Claude is available in WSL

use crate::shell_environment::{
    check_claude_in_wsl, detect_available_shells, AvailableShells, ShellConfig, ShellEnvironment,
};
use log::{info, warn};
use tauri::Manager;

/// Get available shell environments on the current system
#[tauri::command]
pub async fn get_available_shells() -> Result<AvailableShells, String> {
    info!("Getting available shell environments");
    Ok(detect_available_shells())
}

/// Get the current shell configuration
#[tauri::command]
pub async fn get_shell_config(app: tauri::AppHandle) -> Result<ShellConfig, String> {
    info!("Getting shell configuration");

    if let Ok(app_data_dir) = app.path().app_data_dir() {
        let db_path = app_data_dir.join("agents.db");
        if db_path.exists() {
            if let Ok(conn) = rusqlite::Connection::open(&db_path) {
                // Get shell environment preference
                let environment = conn
                    .query_row(
                        "SELECT value FROM app_settings WHERE key = 'shell_environment'",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_default();

                // Get WSL distribution preference
                let wsl_distro = conn
                    .query_row(
                        "SELECT value FROM app_settings WHERE key = 'wsl_distro'",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .ok();

                // Get WSL Claude path
                let wsl_claude_path = conn
                    .query_row(
                        "SELECT value FROM app_settings WHERE key = 'wsl_claude_path'",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .ok();

                // Get Git Bash path
                let git_bash_path = conn
                    .query_row(
                        "SELECT value FROM app_settings WHERE key = 'git_bash_path'",
                        [],
                        |row| row.get::<_, String>(0),
                    )
                    .ok();

                return Ok(ShellConfig {
                    environment,
                    wsl_distro,
                    wsl_claude_path,
                    git_bash_path,
                });
            }
        }
    }

    // Return default config if no database or settings found
    Ok(ShellConfig::default())
}

/// Save the shell configuration
#[tauri::command]
pub async fn save_shell_config(app: tauri::AppHandle, config: ShellConfig) -> Result<(), String> {
    info!("Saving shell configuration: {:?}", config);

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let db_path = app_data_dir.join("agents.db");
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    // Ensure app_settings table exists
    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("Failed to create settings table: {}", e))?;

    // Save shell environment
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('shell_environment', ?)",
        [config.environment.to_string()],
    )
    .map_err(|e| format!("Failed to save shell_environment: {}", e))?;

    // Save WSL distribution (if set)
    if let Some(ref distro) = config.wsl_distro {
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('wsl_distro', ?)",
            [distro],
        )
        .map_err(|e| format!("Failed to save wsl_distro: {}", e))?;
    } else {
        conn.execute("DELETE FROM app_settings WHERE key = 'wsl_distro'", [])
            .ok();
    }

    // Save WSL Claude path (if set)
    if let Some(ref path) = config.wsl_claude_path {
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('wsl_claude_path', ?)",
            [path],
        )
        .map_err(|e| format!("Failed to save wsl_claude_path: {}", e))?;
    } else {
        conn.execute("DELETE FROM app_settings WHERE key = 'wsl_claude_path'", [])
            .ok();
    }

    // Save Git Bash path (if set)
    if let Some(ref path) = config.git_bash_path {
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('git_bash_path', ?)",
            [path],
        )
        .map_err(|e| format!("Failed to save git_bash_path: {}", e))?;
    } else {
        conn.execute("DELETE FROM app_settings WHERE key = 'git_bash_path'", [])
            .ok();
    }

    info!("Shell configuration saved successfully");
    Ok(())
}

/// Check if Claude is available in WSL and return the path
#[tauri::command]
pub async fn check_wsl_claude(distro: Option<String>) -> Result<Option<String>, String> {
    info!("Checking for Claude in WSL (distro: {:?})", distro);
    Ok(check_claude_in_wsl(distro.as_deref()))
}

/// Detect Claude installation in WSL and auto-configure if found
#[tauri::command]
pub async fn auto_detect_wsl_claude(
    app: tauri::AppHandle,
    distro: Option<String>,
) -> Result<Option<ShellConfig>, String> {
    info!("Auto-detecting Claude in WSL");

    // First check available shells
    let shells = detect_available_shells();

    // If no WSL distributions, return None
    if shells.wsl_distributions.is_empty() {
        info!("No WSL distributions found");
        return Ok(None);
    }

    // Determine which distro to check
    let target_distro = distro.or_else(|| {
        shells
            .wsl_distributions
            .iter()
            .find(|d| d.is_default)
            .or(shells.wsl_distributions.first())
            .map(|d| d.name.clone())
    });

    // Check for Claude in the target distro
    if let Some(ref distro_name) = target_distro {
        if let Some(claude_path) = check_claude_in_wsl(Some(distro_name)) {
            info!(
                "Found Claude at {} in WSL distro {}",
                claude_path, distro_name
            );

            let config = ShellConfig {
                environment: ShellEnvironment::Wsl,
                wsl_distro: Some(distro_name.clone()),
                wsl_claude_path: Some(claude_path),
                git_bash_path: shells.git_bash_path,
            };

            // Save the configuration
            save_shell_config(app, config.clone()).await?;

            return Ok(Some(config));
        }
    }

    warn!("Claude not found in any WSL distribution");
    Ok(None)
}
