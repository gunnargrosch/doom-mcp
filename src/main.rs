mod doom;
mod log;
mod paths;
mod renderer;
use crate::doom_debug as debug;
use serde_json::{json, Value};
use std::io::{self, BufRead};

extern crate libc;

/// Write a JSON-RPC response directly to fd 1 (stdout) using libc::write.
///
/// We bypass Rust's std::io::stdout() because it was observed to duplicate
/// output to both stdout and stderr in some environments, which breaks the
/// MCP protocol (clients read from stdout only). Using raw libc::write to
/// fd 1 ensures output goes exclusively to stdout.
fn send(msg: &str) {
    debug!(">> {}", if msg.len() > 200 { &msg[..200] } else { msg });
    let bytes = msg.as_bytes();
    let mut off = 0;
    while off < bytes.len() {
        let n = unsafe {
            libc::write(1, bytes[off..].as_ptr() as *const libc::c_void, (bytes.len() - off) as _)
        };
        if n <= 0 { break; }
        off += n as usize;
    }
    unsafe { libc::write(1, b"\n".as_ptr() as *const libc::c_void, 1 as _); }
}

fn main() {
    debug!("doom-mcp starting (debug={})", log::is_debug());

    let stdin = io::stdin();
    let mut engine: Option<doom::Engine> = None;
    let mut tracker = GameTracker::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        debug!("<< {}", if line.len() > 200 { &line[..200] } else { &line });

        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                debug!("parse error: {e}");
                continue;
            }
        };

        let method = msg["method"].as_str().unwrap_or("");
        let id = msg.get("id").cloned();

        // Notifications (no id) - don't send a response
        if id.is_none() {
            if method == "notifications/initialized" {
                // eprintln!("Client initialized");
            }
            continue;
        }

        let id = id.unwrap();
        let params = msg.get("params").cloned().unwrap_or(json!({}));

        let response = match method {
            "initialize" => handle_initialize(&params),
            "tools/list" => handle_tools_list(),
            "tools/call" => handle_tool_call(&params, &mut engine, &mut tracker),
            "ping" => json!({}),
            _ => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Method not found: {method}")
                    }
                });
                send(&serde_json::to_string(&err_resp).unwrap());
                continue;
            }
        };

        let full = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": response
        });

        send(&serde_json::to_string(&full).unwrap());
    }

}

// --- MCP Protocol Handlers ---

fn handle_initialize(params: &Value) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("2024-11-05");

    json!({
        "protocolVersion": version,
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "doom-mcp",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn handle_tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "doom_start",
                "description": "Start or restart DOOM. Use this whenever someone says 'play doom', 'new game', 'restart', 'start over', etc. Safe to call even if a game is already running — it will restart cleanly.\n\nBefore calling, ask the user:\n1. 'I direct' - user gives commands, you execute with doom_action\n2. 'You play' - you play autonomously with doom_action\n\nAfter starting, use doom_action to play. Describe what happens each turn.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "skill": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 5,
                            "description": "Difficulty: 1=baby, 2=easy, 3=medium, 4=hard, 5=nightmare"
                        },
                        "episode": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 4,
                            "description": "Episode (1-4)"
                        },
                        "map": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 9,
                            "description": "Map (1-9)"
                        },
                        "width": {
                            "type": "integer",
                            "minimum": 40,
                            "maximum": 200,
                            "description": "ANSI render width in columns (default: 100)"
                        }
                    }
                }
            },
            {
                "name": "doom_action",
                "description": "Perform actions in DOOM. All actions held simultaneously for the tick duration.\n\nActions: forward, backward, turn_left, turn_right, strafe_left, strafe_right, fire, use, run, 1-7\n\nFIRE: 'fire' once = hold fire. Pistol auto-fires every ~10 ticks. Do NOT repeat 'fire'.\n\nRULES:\n1. ONLY fire when enemies in sight and angle near 0. NEVER combine turn+fire.\n2. To reach an item/enemy: turn to face it (get angle to ~0) in ONE action, then 'forward,run' in the NEXT. Don't spiral - 2 actions max.\n3. RECEDING items are auto-hidden after 2 ticks — if an item disappears, it was unreachable. Don't chase it.\n4. Doors and switches appear as NEARBY — press 'use' while facing them to open/activate.\n5. In 'I direct' mode: ONE action, describe surroundings vividly, then WAIT.\n6. Use big ticks for movement (20-35). Small ticks (2-5) for precise aiming only.\n7. Items only shown when player needs them (health items hidden at full HP).\n8. SAFETY: Never advance within close range of a healthy Imp or Pinky Demon — they deal massive melee damage. Stay at nearby range or farther and strafe.\n9. ESCAPE: When low HP, use 'strafe_left,backward,run' or 'strafe_right,backward,run' — do NOT turn in place while retreating.\n10. If an enemy is not taking damage after 3+ shots, it may be behind cover — reposition rather than wasting ammo.",
                "inputSchema": {
                    "type": "object",
                    "required": ["actions"],
                    "properties": {
                        "actions": {
                            "type": "string",
                            "description": "Comma-separated actions: forward, backward, turn_left, turn_right, fire, use, run, strafe_left, strafe_right, enter, escape, tab, y, n, 1-7"
                        },
                        "ticks": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 105,
                            "description": "Game ticks to advance (default: 7)"
                        },
                        "width": {
                            "type": "integer",
                            "minimum": 40,
                            "maximum": 200,
                            "description": "ANSI render width in columns (default: 100)"
                        }
                    }
                }
            },
            {
                "name": "doom_screenshot",
                "description": "Save a full-resolution screenshot of the current DOOM view and open it in an image viewer. Use this when the user asks to see what's happening, wants to see the screen, or asks for a screenshot. Does not advance the game.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
        ]
    })
}

