use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("api request failed: {0}")]
    Api(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("repository error: {0}")]
    Repository(String),
    #[error("relay error: {0}")]
    Relay(String),
    #[error("discord error: {0}")]
    Discord(String),
    #[error("unexpected empty leaderboard for period {0}")]
    EmptyLeaderboard(String),
    #[error("no active creator found for period {0}")]
    NoActiveCreator(String),
}
