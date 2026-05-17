use crate::{
    CHAIN_ID,
    errors::{BlazePolyError, Result},
    relayer::{RelayerClient, handle_response},
    wallet::DEPOSIT_WALLET_FACTORY,
};
use alloy_primitives::{Address, U256, hex};
use alloy_signer::SignerSync as _;
use alloy_sol_types::{eip712_domain, sol};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_DEADLINE: u64 = 600;

sol! {
    #[derive(Serialize)]
    struct RelayerCall {
        address target;
        uint256 value;
        bytes data;
    }

    struct RelayerBatch {
        address wallet;
        uint256 nonce;
        uint256 deadline;
        RelayerCall[] calls;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitBatchRequest {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub from: Address,
    pub to: Address,
    pub nonce: U256,
    pub signature: String,
    pub deposit_wallet_params: DepositWalletParams,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositWalletParams {
    pub deposit_wallet: Address,
    pub deadline: U256,
    pub calls: Vec<RelayerCall>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubmitBatchResponse {
    transaction_id: Option<String>,
    state: Option<String>,
}

/// Get current Unix timestamp in seconds
#[must_use]
pub fn get_current_unix_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

// 65-byte EIP-712 with 0x prefix!
impl RelayerClient {
    pub async fn execute(&self, calls: Vec<RelayerCall>) -> Result<SubmitBatchResponse> {
        let current_time = get_current_unix_time_secs();

        let deadline = current_time.saturating_add(DEFAULT_DEADLINE);

        self.create_batch(calls, deadline).await
    }

    pub async fn create_batch(
        &self,
        calls: Vec<RelayerCall>,
        deadline: u64,
    ) -> Result<SubmitBatchResponse> {
        let nonce = self.wallet_nonce().await?;

        let deposit_wallet = self
            .deposit_wallet()
            .ok_or_else(|| BlazePolyError::Internal("deposit wallet not set".to_string()))?;

        let owner_signer = self.signer();

        let message = RelayerBatch {
            wallet: deposit_wallet,
            nonce,
            deadline: U256::from(deadline),
            calls,
        };

        let domain = eip712_domain!(
            name: "DepositWallet",
            version: "1",
            chain_id: CHAIN_ID,
            verifying_contract: message.wallet,
        );

        let signature = owner_signer
            .sign_typed_data_sync(&message, &domain)
            .map_err(|e| BlazePolyError::Internal(format!("failed to sign wallet batch: {e}")))?;

        let v = signature.v() as u8;

        let mut packed = [0u8; 65];

        packed[..32].copy_from_slice(&signature.r().to_be_bytes::<32>());
        packed[32..64].copy_from_slice(&signature.s().to_be_bytes::<32>());
        packed[64] = v;
        let signature = format!("0x{}", hex::encode(packed));

        tracing::debug!(signature = %signature, "Batch signature");

        self.submit_batch(message, signature).await
    }

    pub async fn submit_batch(
        &self,
        batch: RelayerBatch,
        signature: String,
    ) -> Result<SubmitBatchResponse> {
        let request = SubmitBatchRequest {
            kind: "WALLET",
            from: self.owner(),
            to: DEPOSIT_WALLET_FACTORY,
            nonce: batch.nonce,
            signature,
            deposit_wallet_params: DepositWalletParams {
                deposit_wallet: batch.wallet,
                deadline: batch.deadline,
                calls: batch.calls,
            },
        };

        let body = serde_json::to_string(&request).map_err(|e| {
            BlazePolyError::Internal(format!("failed to serialize batch request into json: {e}"))
        })?;

        let resp =
            handle_response::<SubmitBatchResponse>(self.post("/submit")?.body(body).send().await)
                .await?;

        tracing::debug!(
            transaction_id = resp.transaction_id,
            state = resp.state,
            "got submit relayer batch response"
        );

        Ok(resp)
    }
}