fn handle_tool_call(params: &Value, engine: &mut Option<doom::Engine>, tracker: &mut GameTracker) -> Value {
    let tool_name = params["name"].as_str().unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    debug!("tool_call: {} args={}", tool_name, args);

    match tool_name {
        "doom_screenshot" => {
            let Some(eng) = engine.as_mut() else {
                return tool_error("Engine not running. Use doom_start to begin a game (e.g. with skill:3, episode:1, map:1).");
            };
            // Tick enough for weapon animations to finish and frame to settle
            eng.tick(SCREENSHOT_SETTLE_TICKS, &[]);
            let frame = eng.get_frame();
            let png = renderer::render_png_full(&frame);
            let path = std::env::temp_dir().join("doom-screenshot.png");
            let path_str = path.to_string_lossy().to_string();
            if let Err(e) = std::fs::write(&path, &png) {
                return tool_error(&format!("Failed to save screenshot: {e}"));
            }
            debug!("screenshot saved to {} ({} bytes)", path_str, png.len());

            // Auto-open with system viewer
            let openers = ["wslview", "xdg-open", "open", "cmd"];
            for opener in &openers {
                let result = if *opener == "cmd" {
                    std::process::Command::new("cmd")
                        .args(["/c", "start", "", &path_str])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                } else {
                    std::process::Command::new(opener)
                        .arg(&path_str)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                };
                if result.is_ok() {
                    debug!("screenshot opened with {}", opener);
                    break;
                }
            }

            let state = eng.get_state();
            let enemies = eng.get_enemies();
            tool_text(&format!(
                "Screenshot saved to {}\n{}\n{}",
                path_str,
                format_state(&state),
                format_enemies(&enemies, tracker, false),
            ))
        }

        "doom_start" => {
            let skill = args.get("skill").and_then(|v| v.as_i64()).unwrap_or(3) as i32;
            let episode = args.get("episode").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
            let map = args.get("map").and_then(|v| v.as_i64()).unwrap_or(1) as i32;

            if let Some(eng) = engine.as_mut() {
                // Engine already running — restart in-place using G_DeferedInitNew
                *tracker = GameTracker::new();
                eng.restart(skill, episode, map);
                return make_frame_response(eng, tracker);
            }

            let skill = Some(skill);
            let episode = Some(episode);
            let map = Some(map);
            let _width = args
                .get("width")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;

            let title_art = DOOM_TITLE;

            match doom::Engine::new(skill, episode, map) {
                Ok(mut eng) => {
                    // Run enough ticks for -warp to take effect and level to load
                    eng.tick(35, &[]);
                    eng.tick(35, &[]);
                    eng.tick(35, &[]);

                    let state = eng.get_state();
                    let enemies = eng.get_enemies();
                    let state_text = format_state(&state);
                    let enemy_text = format_enemies(&enemies, tracker, false);

                    let startup = format!(
                        "{}\n\n{}\n{}",
                        title_art,
                        state_text,
                        enemy_text,
                    );

                    *engine = Some(eng);

                    json!({
                        "content": [
                            {"type": "text", "text": startup},
                        ]
                    })
                }
                Err(e) => tool_error(&e),
            }
        }

        "doom_action" => {
            let Some(eng) = engine.as_mut() else {
                return tool_error("Engine not running. Use doom_start to begin a game (e.g. with skill:3, episode:1, map:1).");
            };

            let actions_str = args["actions"].as_str().unwrap_or("");
            let actions: Vec<&str> = actions_str.split(',').map(|s| s.trim()).collect();
            let ticks = args
                .get("ticks")
                .and_then(|v| v.as_i64())
                .unwrap_or(7) as i32;
            let _width = args
                .get("width")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as u32;

            let action_warnings = doom::Engine::validate_actions(&actions);

            let attempted_movement = actions.iter().any(|a| {
                matches!(*a, "forward" | "backward" | "strafe_left" | "strafe_right")
            });
            let is_firing = actions.iter().any(|a| *a == "fire");

            eng.tick(ticks, &actions);
            let mut response = make_frame_response_with_actions(eng, tracker, attempted_movement, is_firing);

            if !action_warnings.is_empty() {
                if let Some(content) = response.get_mut("content") {
                    if let Some(arr) = content.as_array_mut() {
                        if let Some(first) = arr.first_mut() {
                            if let Some(text) = first.get_mut("text") {
                                let prefix = action_warnings.join("\n");
                                *text = json!(format!("{}\n{}", prefix, text.as_str().unwrap_or("")));
                            }
                        }
                    }
                }
            }

            response
        }

        _ => tool_error(&format!("Unknown tool: {tool_name}")),
    }
}

