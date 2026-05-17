pub mod approval;
pub mod ctf;
pub mod errors;
pub mod relayer;
pub mod wallet;

pub const CHAIN_ID: u64 = 137;

pub fn init() {
    tracing_subscriber::fmt::init();
    tracing::debug!("blaze-poly initialized");
}

pub use ctf::RelayerCall;
pub use ctf::SubmitBatchResponse;
pub use relayer::RelayerClient;
