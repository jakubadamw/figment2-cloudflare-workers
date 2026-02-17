use figment2::Figment;
use figment2_cloudflare_env::CloudflareWorkersBindings;
use serde::{Deserialize, Serialize};
use worker::*;

/// All fields required — tests that every binding is read.
#[derive(Deserialize, Serialize)]
struct FullConfig {
    api_base_url: String,
    api_key: String,
    max_retries: String,
}

/// Optional fields — tests that missing bindings are silently skipped.
#[derive(Deserialize, Serialize)]
struct PartialConfig {
    api_base_url: Option<String>,
    missing_field: Option<String>,
    api_key: Option<String>,
}

/// Single required field.
#[derive(Deserialize, Serialize)]
struct SingleConfig {
    api_base_url: String,
}

#[event(fetch)]
async fn fetch(request: Request, environment: Env, _context: Context) -> Result<Response> {
    let url = request.url()?;
    let path = url.path();

    match path {
        "/full" => {
            // All fields present (mix of vars and secrets).
            let config: FullConfig = Figment::new()
                .merge(CloudflareWorkersBindings::from_struct::<FullConfig>(
                    &environment,
                ))
                .extract()
                .map_err(|error| worker::Error::RustError(error.to_string()))?;
            Response::from_json(&config)
        }
        "/partial" => {
            // Some fields missing — `Option` fields get `None`.
            let config: PartialConfig = Figment::new()
                .merge(CloudflareWorkersBindings::from_struct::<PartialConfig>(
                    &environment,
                ))
                .extract()
                .map_err(|error| worker::Error::RustError(error.to_string()))?;
            Response::from_json(&config)
        }
        "/single" => {
            // Single field extraction.
            let config: SingleConfig = Figment::new()
                .merge(CloudflareWorkersBindings::from_struct::<SingleConfig>(
                    &environment,
                ))
                .extract()
                .map_err(|error| worker::Error::RustError(error.to_string()))?;
            Response::from_json(&config)
        }
        "/profile" => {
            // Custom profile: values land under "staging", then we select it.
            let config: SingleConfig = Figment::new()
                .merge(
                    CloudflareWorkersBindings::from_struct::<SingleConfig>(&environment)
                        .profile("staging"),
                )
                .select("staging")
                .extract()
                .map_err(|error| worker::Error::RustError(error.to_string()))?;
            Response::from_json(&config)
        }
        "/missing-all" => {
            // All required fields missing — extraction should fail.
            let result = Figment::new()
                .merge(CloudflareWorkersBindings::from_struct::<SingleConfig>(
                    &environment,
                ))
                .extract::<SingleConfig>();
            match result {
                Ok(_) => Response::from_json(&serde_json::json!({"error": false})),
                Err(error) => Response::from_json(
                    &serde_json::json!({"error": true, "message": error.to_string()}),
                ),
            }
        }
        _ => Response::error("Not found", 404),
    }
}
