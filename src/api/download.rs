use std::collections::HashMap;

use axum::body::Bytes;
use axum::extract::Query;
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::bundle::{create_bundle, VersionedModule};
use super::manifest::fetch_manifest;


#[derive(Deserialize)]
pub struct QueryParams {
    version: String,
    modules: String,
}


pub async fn download(Query(params): Query<QueryParams>) -> impl IntoResponse {
    if params.version.is_empty() || params.modules.is_empty() {
        return (StatusCode::BAD_REQUEST, "Version and modules cannot be empty.").into_response();
    }

    let mut modules = vec![];
    let mut versions: HashMap<&str, Vec<&str>> = HashMap::new();

    for entry in params.modules.split(',') {
        let (module_id, version) = entry.split_once(':').unwrap_or((entry, &params.version));
        versions.entry(version).or_default().push(module_id);
    }

    for (version, module_ids) in versions {
        let manifest = match fetch_manifest(version.to_string()).await {
            Ok(Some(m)) => m.into_latest(),
            Ok(None) => return (
                StatusCode::BAD_REQUEST,
                format!("Version `{}` not found.", version),
            ).into_response(),
            Err(_) => return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to retrieve manifest for version `{}`.", version),
            ).into_response(),
        };

        for module_id in module_ids {
            if let Some(module) = manifest.modules.iter().find(|m| m.id == module_id) {
                modules.push(VersionedModule::new(
                    module.id.clone(),
                    module.slug.clone(),
                    module.kind,
                    version.to_string(),
                ));
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    format!("Module `{}` does not exist in version `{}`.", module_id, version),
                ).into_response();
            }
        }
    }

    match create_bundle(modules).await {
        Ok(data) => {
            let headers = [
                (header::CONTENT_TYPE, "application/zip"),
                (header::CONTENT_DISPOSITION, "attachment; filename=\"bookshelf-packs.zip\""),
            ];
            (StatusCode::OK, headers, Bytes::from(data)).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create the bundle.").into_response(),
    }
}
