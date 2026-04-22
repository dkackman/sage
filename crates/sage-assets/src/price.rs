use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::UriError;

#[derive(Debug, Clone, Copy)]
pub struct XchUsdPrice {
    pub usd: f64,
}

impl XchUsdPrice {
    pub async fn fetch() -> Result<Self, UriError> {
        let response = price_client()?
            .get("https://api.coinmarketcap.com/data-api/v3/cryptocurrency/market-pairs/latest?slug=chia-network")
            .send()
            .await?
            .error_for_status()?
            .json::<CoinMarketCapMarketPairsResponse>()
            .await?;

        let market_pair = response
            .data
            .market_pairs
            .into_iter()
            .find(|pair| pair.price.is_finite() && pair.price > 0.0)
            .expect("CMC returned no usable XCH/USD market price");

        Ok(Self {
            usd: market_pair.price,
        })
    }
}

fn price_client() -> Result<Client, UriError> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!(
            "{}/{}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ))
        .build()?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoinMarketCapMarketPairsResponse {
    data: CoinMarketCapMarketPairsData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoinMarketCapMarketPairsData {
    market_pairs: Vec<CoinMarketCapMarketPair>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoinMarketCapMarketPair {
    price: f64,
}
