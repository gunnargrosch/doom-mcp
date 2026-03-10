use std::path::Path;

/// Build script for doom-mcp: compiles the doomgeneric C engine into a static library.
///
/// # Why a whitelist approach?
///
/// The doomgeneric repository contains the portable DOOM engine alongside many
/// platform-specific backends (SDL, Allegro, Xlib, Emscripten, Windows, Linux VT, etc.).
/// Each of these backends provides implementations of the DG_* interface functions
/// (DG_Init, DG_DrawFrame, DG_GetTicksMs, etc.) that tie the engine to a particular
/// display/input/audio system.
///
/// We cannot compile all .c files because:
///   1. Multiple backends would define the same DG_* symbols, causing linker conflicts.
///   2. Platform backends pull in external library dependencies (SDL2, X11, Allegro)
///      that we don't need and may not be available.
///   3. Sound/music files (i_sdlsound.c, i_sdlmusic.c, i_allegrosound.c, etc.) depend
///      on audio libraries we don't use -- this is a headless MCP server.
///
/// Instead, we explicitly list only the core engine files that implement the
/// platform-independent DOOM game logic, and provide our own platform layer.
///
/// # Excluded files (and why)
///
/// Platform backends (each provides DG_* functions for a specific platform):
///   - doomgeneric_sdl.c       -- SDL2 display/input (requires libSDL2)
///   - doomgeneric_allegro.c   -- Allegro display/input (requires Allegro)
///   - doomgeneric_xlib.c      -- X11/Xlib display/input (requires libX11)
///   - doomgeneric_win.c       -- Windows GDI display/input
///   - doomgeneric_emscripten.c -- Emscripten/WebAssembly backend
///   - doomgeneric_linuxvt.c   -- Linux virtual terminal backend
///   - doomgeneric_soso.c      -- SOSO OS backend
///   - doomgeneric_sosox.c     -- SOSO OS X backend
///
/// Sound/music (depend on SDL_mixer, Allegro audio, or CD-ROM):
///   - i_sdlsound.c            -- SDL2 sound effects
///   - i_sdlmusic.c            -- SDL2 music playback
///   - i_allegrosound.c        -- Allegro sound effects
///   - i_allegromusic.c        -- Allegro music playback
///   - i_cdmus.c               -- CD-ROM music playback
///   - i_sound.c               -- Sound system dispatcher
///
/// System/timer (we provide our own implementations via platform.c):
///   - i_system.c              -- System interface (exit, alloc, timer functions)
///   - i_timer.c               -- Real-time timer (we use virtual time instead)
///   - i_joystick.c            -- Joystick input (not needed for MCP control)
///
/// Other:
///   - w_main.c                -- WAD file main (not used in doomgeneric builds)
///
/// # Our platform layer (csrc/platform.c)
///
/// Instead of any upstream backend, we compile csrc/platform.c which provides:
///   - DG_* function implementations for headless/MCP operation
///   - Virtual time control (mcp_enable_virtual_time / mcp_advance_tick)
///   - Key input injection (mcp_set_key / mcp_clear_keys)
///   - Game state extraction (mcp_get_game_state, mcp_get_enemies, mcp_get_items)
///   - Stubs for sound, music, joystick, and other I/O we don't need
///
/// # About doomgeneric
///
/// doomgeneric (https://github.com/ozkl/doomgeneric) is a portable DOOM engine
/// derived from Chocolate Doom. It factors out all platform-specific code behind
/// a small DG_* interface, making it straightforward to embed DOOM into any
/// environment by providing ~6 callback functions. This project uses that
/// interface to run DOOM headlessly, controlled via the Model Context Protocol.
fn main() {
    let dg_src = Path::new("engine/doomgeneric/doomgeneric");

    if !dg_src.exists() {
        println!(
            "cargo:warning=doomgeneric source not found at {:?}. Run: bash scripts/setup.sh",
            dg_src
        );
        return;
    }

    let mut build = cc::Build::new();

    // Whitelist: only the core, platform-independent engine files.
    // See the module-level documentation above for why we use a whitelist
    // and which files are excluded.
    let engine_files = [
        "am_map.c",
        "d_event.c",
        "d_items.c",
        "d_iwad.c",
        "d_loop.c",
        "d_main.c",
        "d_mode.c",
        "d_net.c",
        "doomdef.c",
        "doomgeneric.c",
        "doomstat.c",
        "dstrings.c",
        "dummy.c",
        "f_finale.c",
        "f_wipe.c",
        "g_game.c",
        "gusconf.c",
        "hu_lib.c",
        "hu_stuff.c",
        "i_endoom.c",
        "i_input.c",
        "i_scale.c",
        "i_video.c",
        "icon.c",
        "info.c",
        "m_argv.c",
        "m_bbox.c",
        "m_cheat.c",
        "m_config.c",
        "m_controls.c",
        "m_fixed.c",
        "m_menu.c",
        "m_misc.c",
        "m_random.c",
        "memio.c",
        "mus2mid.c",
        "p_ceilng.c",
        "p_doors.c",
        "p_enemy.c",
        "p_floor.c",
        "p_inter.c",
        "p_lights.c",
        "p_map.c",
        "p_maputl.c",
        "p_mobj.c",
        "p_plats.c",
        "p_pspr.c",
        "p_saveg.c",
        "p_setup.c",
        "p_sight.c",
        "p_spec.c",
        "p_switch.c",
        "p_telept.c",
        "p_tick.c",
        "p_user.c",
        "r_bsp.c",
        "r_data.c",
        "r_draw.c",
        "r_main.c",
        "r_plane.c",
        "r_segs.c",
        "r_sky.c",
        "r_things.c",
        "s_sound.c",
        "sha1.c",
        "sounds.c",
        "st_lib.c",
        "st_stuff.c",
        "statdump.c",
        "tables.c",
        "v_video.c",
        "w_checksum.c",
        "w_file.c",
        "w_file_stdc.c",
        "w_wad.c",
        "wi_stuff.c",
        "z_zone.c",
    ];

    for file in &engine_files {
        build.file(dg_src.join(file));
    }

    // Our platform implementation (DG_ functions + stubs)
    build.file("csrc/platform.c");

    build.define("DOOMGENERIC_RESX", "320");
    build.define("DOOMGENERIC_RESY", "200");
    build.include(dg_src);
    build.opt_level(2);
    build.warnings(false);

    build.compile("doomengine");

    println!("cargo:rerun-if-changed=csrc/platform.c");
    println!("cargo:rerun-if-changed=build.rs");
}
