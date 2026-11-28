//! Relax Server - 服务端逻辑

pub mod state;
pub mod middleware;
pub mod routes;
pub mod migrate;
pub mod cert;
pub mod common;
pub mod app;
pub mod auth;
pub mod modules;

pub use state::{AppState, AppInfo};
pub use app::app;
pub use common::{AppError, TraceId, ok_envelope, err_envelope};
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
