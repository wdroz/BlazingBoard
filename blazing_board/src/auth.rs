#![cfg(feature = "server")]

use std::env;

use axum::{
    extract::Query,
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{ACCEPT, COOKIE, SET_COOKIE, USER_AGENT},
    },
    response::{IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Duration, Utc};
use firestore::paths;
use oauth2::{
    AuthType, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, TokenResponse, TokenUrl, basic::BasicClient,
    reqwest::ClientBuilder,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{backend::get_client_db, models::UserProfile};

const GITHUB_AUTHORIZE_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_URL: &str = "https://api.github.com/user";
const DEFAULT_CALLBACK_URL: &str = "https://blazingboard.ch/auth/github/callback";
const OAUTH_STATE_COOKIE: &str = "bb_oauth_state";
const SESSION_COOKIE: &str = "bb_session";
const OAUTH_STATES_COLLECTION: &str = "oauth_states";
const SESSIONS_COLLECTION: &str = "sessions";
const USERS_COLLECTION: &str = "users";

#[derive(Debug, Clone, Deserialize, Serialize)]
struct OAuthStateRecord {
    pkce_verifier: String,
    #[serde(with = "firestore::serialize_as_timestamp")]
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SessionRecord {
    user_id: String,
    #[serde(with = "firestore::serialize_as_timestamp")]
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GithubCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubUser {
    id: i64,
    login: String,
    name: Option<String>,
    avatar_url: String,
}

struct OAuthConfig {
    client_id: String,
    client_secret: String,
    callback_url: String,
}

impl OAuthConfig {
    fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();

        Ok(Self {
            client_id: env::var("CLIENT_ID").map_err(|_| "CLIENT_ID is not set".to_string())?,
            client_secret: env::var("CLIENT_SECRET")
                .map_err(|_| "CLIENT_SECRET is not set".to_string())?,
            callback_url: env::var("GITHUB_CALLBACK_URL")
                .unwrap_or_else(|_| DEFAULT_CALLBACK_URL.to_string()),
        })
    }

    fn secure_cookies(&self) -> bool {
        self.callback_url.starts_with("https://")
    }
}

pub(crate) async fn github_login() -> Response {
    match begin_github_login().await {
        Ok(response) => response,
        Err(message) => {
            eprintln!("Unable to start GitHub login: {message}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "GitHub login is temporarily unavailable.",
            )
                .into_response()
        }
    }
}

async fn begin_github_login() -> Result<Response, String> {
    let config = OAuthConfig::from_env()?;
    let client = oauth_client(&config)?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorization_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(pkce_challenge)
        .url();

    let record = OAuthStateRecord {
        pkce_verifier: pkce_verifier.secret().to_string(),
        expires_at: Utc::now() + Duration::minutes(10),
    };
    let state = csrf_token.secret();

    get_client_db()
        .await
        .fluent()
        .update()
        .in_col(OAUTH_STATES_COLLECTION)
        .document_id(token_hash(state))
        .object(&record)
        .execute::<()>()
        .await
        .map_err(|error| format!("Unable to store OAuth state: {error}"))?;

    let mut response = Redirect::to(authorization_url.as_str()).into_response();
    append_set_cookie(
        &mut response,
        &build_cookie(OAUTH_STATE_COOKIE, state, 10 * 60, config.secure_cookies()),
    )?;
    Ok(response)
}

pub(crate) async fn github_callback(
    Query(query): Query<GithubCallbackQuery>,
    headers: HeaderMap,
) -> Response {
    match complete_github_login(query, &headers).await {
        Ok(mut response) => {
            clear_oauth_cookie(&mut response);
            response
        }
        Err(message) => {
            eprintln!("GitHub login callback failed: {message}");
            let mut response = Redirect::to("/?login=failed").into_response();
            clear_oauth_cookie(&mut response);
            response
        }
    }
}

async fn complete_github_login(
    query: GithubCallbackQuery,
    headers: &HeaderMap,
) -> Result<Response, String> {
    if let Some(error) = query.error {
        return Err(format!("GitHub returned an authorization error: {error}"));
    }

    let code = query.code.ok_or_else(|| "Missing OAuth code".to_string())?;
    let returned_state = query
        .state
        .ok_or_else(|| "Missing OAuth state".to_string())?;
    let cookie_state = cookie_value(headers, OAUTH_STATE_COOKIE)
        .ok_or_else(|| "Missing OAuth state cookie".to_string())?;

    if returned_state != cookie_state {
        return Err("OAuth state does not match".to_string());
    }

    let db = get_client_db().await;
    let state_document_id = token_hash(&returned_state);
    let state_record = db
        .fluent()
        .select()
        .by_id_in(OAUTH_STATES_COLLECTION)
        .obj::<OAuthStateRecord>()
        .one(&state_document_id)
        .await
        .map_err(|error| format!("Unable to load OAuth state: {error}"))?
        .ok_or_else(|| "OAuth state has already been used or expired".to_string())?;

    db.fluent()
        .delete()
        .from(OAUTH_STATES_COLLECTION)
        .document_id(&state_document_id)
        .execute()
        .await
        .map_err(|error| format!("Unable to consume OAuth state: {error}"))?;

    if state_record.expires_at <= Utc::now() {
        return Err("OAuth state expired".to_string());
    }

    let config = OAuthConfig::from_env()?;
    let client = oauth_client(&config)?;
    let http_client = ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("Unable to create OAuth HTTP client: {error}"))?;
    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(PkceCodeVerifier::new(state_record.pkce_verifier))
        .request_async(&http_client)
        .await
        .map_err(|_| "GitHub token exchange failed".to_string())?;

    let github_user = http_client
        .get(GITHUB_USER_URL)
        .bearer_auth(token.access_token().secret())
        .header(USER_AGENT, "BlazingBoard")
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| "Unable to request the GitHub profile".to_string())?
        .error_for_status()
        .map_err(|_| "GitHub rejected the profile request".to_string())?
        .json::<GithubUser>()
        .await
        .map_err(|_| "Unable to decode the GitHub profile".to_string())?;

    let user_id = upsert_github_user(github_user).await?;
    let session_token = CsrfToken::new_random().secret().to_string();
    let session = SessionRecord {
        user_id,
        expires_at: Utc::now() + Duration::days(30),
    };

    db.fluent()
        .update()
        .in_col(SESSIONS_COLLECTION)
        .document_id(token_hash(&session_token))
        .object(&session)
        .execute::<()>()
        .await
        .map_err(|error| format!("Unable to store the session: {error}"))?;

    let mut response = Redirect::to("/?login=success").into_response();
    append_set_cookie(
        &mut response,
        &build_cookie(
            SESSION_COOKIE,
            &session_token,
            30 * 24 * 60 * 60,
            config.secure_cookies(),
        ),
    )?;
    Ok(response)
}

