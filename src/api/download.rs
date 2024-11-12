use axum::{
    body::Bytes,
    extract::Query,
    http::{header, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;

use super::manifest::fetch_manifest;
use crate::bundle::create_bundle;

#[derive(Deserialize)]
pub struct QueryParams {
    version: String,
    modules: Option<String>,
}

pub async fn download(Query(params): Query<QueryParams>) -> impl IntoResponse {
    let manifest = match fetch_manifest(params.version.clone()).await {
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
        Ok(None) => return (StatusCode::UNPROCESSABLE_ENTITY).into_response(),
        Ok(Some(manifest)) => manifest,
    };

    let mut modules = manifest.into_latest().modules;
    if let Some(params) = params.modules {
        let ids: Vec<&str> = params.split(',').collect();
        modules.retain(|module| ids.contains(&module.id.as_str()));
    }
    if modules.is_empty() {
        return (StatusCode::UNPROCESSABLE_ENTITY).into_response();
    }

    match create_bundle(params.version.clone(), modules).await {
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
        Ok(data) => {
            let headers = [
                (header::CONTENT_TYPE, "application/zip"),
                (
                    header::CONTENT_DISPOSITION,
                    &format!("attachment; filename=\"bookshelf-{}.zip\"", params.version),
                ),
            ];
            (StatusCode::OK, headers, Bytes::from(data)).into_response()
        }
    }
}
