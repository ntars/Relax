//! Relax Server - 服务端逻辑

pub mod state;
pub mod middleware;
pub mod routes;
pub mod migrate;

pub use state::{AppState, AppInfo};
pub use routes::router;
pub use migrate::{migrate, checksum, schema_version, MigrateError, MigrateOutcome};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_api_exposed() {
        let _ = checksum("select 1;");
    }
}