async fn upsert_github_user(github_user: GithubUser) -> Result<String, String> {
    let db = get_client_db().await;
    let github_id = github_user.id.to_string();
    let existing = db
        .fluent()
        .select()
        .by_id_in(USERS_COLLECTION)
        .obj::<UserProfile>()
        .one(&github_id)
        .await
        .map_err(|error| format!("Unable to load the user profile: {error}"))?;
    let now = Utc::now();
    if let Some(existing) = existing {
        let profile = UserProfile {
            login: github_user.login,
            display_name: github_user.name,
            avatar_url: github_user.avatar_url,
            last_login_at: now,
            ..existing
        };
        db.fluent()
            .update()
            .fields(paths!(UserProfile::{
                login,
                display_name,
                avatar_url,
                last_login_at
            }))
            .in_col(USERS_COLLECTION)
            .document_id(&github_id)
            .object(&profile)
            .execute::<()>()
            .await
            .map_err(|error| format!("Unable to update the user profile: {error}"))?;
    } else {
        let profile = UserProfile {
            github_id: github_id.clone(),
            login: github_user.login,
            display_name: github_user.name,
            avatar_url: github_user.avatar_url,
            created_at: now,
            last_login_at: now,
            total_runs: 0,
            best_wpm: 0.0,
            best_accuracy: 0.0,
            best_score: 0,
        };
        db.fluent()
            .update()
            .in_col(USERS_COLLECTION)
            .document_id(&github_id)
            .object(&profile)
            .execute::<()>()
            .await
            .map_err(|error| format!("Unable to create the user profile: {error}"))?;
    }

    Ok(github_id)
}

