use alloy_primitives::{Address, B256, address, hex, keccak256};

use crate::relayer::RelayerClient;

pub const DEPOSIT_WALLET_FACTORY: Address = address!("0x00000000000Fb5C9ADea0298D729A0CB3823Cc07");

const DEPOSIT_WALLET_IMPL: Address = address!("0x58CA52ebe0DadfdF531Cde7062e76746de4Db1eB");

const ERC1967_CONST1: &[u8] =
    &hex!("0xcc3735a920a3ca505d382bbc545af43d6000803e6038573d6000fd5b3d6000f3");

const ERC1967_CONST2: &[u8] =
    &hex!("0x5155f3363d3d373d3d363d7f360894a13ba1a3210667c828492db98dca3e2076");

const CONST_6009: [u8; 2] = [0x60, 0x09];

const ERC1967_PREFIX: u128 = 0x6100_3D3D_8160_233D_3973;

impl RelayerClient {
    pub fn derive_deposit_wallet_address(&mut self) {
        let mut wallet_id = [0u8; 32];
        wallet_id[12..].copy_from_slice(self.owner().as_slice());

        let mut args = [0u8; 64];

        args[12..32].copy_from_slice(DEPOSIT_WALLET_FACTORY.as_slice());

        args[32..64].copy_from_slice(&wallet_id);

        let bytecode_hash = init_code_hash_erc1967(&args);

        let salt = keccak256(args);

        let dep_wallet_addr = DEPOSIT_WALLET_FACTORY.create2(salt, bytecode_hash);
        self.set_deposit_wallet(dep_wallet_addr);
    }
}

fn init_code_hash_erc1967(args: &[u8]) -> B256 {
    let n = args.len() as u64;

    let combined: u128 = ERC1967_PREFIX + (u128::from(n) << 56);

    let combined_bytes = &combined.to_be_bytes()[6..];

    let mut init_code = Vec::with_capacity(
        combined_bytes.len()
            + 20
            + CONST_6009.len()
            + ERC1967_CONST2.len()
            + ERC1967_CONST1.len()
            + args.len(),
    );

    init_code.extend_from_slice(combined_bytes);
    init_code.extend_from_slice(DEPOSIT_WALLET_IMPL.as_slice());
    init_code.extend_from_slice(&CONST_6009);
    init_code.extend_from_slice(ERC1967_CONST2);
    init_code.extend_from_slice(ERC1967_CONST1);
    init_code.extend_from_slice(args);

    keccak256(&init_code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_signer_local::PrivateKeySigner;

    #[test]
    fn test_derive_deposit_wallet_address() {
        let private_key = "0x1234567890123456789012345678901234567890123456789012345678901234";

        const RELAYER_API_KEY: &'static str = "";
        const RELAYER_API_KEY_ADDRESS: Address = Address::ZERO;

        let signer: PrivateKeySigner = private_key.parse().expect("Valid private key");

        let mut client = RelayerClient::new(
            None,
            RELAYER_API_KEY.to_string(),
            RELAYER_API_KEY_ADDRESS,
            signer,
        )
        .expect("Valid Input");

        let actual_deposit_wallet_addr = address!("0xc223ea304a9d2fea3033eac327c87c26e26d32c1");

        client.derive_deposit_wallet_address();

        assert_eq!(client.deposit_wallet(), Some(actual_deposit_wallet_addr));
    }
}
