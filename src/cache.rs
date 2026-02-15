use std::io;
use std::path::Path;

use crate::models::AlbumResult;

const CACHE_FILENAME: &str = "dr_report.json";

/// Load a cached album result from `dr_report.json` in the given directory.
/// Returns `None` if the file is missing or cannot be parsed.
pub fn load_cached_report(dir: &Path) -> Option<AlbumResult> {
    let path = dir.join(CACHE_FILENAME);
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Save an album result as pretty-printed JSON to `dr_report.json` in the given directory.
pub fn save_report(dir: &Path, result: &AlbumResult) -> io::Result<()> {
    let path = dir.join(CACHE_FILENAME);
    let json = serde_json::to_string_pretty(result)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    std::fs::write(&path, json)
}

const TEXT_REPORT_FILENAME: &str = "dr_report.txt";

/// Check if all requested report files already exist in the given directory.
pub fn reports_exist(dir: &Path, json: bool, txt: bool) -> bool {
    if json && !dir.join(CACHE_FILENAME).exists() {
        return false;
    }
    if txt && !dir.join(TEXT_REPORT_FILENAME).exists() {
        return false;
    }
    true
}

/// Save a text report to `dr_report.txt` in the given directory.
pub fn save_text_report(dir: &Path, content: &str) -> io::Result<()> {
    let path = dir.join(TEXT_REPORT_FILENAME);
    std::fs::write(&path, content)
}
