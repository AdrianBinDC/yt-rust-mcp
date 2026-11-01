pub mod call;
pub mod config;
pub mod error;
pub mod server;
pub mod store;
pub mod youtube;

pub fn default_log_filter() -> &'static str {
    "warn"
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_log_filter_is_warn() {
        assert_eq!(crate::default_log_filter(), "warn");
    }
}
