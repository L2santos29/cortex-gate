// Cortex Gate — Authentication & Authorization
//
// Proporciona validación de tokens de cliente (x-api-key / Bearer)
// y tokens de administración (X-Admin-Token) para los endpoints
// del gateway.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Variable de entorno para la API key por defecto de los clientes.
pub const CLIENT_API_KEY_ENV: &str = "CORTEX_API_KEY";

/// Variable de entorno para el token de administración.
pub const ADMIN_TOKEN_ENV: &str = "CORTEX_ADMIN_TOKEN";

// ---------------------------------------------------------------------------
// AuthError
// ---------------------------------------------------------------------------

/// Error devuelto cuando la autenticación falla.
///
/// Implementa [`IntoResponse`] para poder usarse directamente
/// con el operador `?` en los handlers de Axum.
#[derive(Debug)]
pub struct AuthError {
    /// Mensaje descriptivo del error.
    pub message: String,
    /// Código HTTP (401 Unauthorized o 403 Forbidden).
    pub status: StatusCode,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": {
                "message": self.message,
                "type": "authentication_error",
                "code": self.status.as_u16(),
            }
        });
        (self.status, axum::Json(body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// Helper functions for env secrets
// ---------------------------------------------------------------------------

/// Lee una variable de entorno opcional, devolviendo `None` si no está seteada.
pub fn read_optional_secret(env_var: &str) -> Option<String> {
    std::env::var(env_var).ok()
}

/// Lee una variable de entorno requerida, panic si no está seteada.
///
/// ## Panics
/// Si la variable de entorno `env_var` no está definida.
pub fn read_required_secret(env_var: &str) -> String {
    std::env::var(env_var).unwrap_or_else(|_| {
        panic!(
            "Required environment variable '{}' is not set",
            env_var
        )
    })
}

// ---------------------------------------------------------------------------
// Client authentication
// ---------------------------------------------------------------------------

/// Valida la autenticación de cliente usando el header `x-api-key` o
/// `Authorization: Bearer <token>`.
///
/// Compara el valor recibido contra `expected_api_key`. Si ninguno de
/// los dos headers está presente o el valor no coincide, devuelve un
/// [`AuthError`] con status 401.
pub fn require_client_auth(
    headers: &HeaderMap,
    expected_api_key: &str,
) -> Result<(), AuthError> {
    // 1. Intentar con header x-api-key
    if let Some(val) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        if val == expected_api_key {
            return Ok(());
        }
    }

    // 2. Intentar con Authorization: Bearer <token>
    if let Some(val) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = val.strip_prefix("Bearer ") {
            if token == expected_api_key {
                return Ok(());
            }
        }
    }

    // 3. Fallo
    Err(AuthError {
        message: "Invalid or missing API key. Provide x-api-key or Authorization: Bearer header."
            .to_string(),
        status: StatusCode::UNAUTHORIZED,
    })
}

// ---------------------------------------------------------------------------
// Admin authentication
// ---------------------------------------------------------------------------

