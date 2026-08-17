use crate::error::{ApiError, ApiResult};
use axum::http::HeaderValue;

pub fn session_cookie(token: &str, secure: bool) -> ApiResult<HeaderValue> {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "session={token}; Path=/; HttpOnly; SameSite=Lax; Priority=High; Max-Age=1814400{secure}"
    ))
    .map_err(|error| ApiError::Internal(error.into()))
}

pub fn clear_session_cookie(secure: bool) -> ApiResult<HeaderValue> {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "session=; Path=/; HttpOnly; SameSite=Lax; Priority=High; Max-Age=0{secure}"
    ))
    .map_err(|error| ApiError::Internal(error.into()))
}

#[cfg(test)]
mod tests {
    use super::session_cookie;

    #[test]
    fn marks_production_session_cookies_secure() {
        let cookie = session_cookie("token", true).unwrap();
        let value = cookie.to_str().unwrap();
        assert!(value.contains("HttpOnly"));
        assert!(value.contains("SameSite=Lax"));
        assert!(value.contains("Secure"));
    }
}
