# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-10

### Added

- DOOM engine (doomgeneric) compiled into a Rust MCP server via FFI
- Three MCP tools: `doom_start`, `doom_action`, `doom_screenshot`
- Two play modes: user-directed and AI autonomous
- Virtual time system for deterministic tick-by-tick gameplay
- Enemy detection with line-of-sight checks (no wallhack cheating)
- Nearby item detection (health, ammo, armor, weapons) filtered by need
- Recent enemy memory (enemies don't "vanish" for 3 turns after losing sight)
- Game event hints (kill milestones, heavy damage, critical HP, death, weapon/armor pickups)
- 64-color palette PNG rendering for low token usage (~1KB per frame)
- Full-resolution 320x200 PNG screenshots saved to file and opened in system viewer
- Cross-platform support: Linux x64/ARM64, macOS x64/ARM64, Windows x64
- npm package with pre-built binaries and bundled Freedoom WAD
- GitHub Actions CI pipeline for building all platforms and publishing to npm
- Debug logging to `/tmp/doom-mcp.log` via `DOOM_MCP_DEBUG=1`
- Custom WAD support via `DOOM_WAD_PATH` environment variable
- Input validation with warnings for unknown actions
- SHA256 checksum verification for downloaded Freedoom WAD
- Pinned doomgeneric dependency to known-good commit
- Unit tests for action key mapping
- Integration tests for MCP protocol (initialize, tools/list)
- Comprehensive README with FAQ, architecture overview, and configuration guide
