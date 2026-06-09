use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("photo-duplicate-finder")
}

pub fn database_path() -> PathBuf {
    app_data_dir().join("index.db")
}

pub fn thumbnail_cache_dir() -> PathBuf {
    app_data_dir().join("thumbs")
}

pub fn ensure_app_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(app_data_dir())?;
    std::fs::create_dir_all(thumbnail_cache_dir())?;
    Ok(())
}