/// Valida la autenticación de administrador usando el header
/// `X-Admin-Token`.
///
/// Compara el valor contra `expected_admin_token`. Si el header no está
/// presente o el valor no coincide, devuelve un [`AuthError`] con status
/// 403 (Forbidden) para distinguirlo del error de autenticación de cliente.
pub fn require_admin_auth(
    headers: &HeaderMap,
    expected_admin_token: &str,
) -> Result<(), AuthError> {
    match headers
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
    {
        Some(token) if token == expected_admin_token => Ok(()),
        _ => Err(AuthError {
            message: "Invalid or missing admin token. Provide X-Admin-Token header.".to_string(),
            status: StatusCode::FORBIDDEN,
        }),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    // -- Helpers -----------------------------------------------------------------

    fn make_headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (k, v) in pairs {
            let name: axum::http::HeaderName = k.parse().unwrap();
            let value = HeaderValue::from_str(v).unwrap();
            headers.insert(name, value);
        }
        headers
    }

    const TEST_API_KEY: &str = "sk-test-key-12345";
    const TEST_ADMIN_TOKEN: &str = "admin-test-token-67890";

    // -- require_client_auth -----------------------------------------------------

    #[test]
    fn client_auth_with_x_api_key() {
        let headers = make_headers(&[("x-api-key", TEST_API_KEY)]);
        assert!(require_client_auth(&headers, TEST_API_KEY).is_ok());
    }

    #[test]
    fn client_auth_with_bearer() {
        let headers = make_headers(&[("authorization", &format!("Bearer {}", TEST_API_KEY))]);
        assert!(require_client_auth(&headers, TEST_API_KEY).is_ok());
    }

    #[test]
    fn client_auth_x_api_key_takes_precedence() {
        // x-api-key antes que Bearer
        let headers = make_headers(&[
            ("x-api-key", TEST_API_KEY),
            ("authorization", "Bearer wrong-key"),
        ]);
        assert!(require_client_auth(&headers, TEST_API_KEY).is_ok());
    }

    #[test]
    fn client_auth_wrong_key() {
        let headers = make_headers(&[("x-api-key", "wrong-key")]);
        let result = require_client_auth(&headers, TEST_API_KEY);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn client_auth_missing_header() {
        let headers = HeaderMap::new();
        let result = require_client_auth(&headers, TEST_API_KEY);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn client_auth_bearer_no_space() {
        // Sin espacio después de "Bearer" → no hace strip_prefix
        let headers = make_headers(&[("authorization", &format!("Bearer{}", TEST_API_KEY))]);
        let result = require_client_auth(&headers, TEST_API_KEY);
        assert!(result.is_err());
    }

    #[test]
    fn client_auth_empty_key() {
        let headers = make_headers(&[("x-api-key", "")]);
        let result = require_client_auth(&headers, TEST_API_KEY);
        assert!(result.is_err());
    }

    // -- require_admin_auth ------------------------------------------------------

    #[test]
    fn admin_auth_valid_token() {
        let headers = make_headers(&[("x-admin-token", TEST_ADMIN_TOKEN)]);
        assert!(require_admin_auth(&headers, TEST_ADMIN_TOKEN).is_ok());
    }

    #[test]
    fn admin_auth_wrong_token() {
        let headers = make_headers(&[("x-admin-token", "wrong-token")]);
        let result = require_admin_auth(&headers, TEST_ADMIN_TOKEN);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn admin_auth_missing_header() {
        let headers = HeaderMap::new();
        let result = require_admin_auth(&headers, TEST_ADMIN_TOKEN);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().status, StatusCode::FORBIDDEN);
    }

    // -- read_optional_secret / read_required_secret ------------------------------

    #[test]
    fn read_optional_secret_set() {
        std::env::set_var("CORTEX_TEST_OPTIONAL", "hello");
        assert_eq!(
            read_optional_secret("CORTEX_TEST_OPTIONAL"),
            Some("hello".to_string())
        );
        std::env::remove_var("CORTEX_TEST_OPTIONAL");
    }

    #[test]
    fn read_optional_secret_not_set() {
        std::env::remove_var("CORTEX_TEST_UNSET_OPTIONAL");
        assert_eq!(read_optional_secret("CORTEX_TEST_UNSET_OPTIONAL"), None);
    }

    #[test]
    #[should_panic(expected = "CORTEX_TEST_REQUIRED")]
    fn read_required_secret_panics_when_unset() {
        std::env::remove_var("CORTEX_TEST_REQUIRED");
        read_required_secret("CORTEX_TEST_REQUIRED");
    }

    #[test]
    fn read_required_secret_ok() {
        std::env::set_var("CORTEX_TEST_REQUIRED_OK", "secret-value");
        assert_eq!(
            read_required_secret("CORTEX_TEST_REQUIRED_OK"),
            "secret-value"
        );
        std::env::remove_var("CORTEX_TEST_REQUIRED_OK");
    }

    // -- AuthError IntoResponse --------------------------------------------------

    #[tokio::test]
    async fn auth_error_into_response_returns_json() {
        let err = AuthError {
            message: "test error".to_string(),
            status: StatusCode::UNAUTHORIZED,
        };
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
