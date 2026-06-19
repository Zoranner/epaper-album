use std::{path::Path, path::PathBuf};

use crate::error::AppError;

pub(super) async fn remove_image_files(data_dir: &Path, sha256: &str) -> Result<(), AppError> {
    remove_file_if_exists(display_image_path(data_dir, sha256)).await?;
    for extension in ["jpg", "png", "bmp", "jpeg"] {
        remove_file_if_exists(original_image_dir(data_dir).join(format!("{sha256}.{extension}")))
            .await?;
    }
    Ok(())
}

pub(super) async fn remove_file_if_exists(path: PathBuf) -> Result<(), AppError> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::Internal(error.into())),
    }
}

pub(super) fn original_image_path(data_dir: &Path, sha256: &str, extension: &str) -> PathBuf {
    original_image_dir(data_dir).join(format!("{sha256}.{extension}"))
}

pub(super) fn find_original_image_path(data_dir: &Path, sha256: &str) -> anyhow::Result<PathBuf> {
    let directory = original_image_dir(data_dir);
    for extension in ["jpg", "png", "bmp", "jpeg"] {
        let path = directory.join(format!("{sha256}.{extension}"));
        if path.exists() {
            return Ok(path);
        }
    }

    let legacy_path = directory.join(sha256);
    if legacy_path.exists() {
        return Ok(legacy_path);
    }

    Err(anyhow::anyhow!("original image file missing: {sha256}"))
}

fn original_image_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("images").join("original")
}

pub(super) fn display_image_path(data_dir: &Path, sha256: &str) -> PathBuf {
    data_dir
        .join("images")
        .join("display")
        .join(format!("{sha256}.bmp"))
}

pub(super) fn display_image_temp_path(data_dir: &Path, sha256: &str) -> PathBuf {
    data_dir
        .join("images")
        .join("display")
        .join(format!("{sha256}.tmp"))
}

pub(super) fn sprite_cache_path(data_dir: &Path, sha256: &str) -> PathBuf {
    data_dir.join("sprites").join(format!("{sha256}.bmp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_image_path_uses_bmp_extension() {
        let path = display_image_path(Path::new("data"), "abc");
        assert_eq!(
            path,
            Path::new("data")
                .join("images")
                .join("display")
                .join("abc.bmp")
        );
    }
}
