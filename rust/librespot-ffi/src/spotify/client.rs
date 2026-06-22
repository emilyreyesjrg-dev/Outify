use librespot_oauth::{OAuthClient, OAuthClientBuilder, OAuthToken};
use oauth2::{AuthorizationCode, PkceCodeVerifier};
use once_cell::sync::OnceCell;
use reqwest::{Client, StatusCode, header};
use std::{
    collections::HashMap,
    fs::OpenOptions,
    os::unix::fs::OpenOptionsExt,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

use crate::types::{
    requests::{
        AddItemRequest, CreatePlaylistRequest, RemoveItem, RemoveItemRequest,
        TransferPlaybackRequest,
    },
    responses::{
        ArtistsOrTracksPage, CreatePlaylistResponse, CurrentUserResponse, DevicesResponse,
    },
};
use crate::spotify::{
    error::SpotifyApiError,
    search::extract_all_uris,
    token::{TokenResponse, WebApiToken},
};

const SPOTIFY_API_URL: &str = "https://api.spotify.com";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const SPOTIFY_OAUTH_CALLBACK_URI: &str = "http://127.0.0.1:5588/account/login";
const SPOTIFY_OAUTH_SCOPES: &[&str] = &[
    "streaming",
    "user-read-private",
    "user-read-email",
    "user-top-read",
    "user-library-modify",
    "user-library-read",
    "user-follow-modify",
    "user-read-playback-state",
    "playlist-modify-private",
    "playlist-modify-public",
];

static SPOTIFY_CLIENT: OnceCell<SpotifyClient> = OnceCell::new();

/// OAuth state for SpotifyClient's user authentication flow
pub struct OAuthState {
    pub oauth_client: OAuthClient,
    pub pkce_verifier: Option<PkceCodeVerifier>,
    pub created_at: Instant,
}

pub struct SpotifyClient {
    client_id: Mutex<String>,
    client_secret: Mutex<String>,
    client: Client,
    token: Arc<RwLock<Option<WebApiToken>>>,
    oauth_state: Arc<RwLock<Option<OAuthState>>>,
}

impl SpotifyClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client_id: Mutex::new(client_id),
            client_secret: Mutex::new(client_secret),
            client: Client::builder()
                .pool_idle_timeout(Duration::from_secs(90))
                .build()
                .expect("failed to build client"),
            token: Arc::new(RwLock::new(None)),
            oauth_state: Arc::new(RwLock::new(None)),
        }
    }

    pub fn update_credentials(&self, client_id: String, client_secret: String) {
        *self.client_id.lock().unwrap() = client_id;
        *self.client_secret.lock().unwrap() = client_secret;
    }

    pub async fn search(
        &self,
        query: &str,
        types: &str,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<String>, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let mut params = vec![("q", query.to_string()), ("type", types.to_string())];

        if let Some(l) = limit {
            params.push(("limit", l.to_string()));
        }

        if let Some(o) = offset {
            params.push(("offset", o.to_string()));
        }

        let res = self
            .client
            .get(format!("{}/v1/search", SPOTIFY_API_URL))
            .query(&params)
            .bearer_auth(token.access_token)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status().as_str().to_string();
            let body = res.text().await.unwrap_or_default();
            return Err(SpotifyApiError::Generic(format!(
                "search failed with status {status}, query '{query}': {body}"
            )));
        }

        let text = res.text().await?;

        let parsed: crate::spotify::search::SearchResponse = serde_json::from_str(&text)?;

        let uris = extract_all_uris(parsed);

        Ok(uris)
    }

    // Gets current users profile
    pub async fn get_current_user(&self) -> Result<CurrentUserResponse, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let res = self
            .client
            .get(format!("{}/v1/me", SPOTIFY_API_URL))
            .bearer_auth(token.access_token)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status().as_str().to_string();
            let body = res.text().await.unwrap_or_default();
            return Err(SpotifyApiError::Generic(format!(
                "Request failed with status code: {}. Body: {}",
                status, body
            )));
        }

        let text = res.text().await?;

        let data: CurrentUserResponse = serde_json::from_str(&text)?;

        Ok(data)
    }

    // Saves tracks/episodes/albums/..
    pub async fn save_items(&self, uris: Vec<String>) -> Result<StatusCode, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let ids = uris.join(",");

        let res = self
            .client
            .put(format!("{}/v1/me/library", SPOTIFY_API_URL))
            .query(&[("uris", ids)])
            .header(header::CONTENT_LENGTH, "0")
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

    pub async fn add_to_playlist(
        &self,
        playlist_id: String,
        uris: Vec<String>,
    ) -> Result<StatusCode, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let body = AddItemRequest {
            uris,
            position: Some(0),
        };

        let res = self
            .client
            .post(format!(
                "{}/v1/playlists/{}/items",
                SPOTIFY_API_URL, playlist_id
            ))
            .bearer_auth(token.access_token)
            .json(&body)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status().as_str().to_string();
            let body = res.text().await.unwrap_or_default();
            return Err(SpotifyApiError::Generic(format!(
                "add_to_playlist failed with status {status}: {body}"
            )));
        }

        Ok(res.status())
    }

    // Deletes tracks/episodes/albums/..
    pub async fn delete_items(&self, uris: Vec<String>) -> Result<StatusCode, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let ids = uris.join(",");

        let res = self
            .client
            .delete(format!("{}/v1/me/library", SPOTIFY_API_URL))
            .query(&[("uris", ids)])
            .header(header::CONTENT_LENGTH, "0")
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

    pub async fn delete_from_playlist(
        &self,
        playlist_id: String,
        uris: Vec<String>,
    ) -> Result<StatusCode, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let body = RemoveItemRequest {
            items: uris.into_iter().map(|uri| RemoveItem { uri }).collect(),
        };

        let res = self
            .client
            .delete(format!(
                "{}/v1/playlists/{}/items",
                SPOTIFY_API_URL, playlist_id
            ))
            .bearer_auth(token.access_token)
            .json(&body)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status().as_str().to_string();
            let body = res.text().await.unwrap_or_default();
            return Err(SpotifyApiError::Generic(format!(
                "delete_from_playlist failed with status {status}: {body}"
            )));
        }

        Ok(res.status())
    }

    pub async fn get_devices(&self) -> Result<DevicesResponse, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let url = format!("{}/v1/me/player/devices", SPOTIFY_API_URL);

        let res = self
            .client
            .get(&url)
            .bearer_auth(&token.access_token)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;

        if res.status() == StatusCode::UNAUTHORIZED {
            let new_token = self.refresh_token(&token).await?;
            let res = self
                .client
                .get(&url)
                .bearer_auth(new_token.access_token)
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await?;
            let data = check_response_json::<DevicesResponse>("get_devices", res).await?;
            return Ok(data);
        }

        let data = check_response_json::<DevicesResponse>("get_devices", res).await?;

        Ok(data)
    }

    pub async fn transfer_playback(
        &self,
        device_id: String,
    ) -> Result<StatusCode, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let body = TransferPlaybackRequest {
            device_ids: vec![device_id],
        };

        let res = self
            .client
            .put(format!("{}/v1/me/player", SPOTIFY_API_URL))
            .bearer_auth(token.access_token)
            .timeout(REQUEST_TIMEOUT)
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status().as_str().to_string();
            let body = res.text().await.unwrap_or_default();
            return Err(SpotifyApiError::Generic(format!(
                "transfer_playback failed with status {status}: {body}"
            )));
        }

        Ok(res.status())
    }

    // Gets users top picks in given category.
    // Accepted: artists, tracks
    // Default: artists
    pub async fn get_top(
        &self,
        request_type: Option<String>,
        time_range: String,
    ) -> Result<ArtistsOrTracksPage, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let request_type = request_type.unwrap_or_else(|| "artists".to_string());

        let url = format!(
            "{}/v1/me/top/{}?time_range={}",
            SPOTIFY_API_URL, request_type, time_range
        );

        let res = self
            .client
            .get(&url)
            .bearer_auth(&token.access_token)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;

        if res.status() == StatusCode::UNAUTHORIZED {
            let new_token = self.refresh_token(&token).await?;
            let res = self
                .client
                .get(&url)
                .bearer_auth(new_token.access_token)
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await?;
            let data = check_response_json::<ArtistsOrTracksPage>("get_top", res).await?;
            return Ok(data);
        }

        let data = check_response_json::<ArtistsOrTracksPage>("get_top", res).await?;

        Ok(data)
    }

    pub async fn create_playlist(
        &self,
        name: String,
        description: Option<String>,
        public: bool,
        collaborative: bool,
    ) -> Result<CreatePlaylistResponse, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let body = CreatePlaylistRequest {
            name,
            public,
            collaborative,
            description,
        };

        let url = format!("{}/v1/me/playlists", SPOTIFY_API_URL);

        let res = self
            .client
            .post(&url)
            .json(&body)
            .bearer_auth(&token.access_token)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;

        if res.status() == StatusCode::UNAUTHORIZED {
            let new_token = self.refresh_token(&token).await?;
            let res = self
                .client
                .post(&url)
                .json(&body)
                .bearer_auth(new_token.access_token)
                .timeout(REQUEST_TIMEOUT)
                .send()
                .await?;
            let data =
                check_response_json::<CreatePlaylistResponse>("create_playlist", res).await?;
            return Ok(data);
        }

        let data = check_response_json::<CreatePlaylistResponse>("create_playlist", res).await?;

        Ok(data)
    }

    pub async fn modify_playlist(
        &self,
        id: String,
        name: String,
        description: Option<String>,
        public: bool,
        collaborative: bool,
    ) -> Result<StatusCode, SpotifyApiError> {
        let token = match self.load_token().await {
            Ok(o) => match o {
                Some(t) => t,
                None => {
                    return Err(SpotifyApiError::Generic(
                        "No account token present!".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(e);
            }
        };

        let body = CreatePlaylistRequest {
            name,
            public,
            collaborative,
            description,
        };

        let res = self
            .client
            .put(format!("{}/v1/playlists/{}", SPOTIFY_API_URL, id))
            .json(&body)
            .bearer_auth(token.access_token)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status().as_str().to_string();
            let body = res.text().await.unwrap_or_default();
            return Err(SpotifyApiError::Generic(format!(
                "modify_playlist failed with status {status}: {body}"
            )));
        }

        Ok(res.status())
    }

    pub async fn get_oauth_url(&self) -> String {
        format!("{}", SPOTIFY_OAUTH_CALLBACK_URI)
    }

    /// Starts the OAuth flow and returns the authorization URL
    pub async fn start_oauth_flow(&self) -> Result<String, SpotifyApiError> {
        // Build OAuthClient using librespot_oauth
        let client_id = self.client_id.lock().unwrap().clone();
        let oauth_client = OAuthClientBuilder::new(
            &client_id,
            SPOTIFY_OAUTH_CALLBACK_URI,
            SPOTIFY_OAUTH_SCOPES.to_vec(),
        )
        .build()
        .map_err(|e| SpotifyApiError::Generic(format!("Failed to build OAuth client: {}", e)))?;

        // Get authorization URL and PKCE verifier
        let (auth_url, pkce_verifier) = oauth_client.set_auth_url();

        let state = OAuthState {
            oauth_client,
            pkce_verifier: Some(pkce_verifier),
            created_at: Instant::now(),
        };

        let mut oauth_state_guard = self.oauth_state.write().await;
        *oauth_state_guard = Some(state);

        debug!("oauth flow started with url: {auth_url}");
        Ok(auth_url.to_string())
    }

    /// Completes the OAuth flow by exchanging the authorization code for tokens
    pub async fn complete_oauth_flow(&self, code: String) -> Result<WebApiToken, SpotifyApiError> {
        let mut oauth_state_guard = self.oauth_state.write().await;
        let state = oauth_state_guard.as_mut().ok_or(SpotifyApiError::Generic(
            "OAuth flow not started. Call start_oauth_flow first.".to_string(),
        ))?;

        if state.created_at.elapsed() > Duration::from_secs(600) {
            error!("oauth state expired");
            return Err(SpotifyApiError::Generic(
                "OAuth state expired. Please restart the flow.".to_string(),
            ));
        }

        let pkce_verifier = state.pkce_verifier.take().ok_or(SpotifyApiError::Generic(
            "PKCE verifier not found. OAuth flow may have already completed.".to_string(),
        ))?;
        let oauth_client = &state.oauth_client;

        // Use the OAuthClient to exchange code for token
        let auth_code = AuthorizationCode::new(code);
        let token_response: OAuthToken = oauth_client
            .get_access_token_with_verifier_async(pkce_verifier, auth_code)
            .await
            .map_err(|e| {
                error!("oauth token exchange failed: {e}");
                SpotifyApiError::Generic(format!("Token exchange failed: {e}"))
            })?;

        // Calculate remaining seconds until expiration
        let now = Instant::now();
        let expires_in = if token_response.expires_at > now {
            token_response.expires_at.duration_since(now).as_secs()
        } else {
            0
        };

        let new_token = WebApiToken::new(
            token_response.access_token,
            token_response.refresh_token,
            expires_in,
            token_response.scopes.join(" "),
        );

        let mut token_guard = self.token.write().await;
        *token_guard = Some(new_token.clone());

        // Clear OAuth state after successful exchange
        drop(oauth_state_guard);
        let mut oauth_state_guard = self.oauth_state.write().await;
        *oauth_state_guard = None;

        debug!("oauth flow completed");

        match self.save_token(&new_token).await {
            Ok(_) => debug!("oauth token saved to account.json"),
            Err(e) => {
                error!("oauth token save failed: {e}");
            }
        };

        Ok(new_token)
    }

    pub async fn save_token(&self, token: &WebApiToken) -> Result<(), SpotifyApiError> {
        let mut path = crate::FILES_DIR
            .get()
            .ok_or_else(|| SpotifyApiError::Generic("Android file path is not set!".to_string()))?
            .clone();

        path.push("account.json");

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;

        let json = serde_json::to_string(token).map_err(|e| {
            SpotifyApiError::Generic(format!("Failed to serialize WebApiToken: {e}"))
        })?;

        std::io::Write::write_all(&mut file, json.as_bytes())?;
        Ok(())
    }

    pub fn remove_token(&self) -> Result<(), SpotifyApiError> {
        let mut path = crate::FILES_DIR
            .get()
            .ok_or_else(|| SpotifyApiError::Generic("Android file path is not set!".to_string()))?
            .clone();

        path.push("account.json");

        match std::fs::remove_file(path) {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("account.json removal failed: {e}");
                Err(SpotifyApiError::IO(e))
            }
        }
    }

    /// Check if OAuth token exists (user is authenticated with Spotify)
    pub async fn is_oauth_authenticated(&self) -> bool {
        match self.load_token().await {
            Ok(Some(_)) => true,
            _ => false,
        }
    }

    // Returns scopes seperated by space
    pub async fn get_scope(&self) -> Option<String> {
        match self.load_token().await {
            Ok(Some(t)) => Some(t.scope),
            _ => None,
        }
    }

    /// Loads the token from the session's cache if available
    pub async fn load_token(&self) -> Result<Option<WebApiToken>, SpotifyApiError> {
        let mut path = crate::FILES_DIR
            .get()
            .ok_or_else(|| SpotifyApiError::Generic("Android file path is not set!".to_string()))?
            .clone();

        path.push("account.json");

        let token = match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str::<WebApiToken>(&contents).map_err(|e| {
                SpotifyApiError::Generic(format!("Failed to parse token JSON: {e}"))
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(SpotifyApiError::IO(e)),
        };

        if token.is_expired() {
            let refreshed = self.refresh_token(&token).await?;
            return Ok(Some(refreshed));
        }

        Ok(Some(token))
    }

    async fn refresh_token(&self, token: &WebApiToken) -> Result<WebApiToken, SpotifyApiError> {
        let mut form = HashMap::new();
        form.insert("grant_type", "refresh_token");
        form.insert("refresh_token", &token.refresh_token);
        let client_id = self.client_id.lock().unwrap().clone();
        form.insert("client_id", &client_id);

        let response = self
            .client
            .post("https://accounts.spotify.com/api/token")
            .form(&form)
            .send()
            .await?;

        let response = check_response_json::<TokenResponse>("refresh_token", response).await?;

        let new_token = WebApiToken::from(response, Some(&token.refresh_token));
        self.save_token(&new_token).await?;
        Ok(new_token)
    }
}

async fn check_response_json<T: serde::de::DeserializeOwned>(
    method: &str,
    res: reqwest::Response,
) -> Result<T, SpotifyApiError> {
    if !res.status().is_success() {
        let status = res.status().as_str().to_string();
        let body = res.text().await.unwrap_or_default();
        return Err(SpotifyApiError::Generic(format!(
            "{method} failed with status {status}: {body}"
        )));
    }
    let text = res.text().await?;
    let data = serde_json::from_str(&text)?;
    Ok(data)
}

pub fn init_client(client_id: String, client_secret: String) {
    let client = SpotifyClient::new(client_id, client_secret);
    SPOTIFY_CLIENT.set(client);
}

pub fn get_client() -> &'static SpotifyClient {
    SPOTIFY_CLIENT
        .get()
        .expect("SpotifyClient not initialized!")
}

pub fn update_client(client_id: String, client_secret: String) {
    if let Some(client) = SPOTIFY_CLIENT.get() {
        client.update_credentials(client_id, client_secret);
        info!("spotify client credentials updated");
    }
}
