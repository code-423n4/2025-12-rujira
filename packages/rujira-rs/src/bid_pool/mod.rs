/// See https://github.com/liquity/liquity/blob/master/papers/Scalable_Reward_Distribution_with_Compounding_Stakes.pdf
/// for algorithm reference
mod bid;
mod error;
mod pool;
mod sum_snapshot;

pub use bid::Bid;
pub use error::BidPoolError;
pub use pool::Pool;
pub use sum_snapshot::{SumSnapshot, SumSnapshotKey};
