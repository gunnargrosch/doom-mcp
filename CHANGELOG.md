# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-03-11

### Added

- Door and switch detection: nearby usable linedefs (doors, locked doors, exit switches) are reported as `NEARBY: Door AHEAD (use to activate)` so the AI knows when to press `use`
- Stuck detection: warns after 2+ consecutive movement ticks with no position change (`STUCK — try: turn, strafe, or use on nearby walls`)
- Point-blank danger warning: enemies within melee range are flagged with `⚠ TOO CLOSE — strafe and back away!`
- Stale fire detection: warns when the most-ahead enemy takes no damage after 3+ consecutive fire ticks, suggesting the enemy may be behind cover
- Low-HP escape hint: at critical HP the server now suggests `strafe_left/strafe_right,backward,run` rather than turning in place

### Improved

- Directions replaced raw degree values with natural language: `slightly to your left`, `to your right`, `hard left`, `behind you to the right`, etc. Turn hints remain in parentheses for AI navigation
- Distances replaced raw map units with bucketed labels: `point-blank`, `close`, `nearby`, `far`, `very far`
- RECEDING items are auto-suppressed after 2 consecutive ticks of receding — unreachable items no longer distract the AI
- Thumbnail upgraded from 160×100 (64-color) to 200×125 (216-color web-safe palette) for significantly better color fidelity with modest token increase

## [0.1.1] - 2026-03-11

### Fixed

- Windows build: platform.c now compiles on MSVC (added `#ifdef _WIN32` for timing functions)
- Windows build: `libc::write` count parameter uses platform-correct type
- Screenshot path uses system temp directory instead of hardcoded `/tmp/` (fixes Windows)
- Screenshot viewer supports Windows (`cmd /c start`)
- CI pipeline: create `npm/engine/` directory before copying binaries
- CI pipeline: copy README/LICENSE/CHANGELOG to npm package before publish
- Tool schema: `ticks` maximum now correctly shows 105 (was 35)

### Improved

- AI vision thumbnails increased from 80x50 to 160x100 (~2KB per frame) for much better scene recognition
- Item tracking shows CLOSING/RECEDING distance changes to help AI navigate to pickups
- Items filtered by field of view (±45°) — only items visible on screen are reported
- Turn hints added to item/enemy output (e.g. `turn_left ~11 ticks`) for precise aiming
- Reach hints added to item output (e.g. `~18 ticks fwd+run to reach`) to prevent overshooting
- `doom_start` now restarts an in-progress game in-place using DOOM's native new-game mechanism — no need to start a new session to play again

### Removed

- Dead `doom_look` tool handler (was not advertised in tools/list)

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