pub(crate) async fn github_logout(headers: HeaderMap) -> Response {
    if let Some(session_token) = cookie_value(&headers, SESSION_COOKIE) {
        let _ = get_client_db()
            .await
            .fluent()
            .delete()
            .from(SESSIONS_COLLECTION)
            .document_id(token_hash(&session_token))
            .execute()
            .await;
    }

    let mut response = Redirect::to("/").into_response();
    append_clear_cookie(&mut response, SESSION_COOKIE);
    response
}

pub(crate) async fn authenticated_user_id(headers: &HeaderMap) -> Result<Option<String>, String> {
    let Some(session_token) = cookie_value(headers, SESSION_COOKIE) else {
        return Ok(None);
    };

    let document_id = token_hash(&session_token);
    let db = get_client_db().await;
    let session = db
        .fluent()
        .select()
        .by_id_in(SESSIONS_COLLECTION)
        .obj::<SessionRecord>()
        .one(&document_id)
        .await
        .map_err(|error| format!("Unable to load session: {error}"))?;

    let Some(session) = session else {
        return Ok(None);
    };
    if session.expires_at <= Utc::now() {
        let _ = db
            .fluent()
            .delete()
            .from(SESSIONS_COLLECTION)
            .document_id(document_id)
            .execute()
            .await;
        return Ok(None);
    }

    Ok(Some(session.user_id))
}

fn oauth_client(
    config: &OAuthConfig,
) -> Result<
    BasicClient<
        oauth2::EndpointSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointSet,
    >,
    String,
> {
    Ok(BasicClient::new(ClientId::new(config.client_id.clone()))
        .set_client_secret(ClientSecret::new(config.client_secret.clone()))
        .set_auth_uri(
            AuthUrl::new(GITHUB_AUTHORIZE_URL.to_string())
                .map_err(|error| format!("Invalid GitHub authorization URL: {error}"))?,
        )
        .set_token_uri(
            TokenUrl::new(GITHUB_TOKEN_URL.to_string())
                .map_err(|error| format!("Invalid GitHub token URL: {error}"))?,
        )
        .set_redirect_uri(
            RedirectUrl::new(config.callback_url.clone())
                .map_err(|error| format!("Invalid GitHub callback URL: {error}"))?,
        )
        .set_auth_type(AuthType::RequestBody))
}

pub(crate) fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find_map(|(cookie_name, value)| (cookie_name == name).then(|| value.to_string()))
}

fn build_cookie(name: &str, value: &str, max_age: i64, secure: bool) -> String {
    let secure_attribute = if secure { "; Secure" } else { "" };
    format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure_attribute}")
}

fn append_set_cookie(response: &mut Response, cookie: &str) -> Result<(), String> {
    let value = HeaderValue::from_str(cookie)
        .map_err(|error| format!("Unable to build session cookie: {error}"))?;
    response.headers_mut().append(SET_COOKIE, value);
    Ok(())
}

fn append_clear_cookie(response: &mut Response, name: &str) {
    if let Ok(value) = HeaderValue::from_str(&format!(
        "{name}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
    )) {
        response.headers_mut().append(SET_COOKIE, value);
    }
}

fn clear_oauth_cookie(response: &mut Response) {
    append_clear_cookie(response, OAUTH_STATE_COOKIE);
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header::COOKIE};

    use super::{build_cookie, cookie_value, token_hash};

    #[test]
    fn reads_named_cookie_without_confusing_neighbors() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_static("other=one; bb_session=secret-token; last=three"),
        );

        assert_eq!(
            cookie_value(&headers, "bb_session").as_deref(),
            Some("secret-token")
        );
        assert_eq!(cookie_value(&headers, "session"), None);
    }

    #[test]
    fn production_cookie_has_security_attributes() {
        let cookie = build_cookie("bb_session", "secret", 60, true);

        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn hashes_tokens_before_storage() {
        let hash = token_hash("secret");

        assert_ne!(hash, "secret");
        assert_eq!(hash.len(), 64);
    }
}
