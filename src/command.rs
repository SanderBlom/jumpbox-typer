use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub(crate) fn resolve_command(
    binary: &str,
    search_path: Option<&OsStr>,
    fallback_paths: &[PathBuf],
) -> Option<PathBuf> {
    search_path
        .into_iter()
        .flat_map(std::env::split_paths)
        .map(|directory| directory.join(binary))
        .chain(fallback_paths.iter().cloned())
        .find_map(|candidate| executable_path(&candidate))
}

fn executable_path(candidate: &Path) -> Option<PathBuf> {
    if !is_executable(candidate) {
        return None;
    }

    candidate.canonicalize().ok()
}

#[cfg(unix)]
fn is_executable(candidate: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    candidate
        .metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(candidate: &Path) -> bool {
    candidate.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must follow Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "jumpbox-typer-command-{label}-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("test directory must be created");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn make_executable(path: &Path) {
        fs::write(path, "#!/bin/sh\nexit 0\n").expect("fake executable must be written");
        let mut permissions = fs::metadata(path)
            .expect("fake executable metadata must exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("fake executable must be executable");
    }

    #[test]
    fn inherited_path_executable_wins_over_fallback() {
        let path_directory = TestDirectory::new("path-first");
        let fallback_directory = TestDirectory::new("fallback-second");
        let path_executable = path_directory.path().join("tesseract");
        let fallback_executable = fallback_directory.path().join("tesseract");
        make_executable(&path_executable);
        make_executable(&fallback_executable);

        let resolved = resolve_command(
            "tesseract",
            Some(path_directory.path().as_os_str()),
            &[fallback_executable],
        );

        assert_eq!(
            resolved,
            Some(
                path_executable
                    .canonicalize()
                    .expect("PATH executable must resolve")
            )
        );
    }

    #[test]
    fn each_configured_fallback_can_supply_a_missing_path_command() {
        for label in ["apple-silicon-homebrew", "intel-homebrew"] {
            let fallback_directory = TestDirectory::new(label);
            let fallback_executable = fallback_directory.path().join("tesseract");
            make_executable(&fallback_executable);

            let resolved = resolve_command(
                "tesseract",
                Some(OsStr::new("")),
                std::slice::from_ref(&fallback_executable),
            );

            assert_eq!(
                resolved,
                Some(
                    fallback_executable
                        .canonicalize()
                        .expect("fallback executable must resolve")
                )
            );
        }
    }

    #[test]
    fn non_executable_and_missing_candidates_are_unavailable() {
        let path_directory = TestDirectory::new("not-executable");
        let non_executable = path_directory.path().join("tesseract");
        fs::write(&non_executable, "not executable").expect("candidate must be written");
        let missing = path_directory.path().join("missing-tesseract");

        let resolved = resolve_command(
            "tesseract",
            Some(path_directory.path().as_os_str()),
            &[missing],
        );

        assert_eq!(resolved, None);
    }
}
