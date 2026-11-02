//! CLI / env selection for stdio vs Streamable HTTP.

/// Default bind for `--http` when neither flag nor env sets an address.
pub const DEFAULT_HTTP_BIND: &str = "127.0.0.1:8087";

/// Resolved transport after CLI + env.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportMode {
    Stdio,
    Http { bind: String },
}

/// Decide transport from `--http` / `--stdio` and env.
///
/// Env: `YT_RUST_MCP_HTTP=1|true` enables HTTP when `--http` was not passed.
/// `YT_RUST_MCP_BIND` overrides the bind string when HTTP is selected.
/// `--http` wins over `--stdio`. Explicit `--stdio` with HTTP env still forces stdio
/// unless `--http` is set.
pub fn resolve_transport(http_flag: bool, stdio_flag: bool, bind_flag: Option<String>) -> TransportMode {
    let env_http = env_truthy("YT_RUST_MCP_HTTP");
    let use_http = http_flag || (env_http && !stdio_flag);
    if !use_http {
        return TransportMode::Stdio;
    }
    let bind = bind_flag
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("YT_RUST_MCP_BIND").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| DEFAULT_HTTP_BIND.to_string());
    TransportMode::Http { bind }
}

fn env_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true")
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env mutation must be serialized across tests in this process.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_http_env() {
        std::env::remove_var("YT_RUST_MCP_HTTP");
        std::env::remove_var("YT_RUST_MCP_BIND");
    }

    #[test]
    fn default_is_stdio() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_http_env();
        assert_eq!(
            resolve_transport(false, false, None),
            TransportMode::Stdio
        );
    }

    #[test]
    fn http_flag_uses_default_bind() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_http_env();
        assert_eq!(
            resolve_transport(true, false, None),
            TransportMode::Http {
                bind: DEFAULT_HTTP_BIND.to_string()
            }
        );
    }

    #[test]
    fn http_flag_uses_bind_flag() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_http_env();
        assert_eq!(
            resolve_transport(true, false, Some("0.0.0.0:8083".into())),
            TransportMode::Http {
                bind: "0.0.0.0:8083".into()
            }
        );
    }

    #[test]
    fn env_http_enables_http() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_http_env();
        std::env::set_var("YT_RUST_MCP_HTTP", "1");
        assert_eq!(
            resolve_transport(false, false, None),
            TransportMode::Http {
                bind: DEFAULT_HTTP_BIND.to_string()
            }
        );
        clear_http_env();
    }

    #[test]
    fn env_bind_overrides_default() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_http_env();
        std::env::set_var("YT_RUST_MCP_HTTP", "true");
        std::env::set_var("YT_RUST_MCP_BIND", "127.0.0.1:9999");
        assert_eq!(
            resolve_transport(false, false, None),
            TransportMode::Http {
                bind: "127.0.0.1:9999".into()
            }
        );
        clear_http_env();
    }

    #[test]
    fn explicit_stdio_beats_env_http() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_http_env();
        std::env::set_var("YT_RUST_MCP_HTTP", "1");
        assert_eq!(
            resolve_transport(false, true, None),
            TransportMode::Stdio
        );
        clear_http_env();
    }

    #[test]
    fn http_flag_beats_stdio_flag() {
        let _g = ENV_LOCK.lock().unwrap();
        clear_http_env();
        assert_eq!(
            resolve_transport(true, true, None),
            TransportMode::Http {
                bind: DEFAULT_HTTP_BIND.to_string()
            }
        );
    }
}
