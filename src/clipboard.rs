use std::fs;
use std::path::PathBuf;

pub(crate) fn copied_image_file() -> Result<Option<PathBuf>, String> {
    select_single_copied_file(platform_copied_files()?)
}

fn select_single_copied_file(mut files: Vec<PathBuf>) -> Result<Option<PathBuf>, String> {
    if files.len() > 1 {
        return Err("Clipboard contains multiple files. Copy one image file.".to_string());
    }

    let Some(path) = files.pop() else {
        return Ok(None);
    };

    let metadata = fs::metadata(&path)
        .map_err(|err| format!("Cannot read copied image file '{}': {err}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "Copied clipboard item is not a file: {}",
            path.display()
        ));
    }

    Ok(Some(path))
}

#[cfg(target_os = "macos")]
fn platform_copied_files() -> Result<Vec<PathBuf>, String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeFileURL};

    let pasteboard = NSPasteboard::generalPasteboard();
    let Some(items) = pasteboard.pasteboardItems() else {
        return Ok(Vec::new());
    };
    let file_url_type = unsafe { NSPasteboardTypeFileURL };
    let mut files = Vec::new();

    for index in 0..items.len() {
        let item = items.objectAtIndex(index);
        let Some(url_string) = item.stringForType(file_url_type) else {
            continue;
        };
        files.push(file_url_path(&url_string)?);
    }

    Ok(files)
}

#[cfg(target_os = "macos")]
fn file_url_path(url_string: &objc2_foundation::NSString) -> Result<PathBuf, String> {
    use objc2_foundation::NSURL;

    let url = NSURL::URLWithString(url_string)
        .ok_or_else(|| "Clipboard contains an invalid file URL.".to_string())?;
    if !url.isFileURL() {
        return Err("Clipboard URL does not refer to a local file.".to_string());
    }
    url.to_file_path()
        .ok_or_else(|| "Clipboard file URL cannot be converted to a local path.".to_string())
}

#[cfg(not(target_os = "macos"))]
fn platform_copied_files() -> Result<Vec<PathBuf>, String> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must follow Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "jumpbox-typer-clipboard-{label}-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("test directory must be created");
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn no_copied_file_uses_direct_clipboard_image_fallback() {
        assert_eq!(select_single_copied_file(Vec::new()).unwrap(), None);
    }

    #[test]
    fn one_regular_copied_file_is_selected() {
        let directory = TestDirectory::new("one-file");
        let image = directory.path().join("image.png");
        fs::write(&image, "image data").expect("test image must be written");

        assert_eq!(
            select_single_copied_file(vec![image.clone()]).unwrap(),
            Some(image)
        );
    }

    #[test]
    fn multiple_copied_files_are_rejected() {
        let error = select_single_copied_file(vec!["one.png".into(), "two.png".into()])
            .expect_err("multiple files must be rejected");

        assert_eq!(
            error,
            "Clipboard contains multiple files. Copy one image file."
        );
    }

    #[test]
    fn missing_copied_file_has_actionable_error() {
        let directory = TestDirectory::new("missing-file");
        let image = directory.path().join("missing.png");

        let error =
            select_single_copied_file(vec![image]).expect_err("missing file must be rejected");

        assert!(error.contains("Cannot read copied image file"));
    }

    #[test]
    fn copied_directory_is_rejected() {
        let directory = TestDirectory::new("directory");

        let error = select_single_copied_file(vec![directory.path().to_path_buf()])
            .expect_err("directory must be rejected");

        assert!(error.contains("is not a file"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_file_url_decodes_spaces_in_path() {
        use objc2_foundation::NSString;

        let url = NSString::from_str("file:///tmp/Raycast%20image.png");

        assert_eq!(
            file_url_path(&url).unwrap(),
            PathBuf::from("/tmp/Raycast image.png")
        );
    }
}
