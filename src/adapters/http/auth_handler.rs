use crate::{
    adapters::http::{cookies::{session_cookie, clear_session_cookie}, helpers::{validate_email, upsert_hackclub_user, create_session, current_user, session_token, token_hash}},
    adapters::http::AppState,
    domain::{LoginQuery, OAuthCallbackQuery, OAuthState},
    error::{ApiError, ApiResult},
};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Redirect, Response},
};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

pub async fn hackclub_login(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LoginQuery>,
) -> ApiResult<Redirect> {
    let email = query.email.map(|email| email.trim().to_lowercase());
    if let Some(email) = &email {
        validate_email(email)?;
    }
    let oauth_state = Uuid::new_v4().to_string();
    state
        .cache
        .set_json(
            &format!("oauth:hackclub:{oauth_state}"),
            &OAuthState {
                email: email.clone(),
                user_id: None,
            },
            Duration::from_mins(10),
        )
        .await;
    Ok(Redirect::to(
        &state
            .providers
            .hackclub_authorize_url(&oauth_state, email.as_deref()),
    ))
}

pub async fn hackclub_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> ApiResult<Response> {
    if query.error.is_some() {
        return Err(ApiError::Unauthorized(
            "Hack Club sign-in was cancelled or denied".into(),
        ));
    }
    let state_key = query
        .state
        .ok_or_else(|| ApiError::BadRequest("missing OAuth state".into()))?;
    let state_cache_key = format!("oauth:hackclub:{state_key}");
    let _oauth_state: OAuthState = state
        .cache
        .get_json(&state_cache_key)
        .await
        .ok_or_else(|| ApiError::Unauthorized("sign-in link expired; start again".into()))?;
    state.cache.delete(&state_cache_key).await;
    let code = query
        .code
        .ok_or_else(|| ApiError::BadRequest("missing OAuth code".into()))?;
    let identity = state.providers.hackclub_identity(&code).await?;
    let user_id = upsert_hackclub_user(&state.db, identity).await?;
    let token = create_session(&state.db, user_id).await?;
    let mut response = Redirect::to("/dashboard").into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, session_cookie(&token, state.cookie_secure)?);
    Ok(response)
}

pub async fn hackatime_login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Redirect> {
    let user_id = current_user(&state, &headers).await?;
    let oauth_state = Uuid::new_v4().to_string();
    state
        .cache
        .set_json(
            &format!("oauth:hackatime:{oauth_state}"),
            &OAuthState {
                email: None,
                user_id: Some(user_id),
            },
            Duration::from_mins(10),
        )
        .await;
    Ok(Redirect::to(
        &state.providers.hackatime_authorize_url(&oauth_state),
    ))
}

pub async fn hackatime_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> ApiResult<Redirect> {
    if query.error.is_some() {
        return Err(ApiError::Unauthorized(
            "Hackatime connection was cancelled or denied".into(),
        ));
    }
    let state_key = query
        .state
        .ok_or_else(|| ApiError::BadRequest("missing OAuth state".into()))?;
    let state_cache_key = format!("oauth:hackatime:{state_key}");
    let oauth_state: OAuthState = state
        .cache
        .get_json(&state_cache_key)
        .await
        .ok_or_else(|| {
            ApiError::Unauthorized("Hackatime connection link expired; start again".into())
        })?;
    state.cache.delete(&state_cache_key).await;
    let user_id = oauth_state
        .user_id
        .ok_or_else(|| ApiError::Unauthorized("invalid Hackatime connection state".into()))?;
    let code = query
        .code
        .ok_or_else(|| ApiError::BadRequest("missing OAuth code".into()))?;
    let (account_id, access_token) = state.providers.hackatime_connection(&code).await?;
    let token = state
        .cipher
        .encrypt(&access_token)
        .map_err(ApiError::Internal)?;
    sqlx::query("INSERT INTO hackatime_connections (user_id, account_id, access_token_ciphertext) VALUES ($1, $2, $3) ON CONFLICT (user_id) DO UPDATE SET account_id = EXCLUDED.account_id, access_token_ciphertext = EXCLUDED.access_token_ciphertext, updated_at = now()")
        .bind(user_id).bind(account_id).bind(token).execute(&state.db).await?;
    state
        .cache
        .delete(&format!("user:{user_id}:hackatime-projects:v2"))
        .await;
    Ok(Redirect::to("/dashboard"))
}

pub async fn logout(State(state): State<Arc<AppState>>, headers: HeaderMap) -> ApiResult<Response> {
    if let Some(token) = session_token(&headers) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash(token))
            .execute(&state.db)
            .await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, clear_session_cookie(state.cookie_secure)?);
    Ok(response)
}
