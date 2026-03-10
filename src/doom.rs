use crate::doom_debug as debug;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};

pub const SCREEN_WIDTH: u32 = 320;
pub const SCREEN_HEIGHT: u32 = 200;
pub const MAX_ENEMIES: usize = 16;
pub const MAX_ITEMS: usize = 16;
pub const MAX_TICKS: i32 = 105;

#[repr(C)]
#[derive(Debug, Default)]
pub struct GameState {
    pub health: i32,
    pub armor: i32,
    pub ammo_bullets: i32,
    pub ammo_shells: i32,
    pub ammo_cells: i32,
    pub ammo_rockets: i32,
    pub weapon: i32,
    pub kills: i32,
    pub items: i32,
    pub secrets: i32,
    pub x: i32,
    pub y: i32,
    pub angle: u32,
    pub episode: i32,
    pub map: i32,
}

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>,
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct ItemInfo {
    pub item_type: i32,
    pub distance: i32,
    pub angle: i32,
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct EnemyInfo {
    pub enemy_type: i32,
    pub health: i32,
    pub distance: i32,
    pub angle: i32,     // degrees relative to player: 0=ahead, +left, -right
    pub visible: i32,
}

extern "C" {
    fn doomgeneric_Create(argc: c_int, argv: *mut *mut c_char);
    fn doomgeneric_Tick();
    static DG_ScreenBuffer: *mut u32;

    fn mcp_set_key(key: u8, pressed: c_int);
    fn mcp_clear_keys();
    fn mcp_get_game_state(state: *mut GameState);
    fn mcp_enable_virtual_time();
    fn mcp_advance_tick();
    fn mcp_get_enemies(enemies: *mut EnemyInfo, max_count: c_int) -> c_int;
    fn mcp_get_items(items: *mut ItemInfo, max_count: c_int) -> c_int;
}

pub struct Engine {
    _initialized: bool,
}

impl Engine {
    pub fn new(skill: Option<i32>, episode: Option<i32>, map: Option<i32>) -> Result<Self, String> {
        let wad = find_wad()?;

        let mut args: Vec<String> = vec![
            "doom-mcp".into(),
            "-iwad".into(),
            wad,
            "-nosound".into(),
        ];

        if let Some(s) = skill {
            args.push("-skill".into());
            args.push(s.to_string());
        }

        if let (Some(ep), Some(m)) = (episode, map) {
            args.push("-warp".into());
            args.push(ep.to_string());
            args.push(m.to_string());
        }

        let c_strings: Vec<CString> = args
            .iter()
            .map(|a| CString::new(a.as_str()).unwrap())
            .collect();
        let mut c_argv: Vec<*mut c_char> = c_strings
            .iter()
            .map(|cs| cs.as_ptr() as *mut c_char)
            .collect();

        debug!("engine: doomgeneric_Create args={:?}", args);
        unsafe {
            doomgeneric_Create(c_argv.len() as c_int, c_argv.as_mut_ptr());
        }
        debug!("engine: doomgeneric_Create returned");

        // Switch to virtual time so each tick = exactly one game tic
        unsafe { mcp_enable_virtual_time(); }
        debug!("engine: virtual time enabled");

        Ok(Engine {
            _initialized: true,
        })
    }

    pub fn tick(&mut self, ticks: i32, actions: &[&str]) {
        let clamped = ticks.clamp(1, MAX_TICKS);

        // Deduplicate and collect unique actions
        let mut seen_keys: Vec<u8> = Vec::new();
        let mut action_names: Vec<&str> = Vec::new();
        let mut invalid_actions: Vec<&str> = Vec::new();
        for action in actions {
            let trimmed = action.trim();
            if trimmed.is_empty() { continue; }
            if let Some(key) = action_to_key(trimmed) {
                if !seen_keys.contains(&key) {
                    seen_keys.push(key);
                    action_names.push(trimmed);
                }
            } else {
                invalid_actions.push(trimmed);
            }
        }
        if !invalid_actions.is_empty() {
            debug!("engine: unknown actions ignored: {:?}", invalid_actions);
        }

        debug!("engine: actions={:?} ticks={}", action_names, clamped);

        unsafe {
            mcp_clear_keys();
            for &key in &seen_keys {
                mcp_set_key(key, 1);
            }
            debug!("engine: running {} ticks", clamped);
            for _ in 0..clamped {
                mcp_advance_tick();
                doomgeneric_Tick();
            }
            // Don't clear keys at end - let them stay held for refire mechanics
        }

        let state = self.get_state();
        debug!(
            "engine: after tick HP={} pos=({},{}) angle={} E{}M{}",
            state.health, state.x, state.y, state.angle, state.episode, state.map
        );
    }

    pub fn get_frame(&self) -> Frame {
        unsafe {
            let buffer = std::slice::from_raw_parts(
                DG_ScreenBuffer,
                (SCREEN_WIDTH * SCREEN_HEIGHT) as usize,
            );

            Frame {
                width: SCREEN_WIDTH,
                height: SCREEN_HEIGHT,
                pixels: buffer.to_vec(),
            }
        }
    }

    pub fn get_state(&self) -> GameState {
        let mut state = GameState::default();
        unsafe {
            mcp_get_game_state(&mut state);
        }
        state
    }

    pub fn get_enemies(&self) -> Vec<EnemyInfo> {
        let mut enemies = vec![EnemyInfo::default(); MAX_ENEMIES];
        let count = unsafe { mcp_get_enemies(enemies.as_mut_ptr(), MAX_ENEMIES as c_int) };
        enemies.truncate(count as usize);
        enemies
    }

    pub fn get_items(&self) -> Vec<ItemInfo> {
        let mut items = vec![ItemInfo::default(); MAX_ITEMS];
        let count = unsafe { mcp_get_items(items.as_mut_ptr(), MAX_ITEMS as c_int) };
        items.truncate(count as usize);
        items
    }

    pub fn validate_actions(actions: &[&str]) -> Vec<String> {
        let mut warnings = Vec::new();
        for action in actions {
            let trimmed = action.trim();
            if !trimmed.is_empty() && action_to_key(trimmed).is_none() {
                warnings.push(format!("Unknown action '{}'. Valid: forward, backward, turn_left, turn_right, strafe_left, strafe_right, fire, use, run, 1-7", trimmed));
            }
        }
        warnings
    }
}

fn action_to_key(action: &str) -> Option<u8> {
    match action {
        "forward" | "up" => Some(0xad),          // KEY_UPARROW (doomkeys.h)
        "backward" | "down" => Some(0xaf),       // KEY_DOWNARROW (doomkeys.h)
        "left" | "turn_left" => Some(0xac),      // KEY_LEFTARROW (doomkeys.h)
        "right" | "turn_right" => Some(0xae),    // KEY_RIGHTARROW (doomkeys.h)
        "fire" => Some(0xa3),                    // KEY_FIRE (doomkeys.h)
        "use" | "open" => Some(0xa2),            // KEY_USE (doomkeys.h)
        "strafe_left" => Some(0xa0),             // KEY_STRAFE_L (doomkeys.h)
        "strafe_right" => Some(0xa1),            // KEY_STRAFE_R (doomkeys.h)
        "run" => Some(0x80 + 0x36),              // KEY_RSHIFT (doomkeys.h)
        "enter" => Some(13),                     // KEY_ENTER
        "escape" | "esc" => Some(27),            // KEY_ESCAPE
        "tab" | "map" => Some(9),                // KEY_TAB
        "y" => Some(b'y'),
        "n" => Some(b'n'),
        "1" => Some(b'1'),
        "2" => Some(b'2'),
        "3" => Some(b'3'),
        "4" => Some(b'4'),
        "5" => Some(b'5'),
        "6" => Some(b'6'),
        "7" => Some(b'7'),
        _ => None,
    }
}

fn find_wad() -> Result<String, String> {
    debug!("find_wad: searching...");
    match crate::paths::find_wad() {
        Some(path) => {
            debug!("find_wad: found {}", path);
            Ok(path)
        }
        None => Err("WAD file not found. Run 'bash scripts/setup.sh' or set DOOM_WAD_PATH.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_to_key() {
        assert_eq!(action_to_key("forward"), Some(0xad));
        assert_eq!(action_to_key("fire"), Some(0xa3));
        assert_eq!(action_to_key("use"), Some(0xa2));
        assert_eq!(action_to_key("run"), Some(0xb6));
        assert_eq!(action_to_key("1"), Some(b'1'));
        assert_eq!(action_to_key("invalid"), None);
        assert_eq!(action_to_key(""), None);
    }
}
