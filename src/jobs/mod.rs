//! Work that runs without a request: the daily digest and ntfy push, the
//! nightly backup and the scheduler that starts them.

pub mod backup;
pub mod digest;
pub mod notify;
pub mod queries;
pub mod scheduler;
