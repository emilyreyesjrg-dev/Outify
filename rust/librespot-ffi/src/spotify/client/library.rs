use reqwest::StatusCode;

use crate::spotify::error::SpotifyApiError;

use super::{SpotifyClient, REQUEST_TIMEOUT, SPOTIFY_API_URL};

impl SpotifyClient {
    pub async fn save_items(&self, uris: Vec<String>) -> Result<StatusCode, SpotifyApiError> {
        let token = self.load_token().await?;
        let token = token.ok_or_else(|| {
            SpotifyApiError::Generic("No account token present!".to_string())
        })?;

        let ids = uris.join(",");

        let res = self
            .client
            .put(format!("{}/v1/me/library", SPOTIFY_API_URL))
            .query(&[("uris", ids)])
            .header(reqwest::header::CONTENT_LENGTH, "0")
            .bearer_auth(token.access_token)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status().as_str().to_string();
            let body = res.text().await.unwrap_or_default();
            return Err(SpotifyApiError::Generic(format!(
                "save_items failed with status {status}: {body}"
            )));
        }

        Ok(res.status())
    }

    pub async fn delete_items(&self, uris: Vec<String>) -> Result<StatusCode, SpotifyApiError> {
        let token = self.load_token().await?;
        let token = token.ok_or_else(|| {
            SpotifyApiError::Generic("No account token present!".to_string())
        })?;

        let ids = uris.join(",");

        let res = self
            .client
            .delete(format!("{}/v1/me/library", SPOTIFY_API_URL))
            .query(&[("uris", ids)])
            .header(reqwest::header::CONTENT_LENGTH, "0")
            .bearer_auth(token.access_token)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status().as_str().to_string();
            let body = res.text().await.unwrap_or_default();
            return Err(SpotifyApiError::Generic(format!(
                "delete_items failed with status {status}: {body}"
            )));
        }

        Ok(res.status())
    }
}
