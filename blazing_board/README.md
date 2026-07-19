# BlazingBoard

A daily typing challenge built with Dioxus. Guests can play without an account. Signing in with GitHub creates a private profile and saves typing results in Firestore.

## Development

Copy `.env.example` to `.env` and fill in the values. For local Firestore authentication, either use Google Application Default Credentials or set `IAMTHEDEV=1` and provide an untracked `key.json` service-account file.

```bash
dx serve --web --fullstack
```

GitHub OAuth Apps support only one callback URL. Use a separate development OAuth App with:

```text
http://127.0.0.1:8080/auth/github/callback
```

Set `GITHUB_CALLBACK_URL` to the same value. The production OAuth App callback must be:

```text
https://blazingboard.ch/auth/github/callback
```

## Authentication

The server requests no GitHub scopes and reads only the public identity returned by `GET /user`. GitHub access tokens are discarded immediately after login.

Sessions are random opaque tokens. Only a SHA-256 hash is stored in Firestore; the browser receives the token in a 30-day `HttpOnly`, `Secure`, `SameSite=Lax` cookie. OAuth state uses PKCE and expires after ten minutes.

## Firestore data

The existing Firestore database is used for:

```text
users/{github_id}
users/{github_id}/typing_results/{run_id}
sessions/{session_token_hash}
oauth_states/{oauth_state_hash}
```

Typing history is private and is accessed only through authenticated server functions. WPM, accuracy, and score are recomputed on the server. The score is `round(WPM × accuracy)`.

The application checks expiration on every request. Firestore TTL policies are also recommended to remove expired documents automatically:

```bash
gcloud firestore fields ttls update expires_at \
  --collection-group=sessions \
  --enable-ttl

gcloud firestore fields ttls update expires_at \
  --collection-group=oauth_states \
  --enable-ttl
```

Add the appropriate project and database flags if they are not already configured in `gcloud`.

## Verification

```bash
cargo test --features server
cargo check --features web
cargo check --features server
```