// --- Response Builders ---

const SCREENSHOT_SETTLE_TICKS: i32 = 15;

struct GameTracker {
    last_kills: i32,
    last_hp: i32,
    last_weapon: i32,
    last_armor: i32,
    screenshot_offered: i32,
    recent_enemies: Vec<(String, i32, i32, i32)>, // (name, hp, angle, dist)
    recent_enemy_age: i32,
    last_item_dists: Vec<(i32, i32, i32)>, // (item_type, distance, receding_ticks)
    last_x: i32,
    last_y: i32,
    stuck_ticks: i32,
    ahead_enemy: Option<(i32, i32, i32)>, // (enemy_type, hp, stale_ticks)
}

impl GameTracker {
    fn new() -> Self {
        Self {
            last_kills: 0,
            last_hp: 100,
            last_weapon: 1,
            last_armor: 0,
            screenshot_offered: 0,
            recent_enemies: Vec::new(),
            recent_enemy_age: 0,
            last_item_dists: Vec::new(),
            last_x: i32::MIN,
            last_y: i32::MIN,
            stuck_ticks: 0,
            ahead_enemy: None,
        }
    }
}

fn make_frame_response(engine: &doom::Engine, tracker: &mut GameTracker) -> Value {
    make_frame_response_with_actions(engine, tracker, false, false)
}

