use std::fs::OpenOptions;
use std::io::Write;
use std::sync::OnceLock;

static DEBUG: OnceLock<bool> = OnceLock::new();
static LOG_PATH: &str = "/tmp/doom-mcp.log";

pub fn is_debug() -> bool {
    *DEBUG.get_or_init(|| std::env::var("DOOM_MCP_DEBUG").is_ok_and(|v| v == "1"))
}

pub fn log(msg: &str) {
    if !is_debug() {
        return;
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let _ = writeln!(f, "[{}] {}", timestamp(), msg);
    }
}

fn timestamp() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let ms = dur.subsec_millis();
    format!("{secs}.{ms:03}")
}

#[macro_export]
macro_rules! doom_debug {
    ($($arg:tt)*) => {
        $crate::log::log(&format!($($arg)*))
    };
}

