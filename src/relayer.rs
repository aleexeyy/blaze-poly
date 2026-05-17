use crate::errors::{BlazePolyError, Result};
use alloy_primitives::{Address, U256};
use alloy_signer_local::PrivateKeySigner;
use reqwest::{
    Client as HttpClient, Url,
    header::{HeaderMap, HeaderValue},
};
use serde::{Deserialize, de::DeserializeOwned};
use std::{str::FromStr, time::Duration};
const RELAYER_DEFAULT_HOST_URL: &str = "https://relayer-v2.polymarket.com";

pub struct RelayerClient {
    client: HttpClient,
    base_url: Url,
    deposit_wallet: Option<Address>,
    signer: PrivateKeySigner,
}

impl RelayerClient {
    pub fn new(
        base_url: Option<&str>,
        api_key: String,
        api_key_address: Address,
        signer: PrivateKeySigner,
    ) -> Result<Self> {
        let mut auth_headers = HeaderMap::new();

        let base_url = Url::from_str(base_url.unwrap_or(RELAYER_DEFAULT_HOST_URL)).unwrap_or(
            Url::from_str(RELAYER_DEFAULT_HOST_URL)
                .map_err(|e| BlazePolyError::Invalid(format!("Invalid relayer base url: {e}")))?,
        );

        let key_value = HeaderValue::from_str(&api_key)
            .map_err(|e| BlazePolyError::Invalid(format!("Invalid relayer api key format: {e}")))?;
        auth_headers.insert("RELAYER_API_KEY", key_value);

        let addr_string = api_key_address.to_string();
        let addr_value = HeaderValue::from_str(&addr_string).map_err(|e| {
            BlazePolyError::Invalid(format!(
                "Invalid relayer api key address header encoding: {e}"
            ))
        })?;
        auth_headers.insert("RELAYER_API_KEY_ADDRESS", addr_value);

        let client = HttpClient::builder()
            .default_headers(auth_headers)
            .no_proxy()
            .http2_adaptive_window(true)
            .http2_initial_stream_window_size(512 * 1024)
            .tcp_nodelay(true)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .map_err(|e| {
                BlazePolyError::Internal(format!("Failed to build a http client for relayer: {e}"))
            })?;

        Ok(Self {
            client,
            base_url,
            deposit_wallet: None,
            signer,
        })
    }

    const fn base_url(&self) -> &Url {
        &self.base_url
    }

    const fn client(&self) -> &HttpClient {
        &self.client
    }

    #[must_use]
    pub const fn owner(&self) -> Address {
        self.signer.address()
    }

    pub(crate) const fn signer(&self) -> &PrivateKeySigner {
        &self.signer
    }

    #[must_use]
    pub const fn deposit_wallet(&self) -> Option<Address> {
        self.deposit_wallet
    }

    pub(crate) const fn set_deposit_wallet(&mut self, deposit_wallet: Address) {
        self.deposit_wallet = Some(deposit_wallet);
    }

    fn url(&self, endpoint: &str) -> Result<Url> {
        self.base_url().join(endpoint).map_err(|e| {
            BlazePolyError::Internal(format!(
                "failed to construct relayer url for endpoint {endpoint}, error: {e}"
            ))
        })
    }
    pub(crate) fn get(&self, endpoint: &str) -> Result<reqwest::RequestBuilder> {
        Ok(self.client().get(self.url(endpoint)?))
    }

    pub(crate) fn post(&self, endpoint: &str) -> Result<reqwest::RequestBuilder> {
        Ok(self.client().post(self.url(endpoint)?))
    }
}

#[derive(Debug, Deserialize)]
pub struct WalletNonceResponse {
    nonce: U256,
}

impl RelayerClient {
    pub async fn wallet_nonce(&self) -> Result<U256> {
        let url = format!("/nonce?address={}&type=WALLET", self.signer.address());

        let nonce = handle_response::<WalletNonceResponse>(self.get(&url)?.send().await)
            .await
            .map(|r| r.nonce)?;

        tracing::debug!(nonce = nonce.to_string(), "got wallet nonce response");

        Ok(nonce)
    }
}

pub async fn handle_response<T: DeserializeOwned>(
    resp: reqwest::Result<reqwest::Response>,
) -> Result<T> {
    match resp {
        Ok(resp) => match resp.error_for_status() {
            Ok(payload) => match payload.json::<T>().await {
                Ok(json) => Ok(json),
                Err(e) => Err(BlazePolyError::Internal(format!(
                    "Failed to parse response: {e}"
                ))),
            },
            Err(e) => Err(BlazePolyError::External(format!(
                "Failed to get response status: {e}"
            ))),
        },
        Err(e) => Err(BlazePolyError::External(format!(
            "Failed to get response: {e}"
        ))),
    }
}
