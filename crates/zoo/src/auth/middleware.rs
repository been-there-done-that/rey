use crate::db::sessions::lookup_session;
use crate::state::AppState;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct AuthState {
    pub pool: sqlx::PgPool,
}

pub async fn auth_middleware(
    state: axum::extract::State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let auth_str = auth_header.to_str().map_err(|_| StatusCode::UNAUTHORIZED)?;
    if !auth_str.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_str[7..];
    let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));

    let session = lookup_session(&state.pool, &token_hash)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    req.extensions_mut().insert(session.user_id);

    Ok(next.run(req).await)
}

pub fn extract_user_id(req: &Request) -> Option<Uuid> {
    req.extensions().get::<Uuid>().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request as HttpRequest;

    #[test]
    fn test_extract_user_id_present() {
        let user_id = Uuid::new_v4();
        let mut req = HttpRequest::builder()
            .uri("/test")
            .body(())
            .unwrap();
        req.extensions_mut().insert(user_id);
        assert_eq!(extract_user_id(&req), Some(user_id));
    }

    #[test]
    fn test_extract_user_id_absent() {
        let req = HttpRequest::builder()
            .uri("/test")
            .body(())
            .unwrap();
        assert_eq!(extract_user_id(&req), None);
    }
}
