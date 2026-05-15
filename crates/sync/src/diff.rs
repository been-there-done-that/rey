use crate::error::SyncError;
use types::sync::{SyncCollectionResponse, SyncFilesResponse, SyncTrashResponse};
use zoo_client::ZooClient;

const DEFAULT_PAGE_SIZE: usize = 100;

pub async fn fetch_collection_page(
    client: &ZooClient,
    since: i64,
) -> Result<SyncCollectionResponse, SyncError> {
    let token = client
        .session_token()
        .await
        .ok_or(SyncError::NetworkError(zoo_client::ZooError::NotAuthenticated))?;

    let url = format!(
        "{}/api/sync/collections?since={}&limit={}",
        client.base_url(),
        since,
        DEFAULT_PAGE_SIZE
    );

    let resp = client
        .client()
        .get(&url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(zoo_client::ZooError::HttpError)?;

    if !resp.status().is_success() {
        return Err(SyncError::NetworkError(zoo_client::ZooError::HttpError(
            resp.error_for_status().unwrap_err(),
        )));
    }

    let result: SyncCollectionResponse =
        resp.json().await.map_err(zoo_client::ZooError::HttpError)?;
    Ok(result)
}

pub async fn fetch_file_page(
    client: &ZooClient,
    collection_id: &str,
    since: i64,
) -> Result<SyncFilesResponse, SyncError> {
    let token = client
        .session_token()
        .await
        .ok_or(SyncError::NetworkError(zoo_client::ZooError::NotAuthenticated))?;

    let url = format!(
        "{}/api/sync/files?collection_id={}&since={}&limit={}",
        client.base_url(),
        collection_id,
        since,
        DEFAULT_PAGE_SIZE
    );

    let resp = client
        .client()
        .get(&url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(zoo_client::ZooError::HttpError)?;

    if !resp.status().is_success() {
        return Err(SyncError::NetworkError(zoo_client::ZooError::HttpError(
            resp.error_for_status().unwrap_err(),
        )));
    }

    let result: SyncFilesResponse =
        resp.json().await.map_err(zoo_client::ZooError::HttpError)?;
    Ok(result)
}

pub async fn fetch_trash_page(
    client: &ZooClient,
    since: i64,
) -> Result<SyncTrashResponse, SyncError> {
    let token = client
        .session_token()
        .await
        .ok_or(SyncError::NetworkError(zoo_client::ZooError::NotAuthenticated))?;

    let url = format!(
        "{}/api/sync/trash?since={}&limit={}",
        client.base_url(),
        since,
        DEFAULT_PAGE_SIZE
    );

    let resp = client
        .client()
        .get(&url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(zoo_client::ZooError::HttpError)?;

    if !resp.status().is_success() {
        return Err(SyncError::NetworkError(zoo_client::ZooError::HttpError(
            resp.error_for_status().unwrap_err(),
        )));
    }

    let result: SyncTrashResponse =
        resp.json().await.map_err(zoo_client::ZooError::HttpError)?;
    Ok(result)
}
