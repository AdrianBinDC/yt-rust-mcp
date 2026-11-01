use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("read config: {0}")]
    Read(#[from] std::io::Error),
    #[error("parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("read server instructions {path}: {source}")]
    Instructions {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("bad integer in {name}: {value}")]
    BadInt { name: &'static str, value: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct TtlSeconds {
    pub search: i64,
    pub videos: i64,
    pub channels: i64,
    pub playlists: i64,
    pub playlist_items: i64,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub search_budget: i64,
    pub non_search_budget: i64,
    pub max_entries: i64,
    pub max_pages_hard_cap: u32,
    pub sqlite_page_cache_kib: i64,
    pub ttl_seconds: TtlSeconds,
    pub cache_dir: PathBuf,
    pub db_path: PathBuf,
    pub prompts_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    search_budget: i64,
    non_search_budget: i64,
    max_entries: i64,
    max_pages_hard_cap: u32,
    sqlite_page_cache_kib: i64,
    cache_dir: Option<PathBuf>,
    ttl_seconds: TtlSeconds,
}

impl Config {
    /// Server instructions shown to the model. Missing file is a startup error.
    pub fn load_instructions(&self) -> Result<String, ConfigError> {
        let path = self.prompts_dir.join("server.md");
        std::fs::read_to_string(&path).map_err(|source| ConfigError::Instructions { path, source })
    }

    pub fn load(config_path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(config_path)?;
        let file: FileConfig = toml::from_str(&text)?;
        let search_budget = env_i64("YOUTUBE_SEARCH_BUDGET", file.search_budget)?;
        let non_search_budget = env_i64("YOUTUBE_NON_SEARCH_BUDGET", file.non_search_budget)?;
        let cache_dir = std::env::var_os("YT_RUST_MCP_CACHE_DIR")
            .map(PathBuf::from)
            .or(file.cache_dir)
            .or_else(|| dirs::cache_dir().map(|d| d.join("yt-rust-mcp")))
            .unwrap_or_else(|| PathBuf::from("yt-rust-mcp-cache"));
        let prompts_dir = std::env::var_os("YT_RUST_MCP_PROMPTS")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("prompts"));
        Ok(Self {
            search_budget,
            non_search_budget,
            max_entries: file.max_entries,
            max_pages_hard_cap: file.max_pages_hard_cap,
            sqlite_page_cache_kib: file.sqlite_page_cache_kib,
            ttl_seconds: file.ttl_seconds,
            db_path: cache_dir.join("yt-rust-mcp.db"),
            cache_dir,
            prompts_dir,
        })
    }
}

fn env_i64(name: &'static str, default: i64) -> Result<i64, ConfigError> {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| ConfigError::BadInt { name, value }),
        Err(_) => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn env_budget_overrides_toml() {
        let dir = std::env::temp_dir().join(format!("yt-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("default.toml");
        let mut file = std::fs::File::create(&path).unwrap();
        write!(
            file,
            "search_budget = 100\nnon_search_budget = 10000\nmax_entries = 10000\nmax_pages_hard_cap = 5\nsqlite_page_cache_kib = 2048\ncache_dir = \"/tmp/yt-default\"\n"
        )
        .unwrap();
        write!(
            file,
            "[ttl_seconds]\nsearch = 3600\nvideos = 86400\nchannels = 604800\nplaylists = 604800\nplaylist_items = 86400\n"
        )
        .unwrap();
        std::env::set_var("YOUTUBE_SEARCH_BUDGET", "7");
        std::env::set_var("YT_RUST_MCP_CACHE_DIR", dir.join("cache").to_str().unwrap());
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg.search_budget, 7);
        assert_eq!(cfg.non_search_budget, 10_000);
        assert_eq!(cfg.ttl_seconds.search, 3600);
        assert_eq!(cfg.db_path, dir.join("cache").join("yt-rust-mcp.db"));
        std::env::remove_var("YOUTUBE_SEARCH_BUDGET");
        std::env::remove_var("YT_RUST_MCP_CACHE_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
