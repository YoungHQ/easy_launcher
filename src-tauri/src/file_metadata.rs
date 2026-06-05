use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    pub is_dir: bool,
    pub size_bytes: Option<u64>,
    pub modified_unix_seconds: Option<u64>,
    pub extension: Option<String>,
    pub full_path: String,
}

pub fn read_file_metadata(path: &str) -> FileMetadata {
    let path_ref = Path::new(path);
    let metadata = fs::metadata(path_ref).ok();
    let is_dir = metadata
        .as_ref()
        .map(fs::Metadata::is_dir)
        .unwrap_or_else(|| path_ref.extension().is_none());

    FileMetadata {
        is_dir,
        size_bytes: metadata
            .as_ref()
            .filter(|metadata| metadata.is_file())
            .map(fs::Metadata::len),
        modified_unix_seconds: metadata
            .and_then(|metadata| metadata.modified().ok())
            .and_then(system_time_to_unix_seconds),
        extension: path_ref
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_lowercase())
            .filter(|extension| !extension.is_empty()),
        full_path: path.to_string(),
    }
}

fn system_time_to_unix_seconds(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::{self, File};

    #[test]
    fn file_metadata_reads_file_details() {
        let path = temp_path("easy-launcher-file-metadata.txt");
        fs::write(&path, b"hello").expect("write temp file");

        let metadata = read_file_metadata(path.to_str().expect("utf-8 path"));

        assert!(!metadata.is_dir);
        assert_eq!(metadata.size_bytes, Some(5));
        assert_eq!(metadata.extension.as_deref(), Some("txt"));
        assert!(metadata.modified_unix_seconds.is_some());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_metadata_reads_directory_details() {
        let path = temp_path("easy-launcher-file-metadata-dir");
        fs::create_dir_all(&path).expect("create temp dir");

        let metadata = read_file_metadata(path.to_str().expect("utf-8 path"));

        assert!(metadata.is_dir);
        assert_eq!(metadata.size_bytes, None);
        assert_eq!(metadata.extension, None);
        assert!(metadata.modified_unix_seconds.is_some());

        let _ = fs::remove_dir(path);
    }

    #[test]
    fn file_metadata_handles_missing_path() {
        let path = temp_path("easy-launcher-missing-file.bin");
        let _ = fs::remove_file(&path);

        let metadata = read_file_metadata(path.to_str().expect("utf-8 path"));

        assert!(!metadata.is_dir);
        assert_eq!(metadata.size_bytes, None);
        assert_eq!(metadata.extension.as_deref(), Some("bin"));
        assert_eq!(metadata.modified_unix_seconds, None);
    }

    #[test]
    fn file_metadata_handles_empty_extension() {
        let path = temp_path("easy-launcher-file-metadata-no-extension");
        File::create(&path).expect("create temp file");

        let metadata = read_file_metadata(path.to_str().expect("utf-8 path"));

        assert_eq!(metadata.extension, None);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_metadata_handles_unreadable_or_unavailable_paths() {
        let metadata = read_file_metadata(r"Z:\path-that-should-not-exist\demo.txt");

        assert_eq!(metadata.size_bytes, None);
        assert_eq!(metadata.modified_unix_seconds, None);
        assert_eq!(metadata.extension.as_deref(), Some("txt"));
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!("{}-{name}", std::process::id()))
    }
}
