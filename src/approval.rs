use crate::{
    CHAIN_ID,
    errors::{BlazePolyError, Result},
    relayer::RelayerClient,
};

use alloy_primitives::{Address, TxHash, U256, address};
use alloy_provider::DynProvider;
use alloy_sol_types::sol;

use tokio::try_join;

const CONDITIONAL_TOKENS: Address = address!("0x4D97DCd97eC945f40cF65F87097ACe5EA0476045");

const EXCHANGE_V2: Address = address!("0xE111180000d2663C0091e4f400237545B87B996B");

const NEG_RISK_EXCHANGE_V2: Address = address!("0xe2222d279d744050d28e00520010520000310F59");

const PUSD: Address = address!("0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB");

const CTF_COLLATERAL_ADAPTER: Address = address!("0xAdA100Db00Ca00073811820692005400218FcE1f");

const NEG_RISK_CTF_COLLATERAL_ADAPTER: Address =
    address!("0xadA2005600Dec949baf300f4C6120000bDB6eAab");

const MAX_ALLOWANCE: U256 = U256::MAX;

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function approve(address spender, uint256 amount) external returns (bool);

        function allowance(
            address owner,
            address spender
        ) external view returns (uint256);

        function balanceOf(address owner)
            external
            view
            returns (uint256);
    }

    #[sol(rpc)]
    interface IERC1155 {
        function setApprovalForAll(
            address operator,
            bool approved
        ) external;

        function isApprovedForAll(
            address account,
            address operator
        ) external view returns (bool);
    }
}

type Erc20 = IERC20::IERC20Instance<DynProvider>;
type Erc1155 = IERC1155::IERC1155Instance<DynProvider>;

// Aprovals must be made from the deposit wallet

impl RelayerClient {
    pub async fn approve_all(&self, provider: DynProvider) -> Result<()> {
        let collateral = IERC20::new(PUSD, provider.clone());

        let conditional = IERC1155::new(CONDITIONAL_TOKENS, provider);

        let owner = self.owner();

        tracing::debug!(
            owner = %owner,
            collateral_token = %PUSD,
            conditional_tokens = %CONDITIONAL_TOKENS,
            chain_id = CHAIN_ID,
            "starting approval bootstrap"
        );

        try_join!(
            ensure_exchange_approvals(
                &collateral,
                &conditional,
                owner,
                EXCHANGE_V2,
                "CTF exchange V2",
            ),
            ensure_exchange_approvals(
                &collateral,
                &conditional,
                owner,
                NEG_RISK_EXCHANGE_V2,
                "NegRisk exchange V2",
            ),
            ensure_erc20_allowance(
                &collateral,
                owner,
                CTF_COLLATERAL_ADAPTER,
                MAX_ALLOWANCE,
                "CTF collateral adapter",
            ),
            ensure_erc20_allowance(
                &collateral,
                owner,
                NEG_RISK_CTF_COLLATERAL_ADAPTER,
                MAX_ALLOWANCE,
                "NegRisk collateral adapter",
            ),
        )?;

        Ok(())
    }
}

async fn ensure_exchange_approvals(
    collateral: &Erc20,
    conditional: &Erc1155,
    owner: Address,
    spender: Address,
    label: &'static str,
) -> Result<()> {
    tracing::debug!(
        contract = label,
        owner = %owner,
        spender = %spender,
        "checking exchange approvals"
    );

    try_join!(
        ensure_erc20_allowance(collateral, owner, spender, MAX_ALLOWANCE, label,),
        ensure_erc1155_approval_for_all(conditional, owner, spender, true, label,),
    )?;

    Ok(())
}

async fn ensure_erc20_allowance(
    token: &Erc20,
    owner: Address,
    spender: Address,
    required: U256,
    label: &'static str,
) -> Result<()> {
    let current =
        token.allowance(owner, spender).call().await.map_err(|e| {
            BlazePolyError::External(format!("failed to read ERC20 allowance: {e}"))
        })?;

    tracing::debug!(
        contract = label,
        owner = %owner,
        spender = %spender,
        current_allowance = %current,
        required_allowance = %required,
        "read ERC20 allowance"
    );

    if current >= required {
        return Ok(());
    }

    tracing::warn!(
        contract = label,
        owner = %owner,
        spender = %spender,
        current_allowance = %current,
        required_allowance = %required,
        "ERC20 allowance insufficient; approving"
    );

    let tx_hash = approve(token, spender, required).await?;

    tracing::debug!(
        contract = label,
        tx_hash = %tx_hash,
        "ERC20 approval confirmed"
    );

    Ok(())
}

async fn ensure_erc1155_approval_for_all(
    token: &Erc1155,
    owner: Address,
    operator: Address,
    approved: bool,
    label: &'static str,
) -> Result<()> {
    let current = token
        .isApprovedForAll(owner, operator)
        .call()
        .await
        .map_err(|e| {
            BlazePolyError::External(format!("failed to read ERC1155 approval for all: {e}"))
        })?;

    tracing::debug!(
        contract = label,
        owner = %owner,
        operator = %operator,
        current_approved = current,
        desired_approved = approved,
        "read ERC1155 approval state"
    );

    if current == approved {
        return Ok(());
    }

    tracing::warn!(
        contract = label,
        owner = %owner,
        operator = %operator,
        current_approved = current,
        desired_approved = approved,
        "ERC1155 approval missing; approving"
    );

    let tx_hash = set_approval_for_all(token, operator, approved).await?;

    tracing::debug!(
        contract = label,
        tx_hash = %tx_hash,
        "ERC1155 approval confirmed"
    );

    Ok(())
}

async fn approve(token: &Erc20, spender: Address, amount: U256) -> Result<TxHash> {
    let pending = token
        .approve(spender, amount)
        .send()
        .await
        .map_err(|e| BlazePolyError::External(format!("failed to set ERC20 allowance: {e}")))?;

    pending.watch().await.map_err(|e| {
        BlazePolyError::External(format!(
            "failed to wait for pending ERC20 allowance setting: {e}"
        ))
    })
}

async fn set_approval_for_all(
    token: &Erc1155,
    operator: Address,
    approved: bool,
) -> Result<TxHash> {
    let pending = token
        .setApprovalForAll(operator, approved)
        .send()
        .await
        .map_err(|e| {
            BlazePolyError::External(format!(
                "failed to submit ERC1155 approval transaction: {e}"
            ))
        })?;

    pending.watch().await.map_err(|e| {
        BlazePolyError::External(format!(
            "failed waiting for ERC1155 approval confirmation: {e}"
        ))
    })
}