fn make_frame_response_with_actions(engine: &doom::Engine, tracker: &mut GameTracker, attempted_movement: bool, is_firing: bool) -> Value {
    let frame = engine.get_frame();
    let state = engine.get_state();
    let enemies = engine.get_enemies();
    let items = engine.get_items();
    let interactables = engine.get_interactables();

    let png = renderer::render_png(&frame);
    let png_b64 = renderer::base64_encode(&png);
    let state_text = format_state(&state);
    let enemy_text = format_enemies(&enemies, tracker, is_firing);
    let item_text = format_items(&items, &state, tracker);
    let interactable_text = format_interactables(&interactables);

    debug!("STATE: {}", state_text);
    debug!("{}", enemy_text);
    debug!("png_b64={} chars", png_b64.len());

    // Stuck detection: only when a movement action was attempted
    let pos_changed = (state.x - tracker.last_x).abs() > 5 || (state.y - tracker.last_y).abs() > 5;
    if pos_changed {
        tracker.stuck_ticks = 0;
    } else if attempted_movement && tracker.last_x != i32::MIN {
        tracker.stuck_ticks += 1;
    } else if !attempted_movement {
        tracker.stuck_ticks = 0;
    }
    tracker.last_x = state.x;
    tracker.last_y = state.y;

    // Track interesting events
    let mut hints = Vec::new();

    if tracker.stuck_ticks >= 2 {
        hints.push("STUCK — position unchanged. Try: turn a different direction, strafe_left/strafe_right, or use on nearby walls.".into());
    }

    let new_kills = state.kills - tracker.last_kills;
    if new_kills > 0 {
        tracker.last_kills = state.kills;
        if state.kills == 1 || state.kills % 5 == 0 || new_kills >= 2 {
            tracker.screenshot_offered += 1;
            if tracker.screenshot_offered <= 5 {
                hints.push(format!("{} kills total! Offer the user a screenshot.", state.kills));
            }
        }
    }
    if state.health < tracker.last_hp - 30 {
        hints.push(format!("Took {} damage!", tracker.last_hp - state.health));
    }
    if state.health <= 20 && tracker.last_hp > 20 {
        hints.push("CRITICAL HP! Escape now: use strafe_left/strafe_right,backward,run — do NOT turn in place.".into());
    }
    if state.health <= 0 && tracker.last_hp > 0 {
        hints.push("YOU DIED! Offer the user a screenshot of the death screen.".into());
    }
    if state.weapon != tracker.last_weapon {
        hints.push(format!("Switched to {}.", weapon_name(state.weapon)));
        tracker.last_weapon = state.weapon;
    }
    if state.armor > tracker.last_armor {
        hints.push(format!("Picked up armor! Now at {}.", state.armor));
    }
    tracker.last_hp = state.health;
    tracker.last_armor = state.armor;

    let hints_text = if hints.is_empty() { String::new() } else { format!("\n{}", hints.join("\n")) };

    let mut full_text = state_text;
    let mut details: Vec<String> = Vec::new();
    if !enemy_text.is_empty() { details.push(enemy_text); }
    if !item_text.is_empty() { details.push(item_text); }
    if !interactable_text.is_empty() { details.push(interactable_text); }
    if !details.is_empty() {
        full_text.push_str(&format!("\n{}", details.join(" | ")));
    }
    if !hints_text.is_empty() {
        full_text.push_str(&hints_text);
    }

    json!({
        "content": [
            {"type": "text", "text": full_text},
            {"type": "image", "data": png_b64, "mimeType": "image/png"}
        ]
    })
}

fn tool_text(text: &str) -> Value {
    json!({
        "content": [{"type": "text", "text": text}]
    })
}

fn tool_error(text: &str) -> Value {
    json!({
        "content": [{"type": "text", "text": text}],
        "isError": true
    })
}

const DOOM_TITLE: &str = "\
======================================================
    ____    ___    ___    __  __
   / __ \\  / _ \\  / _ \\  /  |/  |
  / / / / / / / / / / / / / /|_/ /
 / / / / / / / / / / / / / /  / /
/ /_/ / / /_/ / / /_/ / / /  / /
\\____/  \\____/  \\____/ /_/  /_/

            - via MCP -
   Can it run DOOM? Yes, it can.
======================================================";

fn enemy_type_name(t: i32) -> &'static str {
    // MT_ enum values from info.h
    match t {
        1 => "Zombie",           // MT_POSSESSED (Former Human, HP:20)
        2 => "Shotgun Guy",      // MT_SHOTGUY (HP:30)
        3 => "Archvile",         // MT_VILE
        5 => "Revenant",         // MT_UNDEAD
        8 => "Mancubus",         // MT_FATSO
        10 => "Chaingunner",     // MT_CHAINGUY
        11 => "Imp",             // MT_TROOP (HP:60)
        12 => "Pinky Demon",     // MT_SERGEANT (HP:150)
        13 => "Spectre",         // MT_SHADOWS
        14 => "Cacodemon",       // MT_HEAD (HP:400)
        15 => "Baron of Hell",   // MT_BRUISER
        17 => "Hell Knight",     // MT_KNIGHT
        18 => "Lost Soul",       // MT_SKULL
        19 => "Spider Mastermind",// MT_SPIDER
        20 => "Arachnotron",     // MT_BABY
        21 => "Cyberdemon",      // MT_CYBORG
        22 => "Pain Elemental",  // MT_PAIN
        _ => "Enemy",
    }
}

