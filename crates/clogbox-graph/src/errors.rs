use thiserror::Error;

/// Error type used when an algorithm that does not work with cycles encounters one.
#[derive(Debug, Error, Eq, PartialEq)]
#[error("Cycle detected")]
pub struct CycleDetected;
