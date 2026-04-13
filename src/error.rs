use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("api request failed: {0}")]
    Api(String),
    #[error("unexpected empty leaderboard for period {0}")]
    EmptyLeaderboard(String),
    #[error("no active creator found for period {0}")]
    NoActiveCreator(String),
}