fn item_name(t: i32) -> Option<&'static str> {
    // MT_ enum: MT_PLAYER=0, items start at MT_MISC0=43
    match t {
        43 => Some("Health Bonus (+1 HP)"),
        44 => Some("Armor Bonus (+1 armor)"),
        45 => Some("Medikit (+25 HP)"),
        46 => Some("Soulsphere (+100 HP)"),
        47 => Some("Backpack (ammo)"),
        48 => Some("Blue Keycard"),
        49 => Some("Red Keycard"),
        50 => Some("Yellow Keycard"),
        51 => Some("Blue Skull Key"),
        52 => Some("Red Skull Key"),
        53 => Some("Stimpack (+10 HP)"),
        54 => Some("Armor (Green)"),
        55 => Some("Armor (Blue)"),
        56 | 57 => Some("Invulnerability"),
        58 => Some("Berserk (+100 HP)"),
        59 => Some("Invisibility"),
        60 => Some("Radiation Suit"),
        61 => Some("Computer Map"),
        62 => Some("Light Amp Visor"),
        63 => Some("Megasphere"),
        64 => Some("Ammo Clip"),
        65 => Some("Box of Ammo"),
        66 => Some("Rocket"),
        67 => Some("Box of Rockets"),
        68 => Some("Energy Cell"),
        69 => Some("Energy Cell Pack"),
        70 => Some("Shells (4)"),
        71 => Some("Box of Shells"),
        76 => Some("Chaingun"),
        77 => Some("Rocket Launcher"),
        78 => Some("Plasma Gun"),
        79 => Some("BFG 9000"),
        80 => Some("Shotgun"),
        81 => Some("Super Shotgun"),
        _ => None,
    }
}

fn format_items(items: &[doom::ItemInfo], state: &doom::GameState, tracker: &mut GameTracker) -> String {
    if items.is_empty() {
        tracker.last_item_dists.clear();
        return String::new();
    }

    let mut parts: Vec<String> = Vec::new();
    let mut new_dists: Vec<(i32, i32, i32)> = Vec::new();

    for item in items {
        // Only show items within the player's field of view (90deg FOV = ±45deg)
        if item.angle.abs() > 45 { continue; }

        if let Some(name) = item_name(item.item_type) {
            let dominated = match item.item_type {
                43 | 53 | 45 | 46 | 58 => state.health >= 100,
                44 | 54 | 55 | 63 => state.armor >= 100,
                _ => false,
            };
            if dominated { continue; }

            // Compute delta and receding count from previous tick
            let (delta, receding_ticks) = tracker.last_item_dists.iter()
                .find(|(t, _, _)| *t == item.item_type)
                .map(|(_, prev_dist, prev_receding)| {
                    let d = item.distance - prev_dist;
                    let receding = if d > 10 { prev_receding + 1 } else { 0 };
                    (d, receding)
                })
                .unwrap_or((0, 0));

            // Suppress items that have been receding for 2+ ticks — unreachable
            if receding_ticks >= 2 {
                new_dists.push((item.item_type, item.distance, receding_ticks));
                continue;
            }

            let delta_str = if delta < -10 { " CLOSING" } else if delta > 10 { " RECEDING" } else { "" };

            let dir = format_dir(item.angle);
            let run_ticks = (item.distance as f32 / 16.0).ceil() as i32;
            let reach_hint = format!(" (~{} ticks fwd+run to reach)", run_ticks);
            parts.push(format!("{} {} {}{}{}", name, dir, format_dist(item.distance), delta_str, reach_hint));
            new_dists.push((item.item_type, item.distance, receding_ticks));
        }
    }

    tracker.last_item_dists = new_dists;

    if parts.is_empty() {
        return String::new();
    }

    format!("ITEMS: {}", parts.join(" | "))
}

fn format_dist(distance: i32) -> &'static str {
    match distance {
        d if d < 100 => "point-blank",
        d if d < 300 => "close",
        d if d < 600 => "nearby",
        d if d < 1000 => "far",
        _ => "very far",
    }
}

fn format_dir(angle: i32) -> String {
    // ~3.2 degrees per tick of turning
    let ticks_hint = (angle.abs() as f32 / 3.2).round() as i32;
    let abs = angle.abs();
    if abs <= 10 {
        "AHEAD".into()
    } else {
        let turn = if angle > 0 { "turn_left" } else { "turn_right" };
        let label = match abs {
            11..=30  => if angle > 0 { "slightly to your left" } else { "slightly to your right" },
            31..=60  => if angle > 0 { "to your left"          } else { "to your right"          },
            61..=90  => if angle > 0 { "hard left"             } else { "hard right"              },
            91..=135 => if angle > 0 { "far left"              } else { "far right"               },
            _        => if angle > 0 { "behind you to the left"} else { "behind you to the right" },
        };
        format!("{} ({} ~{})", label, turn, ticks_hint)
    }
}

