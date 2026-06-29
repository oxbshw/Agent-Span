//! OpenAPI 3.0 spec generation via `utoipa`.
//!
//! `GET /openapi.json` serves the machine-readable spec; `GET /docs` serves
//! Swagger UI (loaded from CDN) for interactive exploration. The spec is
//! generated from `#[utoipa::path]` annotations on the route handlers and
//! `ToSchema` derives on the request/response types, so it never drifts from
//! the code.

use axum::{
    http::StatusCode,
    response::{Html, Json},
};
use serde_json::Value;
use utoipa::OpenApi;

use agentspan_core::types::{Content, SearchResult};

use crate::routes::{
    auth::CreateKeyRequest,
    read::ReadQuery,
    search::FederatedRequest,
    v1::{ReadBody, SearchQuery},
};

/// Aggregate API doc. The `paths` macro collects every `#[utoipa::path]`-
/// annotated handler; `components(schemas(...))` registers the types those
/// handlers reference.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "AgentSpan API",
        version = "0.4.0",
        description = "The Web Access Gateway for AI Agents — Multi-Platform, Multi-Tenant. \
                      AgentSpan gives AI coding agents persistent, scalable, cached access to 50+ internet \
                      platforms via REST API, SSE, and native MCP server.",
        license(
            name = "MIT",
            url = "https://github.com/agentspan/agentspan/blob/main/LICENSE"
        ),
    ),
    paths(
        crate::routes::health::health,
        crate::routes::channels::list_channels,
        crate::routes::v1::channel_info,
        crate::routes::v1::channel_read,
        crate::routes::v1::channel_search,
        crate::routes::read::read,
        crate::routes::v1::smart_read,
        crate::routes::search::federated_search,
        crate::routes::doctor::doctor,
        crate::routes::v1::doctor_channel,
        crate::routes::v1::stats,
        crate::routes::v1::config,
        crate::routes::v1::suggestions,
        crate::routes::auth::create_key,
        crate::routes::auth::list_keys,
        crate::routes::auth::revoke_key,
    ),
    components(schemas(
        Content,
        SearchResult,
        ReadQuery,
        ReadBody,
        SearchQuery,
        FederatedRequest,
        CreateKeyRequest,
    )),
    tags(
        (name = "health", description = "Liveness and metrics"),
        (name = "channels", description = "Channel discovery and per-channel read/search"),
        (name = "read", description = "Smart URL read — auto-detects the channel"),
        (name = "search", description = "Federated search across channels"),
        (name = "doctor", description = "Health checks for channels and backends"),
        (name = "admin", description = "Observability and operational reports"),
        (name = "auth", description = "API key management (admin-scoped)"),
    ),
)]
pub struct ApiDoc;

/// Serve the OpenAPI JSON spec at `GET /openapi.json`.
pub async fn openapi_json() -> Json<Value> {
    let spec = ApiDoc::openapi();
    let json = serde_json::to_value(&spec).unwrap_or(Value::Null);
    Json(json)
}

/// Serve Swagger UI at `GET /docs`. Loads the UI from jsdelivr CDN and points
/// it at our own `/openapi.json`. No bundled assets → no axum version conflict.
pub async fn swagger_ui_html() -> (StatusCode, Html<&'static str>) {
    let html = "<!DOCTYPE html>\
<html lang=\"en\">\
  <head>\
    <meta charset=\"utf-8\" />\
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\
    <title>AgentSpan API — Swagger UI</title>\
    <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css\" />\
    <style>body { margin: 0; }</style>\
  </head>\
  <body>\
    <div id=\"swagger-ui\"></div>\
    <script src=\"https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js\" crossorigin></script>\
    <script>\
      window.onload = function () {\
        window.ui = SwaggerUIBundle({\
          url: \"/openapi.json\",\
          dom_id: \"#swagger-ui\",\
          deepLinking: true,\
          presets: [SwaggerUIBundle.presets.apis],\
          layout: \"BaseLayout\"\
        });\
      };\
    </script>\
  </body>\
</html>";
    (StatusCode::OK, Html(html))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_spec_is_valid_json() {
        let spec = ApiDoc::openapi();
        let json = spec.to_json().expect("spec serializes to JSON");
        assert!(json.contains("\"openapi\""));
        assert!(json.contains("\"AgentSpan API\""));
        assert!(json.contains("/api/v1/channels"));
        assert!(json.contains("/api/v1/doctor"));
        assert!(json.contains("/api/v1/auth/keys"));
    }

    #[test]
    fn openapi_spec_lists_all_annotated_paths() {
        let spec = ApiDoc::openapi();
        let json = spec.to_json().unwrap();
        for expected in [
            "/health",
            "/api/v1/channels",
            "/api/v1/channels/{name}",
            "/api/v1/channels/{name}/read",
            "/api/v1/channels/{name}/search",
            "/api/v1/read",
            "/api/v1/search/federated",
            "/api/v1/doctor",
            "/api/v1/doctor/{channel}",
            "/api/v1/stats",
            "/api/v1/config",
            "/api/v1/suggestions",
            "/api/v1/auth/keys",
            "/api/v1/auth/keys/{id}",
        ] {
            assert!(
                json.contains(&format!("\"{expected}\"")),
                "OpenAPI spec missing path: {expected}"
            );
        }
    }

    #[test]
    fn openapi_spec_includes_component_schemas() {
        let spec = ApiDoc::openapi();
        let json = spec.to_json().unwrap();
        assert!(json.contains("\"Content\""), "missing Content schema");
        assert!(
            json.contains("\"SearchResult\""),
            "missing SearchResult schema"
        );
        assert!(
            json.contains("\"FederatedRequest\""),
            "missing FederatedRequest schema"
        );
    }

    #[tokio::test]
    async fn openapi_json_endpoint_returns_spec() {
        let Json(json) = openapi_json().await;
        assert_eq!(json["openapi"], "3.0.3");
        assert!(json["paths"].is_object());
    }

    #[tokio::test]
    async fn docs_endpoint_returns_html() {
        let (status, Html(_)) = swagger_ui_html().await;
        assert_eq!(status, StatusCode::OK);
    }
}
