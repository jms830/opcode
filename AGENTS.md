# Agent Guidelines for Opcode

## Build & Dev Commands
- `bun install` - Install dependencies
- `bun run dev` - Start Vite dev server (frontend only)
- `bun run tauri dev` - Full Tauri development mode
- `bun run build` - Build frontend (`tsc && vite build`)
- `bun run tauri build` - Production build
- `bun run check` - TypeScript + Rust check (`tsc --noEmit && cargo check`)
- `cd src-tauri && cargo test` - Run Rust tests
- `cd src-tauri && cargo fmt && cargo clippy` - Format and lint Rust

## Code Style
- **TypeScript**: Strict mode, no unused vars/params, ES2020 target, React JSX
- **Rust**: Edition 2021, use `cargo fmt` before commits, address `clippy` warnings
- **Imports**: Use `@/*` path alias for `./src/*` in TypeScript
- **Naming**: camelCase (TS), snake_case (Rust), PascalCase for types/components
- **Errors**: Use `Result<T, String>` in Rust commands, try/catch in TS async functions

## Architecture
- Frontend: React + TypeScript + Vite in `src/`
- Backend: Tauri + Rust in `src-tauri/src/`
- Tauri commands in `src-tauri/src/commands/` expose Rust to frontend via `invoke()`
- Use `#[cfg(windows)]` / `#[cfg(unix)]` for platform-specific code