fn format_enemies(enemies: &[doom::EnemyInfo], tracker: &mut GameTracker, is_firing: bool) -> String {
    let mut visible: Vec<String> = Vec::new();

    // Find the most-ahead visible enemy for stale-fire detection
    let most_ahead = enemies.iter()
        .filter(|e| e.visible != 0)
        .min_by_key(|e| e.angle.abs());

    let stale_warning = if let Some(e) = most_ahead {
        // Only accumulate stale ticks while actively firing; reset otherwise
        let stale_ticks = if is_firing {
            match tracker.ahead_enemy {
                Some((t, hp, ticks)) if t == e.enemy_type && hp == e.health => ticks + 1,
                _ => 0,
            }
        } else {
            0
        };
        tracker.ahead_enemy = Some((e.enemy_type, e.health, stale_ticks));
        if is_firing && stale_ticks >= 3 && e.angle.abs() <= 20 {
            Some(format!(
                "{} not taking damage — likely behind cover. Advance or reposition for clear LOS.",
                enemy_type_name(e.enemy_type)
            ))
        } else {
            None
        }
    } else {
        tracker.ahead_enemy = None;
        None
    };

    for e in enemies {
        if e.visible == 0 { continue; }
        let name = enemy_type_name(e.enemy_type);
        let dir = format_dir(e.angle);
        let dist_label = format_dist(e.distance);
        let danger = if e.distance < 100 {
            " ⚠ TOO CLOSE — strafe and back away!"
        } else {
            ""
        };
        visible.push(format!("{} (HP:{}) {} {}{}", name, e.health, dir, dist_label, danger));
    }

    if !visible.is_empty() {
        tracker.recent_enemies.clear();
        for e in enemies {
            if e.visible == 0 { continue; }
            tracker.recent_enemies.push((
                enemy_type_name(e.enemy_type).to_string(),
                e.health,
                e.angle,
                e.distance,
            ));
        }
        tracker.recent_enemy_age = 0;
        let mut result = format!("ENEMIES IN SIGHT (aim for angle~0 then fire): {}", visible.join(" | "));
        if let Some(warn) = stale_warning {
            result.push_str(&format!(" | ⚠ {}", warn));
        }
        result
    } else {
        tracker.recent_enemy_age += 1;
        if tracker.recent_enemy_age <= 3 && !tracker.recent_enemies.is_empty() {
            let last_seen: Vec<String> = tracker.recent_enemies.iter()
                .map(|(name, hp, angle, dist)| {
                    format!("{} (HP:{}) was ~{} {}", name, hp, format_dir(*angle), format_dist(*dist))
                })
                .collect();
            format!("No enemies in sight. Recently seen nearby: {}. Try moving toward them or checking corners.", last_seen.join(" | "))
        } else {
            tracker.recent_enemies.clear();
            "No enemies in sight. Explore: move forward, open doors with 'use', check around corners.".into()
        }
    }
}

fn format_interactables(interactables: &[doom::InteractableInfo]) -> String {
    if interactables.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = interactables.iter().map(|i| {
        let label = match i.kind {
            0 => "Door".to_string(),
            1 => {
                let key = match i.key {
                    1 => "blue key",
                    2 => "red key",
                    3 => "yellow key",
                    _ => "key",
                };
                format!("Locked Door (needs {})", key)
            }
            2 => "Exit Switch".to_string(),
            _ => "Switch".to_string(),
        };
        format!("{} {} (use to activate)", label, format_dir(i.angle))
    }).collect();
    format!("NEARBY: {}", parts.join(" | "))
}

fn weapon_name(w: i32) -> &'static str {
    match w {
        0 => "Fists (key:1)",
        1 => "Pistol (key:2)",
        2 => "Shotgun (key:3)",
        3 => "Chaingun (key:4)",
        4 => "Rocket Launcher (key:5)",
        5 => "Plasma Gun (key:6)",
        6 => "BFG (key:7)",
        7 => "Chainsaw (key:1)",
        _ => "Unknown",
    }
}

fn format_state(s: &doom::GameState) -> String {
    format!(
        "HP:{} Armor:{} | {} | Ammo: {}b {}s {}r {}c | Kills:{} | ({},{}) {}deg | E{}M{}",
        s.health, s.armor,
        weapon_name(s.weapon),
        s.ammo_bullets, s.ammo_shells, s.ammo_rockets, s.ammo_cells,
        s.kills,
        s.x, s.y, s.angle,
        s.episode, s.map
    )
}
