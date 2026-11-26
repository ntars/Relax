pub mod handler;
pub mod state;

pub use handler::*;
pub use state::*;

pub async fn process() -> &'static str {
    "module announcements initialized"
}
