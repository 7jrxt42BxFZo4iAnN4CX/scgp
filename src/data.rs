use std::fmt;

use chrono::{DateTime, Utc};
use ordered_float::NotNan;

#[derive(Debug)]
pub enum FetchError {
    InvalidTicker(String),
    NetworkError(String),
    RateLimited,
    Other(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::InvalidTicker(t) => write!(f, "Ticker not found: {}", t),
            FetchError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            FetchError::RateLimited => write!(f, "Rate limited — please wait and try again"),
            FetchError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for FetchError {}

#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub time: DateTime<Utc>,
    pub open: NotNan<f64>,
    pub high: NotNan<f64>,
    pub low: NotNan<f64>,
    pub close: NotNan<f64>,
    pub volume: NotNan<f64>,
}

impl Quote {
    pub fn new(
        time: DateTime<Utc>,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
    ) -> Option<Self> {
        Some(Self {
            time,
            open: NotNan::new(open).ok()?,
            high: NotNan::new(high).ok()?,
            low: NotNan::new(low).ok()?,
            close: NotNan::new(close).ok()?,
            volume: NotNan::new(volume).ok()?,
        })
    }
}

pub fn yahoo_to_quotes(yahoo_quotes: Vec<yahoo_finance_api::Quote>) -> Vec<Quote> {
    let mut quotes: Vec<Quote> = yahoo_quotes
        .into_iter()
        .filter_map(|yq| {
            let time = DateTime::from_timestamp(i64::try_from(yq.timestamp).ok()?, 0)?;
            Quote::new(time, yq.open, yq.high, yq.low, yq.close, yq.volume as f64)
        })
        .collect();
    quotes.sort_by_key(|q| q.time);
    quotes
}

fn classify_yahoo_error(e: yahoo_finance_api::YahooError, ticker: &str) -> FetchError {
    tracing::error!(ticker, error = %e, "Yahoo API error");
    use yahoo_finance_api::YahooError;
    match e {
        YahooError::NoResult
        | YahooError::NoQuotes
        | YahooError::DataInconsistency
        | YahooError::ApiError(_) => FetchError::InvalidTicker(ticker.to_string()),
        YahooError::ConnectionFailed(_) | YahooError::FetchFailed(_) => {
            FetchError::NetworkError(e.to_string())
        }
        YahooError::TooManyRequests(_) => FetchError::RateLimited,
        _ => FetchError::Other(e.to_string()),
    }
}

pub async fn fetch_quotes(
    ticker: &str,
    interval: &str,
    range: &str,
) -> Result<Vec<Quote>, FetchError> {
    tracing::info!(ticker, interval, range, "Fetching quotes");

    let connector =
        yahoo_finance_api::YahooConnector::new().map_err(|e| FetchError::Other(e.to_string()))?;

    let response = connector
        .get_quote_range(ticker, interval, range)
        .await
        .map_err(|e| classify_yahoo_error(e, ticker))?;

    let yahoo_quotes = response
        .quotes()
        .map_err(|e| classify_yahoo_error(e, ticker))?;

    let quotes = yahoo_to_quotes(yahoo_quotes);
    if quotes.is_empty() {
        tracing::warn!(ticker, "No quotes returned");
        return Err(FetchError::InvalidTicker(ticker.to_string()));
    }

    tracing::info!(ticker, count = quotes.len(), "Quotes loaded");
    Ok(quotes)
}
