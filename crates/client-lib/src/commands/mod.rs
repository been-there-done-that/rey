pub mod auth;
pub mod collections;
pub mod device;
pub mod error;
pub mod files;
pub mod search;
#[cfg(feature = "desktop")]
pub mod sync;
pub mod thumbnails;
pub mod upload;

#[cfg(feature = "desktop")]
pub fn register_commands(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        auth::login,
        auth::logout,
        auth::register,
        auth::get_auth_params,
        collections::list_collections,
        collections::create_collection,
        collections::archive_collection,
        files::list_files,
        files::get_file,
        files::archive_file,
        files::download_file,
        sync::trigger_sync,
        sync::get_sync_status,
        upload::upload_file,
        upload::cancel_upload,
        upload::list_pending_uploads,
        thumbnails::get_thumbnail,
        thumbnails::evict_thumbnail,
        device::register_device,
        device::get_device_info,
        search::search_files,
        search::search_by_date,
        search::search_by_location,
    ])
}
