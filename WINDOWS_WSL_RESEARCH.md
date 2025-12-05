# Windows & WSL Support Research for Opcode (Claudia)

## Summary

This document summarizes the research findings from winfunc/opcode GitHub issues, PRs, and community forks regarding:
1. Windows build availability
2. Claude installation detection on Windows
3. WSL integration for Claude Code execution

---

## 1. Windows Build Availability

### Current State
- **No official Windows builds in releases** - Major pain point for users
- Multiple issues requesting Windows builds: #301, #332, #370, #219, #369

### Community Solutions

#### Issue #370 - Chinese user provided Windows 0.20 build
- Fixed compilation and installation failure
- Fixed local Claude code recognition
- Provided both `.exe` and setup installer
- Status: Open (community contribution, not merged)

#### Issue #78 - Comprehensive Community Fix v4.2 by @Kirchlive
**Gist:** https://gist.github.com/Kirchlive/184cdd96a56bfd7a6c67997836495f3c

Files included:
- `2-claude_binary_patch.rs` - Patched main.rs accepting any Claude version
- `3-setup-windows.bat` - Creates WSL bridge script
- `4-start-claudia.bat` - Launcher with pre-flight checks
- `5-apply-patches.bat` - Build script patches

Key features:
- WSL bridge via `claude.bat` in `%APPDATA%\npm`
- Automatic Claude CLI version detection from WSL
- Filters unsupported parameters (`--system-prompt`, `--no-color`)
- Supports multiple WSL distributions

#### Issue #352 - AppImage on WSL2 Workaround by @mf
Run the Linux AppImage release directly in WSL2:
```bash
apt install libgles2
# Extract and run AppImage
./AppImage --appimage-extract
```

---

## 2. Claude Installation Detection on Windows

### Merged Fix: PR #367 by @Asm3r96
**Status: MERGED into main**

Added Windows support via conditional compilation:
- `#[cfg(windows)]` for `try_which_command()` using `where` instead of `which`
- `#[cfg(windows)]` for `find_nvm_installations()` checking `NVM_HOME`
- `#[cfg(windows)]` for `find_standard_installations()` with Windows paths

Windows paths checked:
- `%USERPROFILE%\.claude\local\claude.exe`
- `%USERPROFILE%\.local\bin\claude.exe`
- `%APPDATA%\Roaming\npm\claude.cmd`
- `%USERPROFILE%\.yarn\bin\claude.cmd`
- `%USERPROFILE%\.bun\bin\claude.exe`

### Related Open PRs/Issues

#### PR #360 - Windows compatibility (by @hexbee)
Additional improvements:
- Replace Unix `which` with Windows `where`
- Support Windows environment variables (`USERPROFILE` vs `HOME`)
- Handle `.cmd`, `.exe` extensions
- Detect Program Files directories

#### PR #348 - Improve Claude installation detection (by @ollieb89)
Focus on better detection logic for Windows

#### Issue #369 - Build Error on Windows 11
Missing `installation_type` field in `ClaudeInstallation` struct
**Fixed by PR #374** (merged)

---

## 3. WSL Integration

### The Core Problem (Issue #168)
> "Claude code can only be installed on Windows via WSL... how can I run Claude code successfully in the Claudia desktop?"

### Community WSL Bridge Solution (from Issue #78)

The `claude.bat` bridge script:
```batch
@echo off
setlocal enabledelayedexpansion

REM Configuration - set your WSL distro
SET "WSL_DISTRO="

REM Version check - get real version from WSL claude
if /I "%~1" == "--version" (
    for /f "delims=" %%i in ('wsl bash -lc "~/.npm-global/bin/claude --version"') do set "CLAUDE_VERSION=%%i"
    echo !CLAUDE_VERSION!
    exit /b 0
)

REM Filter unsupported arguments
REM Skip: --system-prompt, --no-color

REM Execute via WSL
if defined WSL_DISTRO (
    wsl -d "%WSL_DISTRO%" bash -lc "~/.npm-global/bin/claude !CMD!"
) else (
    wsl bash -lc "~/.npm-global/bin/claude !CMD!"
)
```

### Issue #137 - Official WSL Support Request
> "Currently there is a community fix... but we just can't get it to work because of the number of steps involved. Is there a plan for official WSL support?"

**Status: Open, no official response**

### Issue #186 - Windows Shell Support PR by @alexiokay
**Key improvements proposed:**
- New `shell_environment.rs` module for Windows shell detection
- Automatic detection of Git Bash, WSL, PowerShell
- Proper environment setup for Claude Code on Windows
- Better error messages with instructions

---

## 4. Feature Request: Custom Command & Environment Variables

### Issue #400 - Support for Custom Claude Execution Command
Use case: Integration with [claude-code-router](https://github.com/musistudio/claude-code-router)

**Proposed solutions:**

#### Option 1: Custom Claude Binary Path (Recommended)
```
Settings → General → Claude Code Command
[ccr code] (default: claude)
```

#### Option 2: Custom Environment Variables
```
Settings → Advanced → Custom Environment Variables
ANTHROPIC_BASE_URL=https://my-proxy.com/
CLAUDE_CODE_MAX_OUTPUT_TOKENS=20000
```

#### Option 3: Whitelist Configuration
Allow users to extend the environment variable whitelist via config file.

---

## 5. Active Forks with Recent Changes

| Fork | Last Updated | Notable Changes |
|------|--------------|-----------------|
| namastexlabs/opcode | Dec 4, 2025 | Tauri event listener fixes |
| Rixmerz/nova | Dec 5, 2025 | Rebranded, purple theme, haiku model, unified ClaudeOptions API |
| tbarstow-tw/opcode-pm | Dec 4, 2025 | BMAD framework documentation |
| DigitalNomad-Chat/opcode | Dec 4, 2025 | Active development |
| zhu976/opcode-i18n | Dec 2, 2025 | Chinese i18n support |

---

## 6. Recommended Implementation Path

### For Windows Builds
1. Add GitHub Actions workflow for Windows builds (requested in #194, #226)
2. Include the merged Windows detection code from PR #367
3. Fix icon format issues (#75, #276, #262)

### For WSL Integration
1. Implement shell environment detection (from PR #186)
2. Add settings UI for:
   - Shell preference (PowerShell / Git Bash / WSL)
   - WSL distribution selection
   - Custom Claude command path
3. Create WSL bridge in Rust (not batch file) for better integration

### For Multiple Claude Installations
1. Already implemented: `list_claude_installations()` API endpoint
2. Add UI dropdown in settings to select preferred installation
3. Store preference in `app_settings` table

---

## 7. Key Files to Reference

### In winfunc/opcode main branch:
- `src-tauri/src/claude_binary.rs` - Windows detection already merged
- `src-tauri/src/commands/claude.rs` - Command execution (needs WSL support)

### Community Gist (v4.2):
- https://gist.github.com/Kirchlive/184cdd96a56bfd7a6c67997836495f3c

### Related PRs to watch:
- #186 - Shell environment detection
- #360 - Additional Windows compatibility
- #400 - Custom command/env vars
- #407 - Tauri event listener fixes (namastexlabs)
