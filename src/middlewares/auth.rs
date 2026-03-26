use axum::{
    http::Request,
    middleware::Next,
    response::Response,
    http::StatusCode,
};

pub async fn auth_middleware(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let headers = req.headers();

    if let Some(auth) = headers.get("authorization") {
        if auth == "Bearer secret-token" {
            return Ok(next.run(req).await);
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}