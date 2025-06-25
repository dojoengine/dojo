//! Tools for the MCP server.
//!
//! The profile is usually passed as parameter which gives the possibility
//! to the client to select the profile to use for the action.
//!
//! In the current implementation, the manifest path is passed at the server
//! level, and not configurable at the tool level.
pub mod build;
pub mod execute;
pub mod inspect;
pub mod migrate;
pub mod test;

pub use build::BuildRequest;
pub use execute::ExecuteRequest;
pub use inspect::InspectRequest;
pub use migrate::MigrateRequest;
pub use test::TestRequest;
