//! Framework fingerprinting -- detect Django, Flask, FastAPI, Express, etc.
//!
//! Scans dependency files in the project directory to identify frameworks.
//! 10KB cap on file reads to avoid processing lockfiles.

use std::collections::HashSet;
use std::path::Path;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum file size to read (10KB).
const MAX_FILE_SIZE: usize = 10_240;

/// Framework detection markers: (framework_id, filename, dependency_name).
const MARKERS: &[(&str, &str, &str)] = &[
    // Python frameworks
    ("django", "requirements.txt", "django"),
    ("django", "pyproject.toml", "django"),
    ("flask", "requirements.txt", "flask"),
    ("flask", "pyproject.toml", "flask"),
    ("fastapi", "requirements.txt", "fastapi"),
    ("fastapi", "pyproject.toml", "fastapi"),
    // JS/TS frameworks
    ("express", "package.json", "express"),
    ("nextjs", "package.json", "next"),
    ("react", "package.json", "react"),
    ("vue", "package.json", "vue"),
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Detect frameworks from dependency files in `cwd`.
///
/// Checks requirements.txt and pyproject.toml for Python frameworks,
/// package.json for JavaScript frameworks. Returns a set of framework
/// identifiers (e.g., "django", "react", "nextjs").
pub fn detect_frameworks(cwd: &str) -> HashSet<String> {
    let mut detected = HashSet::new();
    let mut file_cache: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for &(framework, filename, dep) in MARKERS {
        if detected.contains(framework) {
            continue;
        }

        let path = Path::new(cwd).join(filename);
        let path_str = path.to_string_lossy().to_string();

        let content = file_cache
            .entry(path_str.clone())
            .or_insert_with(|| read_file_safe(&path_str));

        if content.is_empty() {
            continue;
        }

        let found = if filename == "package.json" {
            check_package_json(content, dep)
        } else {
            check_text_dependency(content, dep)
        };

        if found {
            detected.insert(framework.to_string());
        }
    }

    detected
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Read a file up to `MAX_FILE_SIZE` bytes, return empty string on failure.
fn read_file_safe(path: &str) -> String {
    match std::fs::read(path) {
        Ok(bytes) => {
            let truncated = if bytes.len() > MAX_FILE_SIZE {
                &bytes[..MAX_FILE_SIZE]
            } else {
                &bytes
            };
            String::from_utf8_lossy(truncated).to_string()
        }
        Err(_) => String::new(),
    }
}

/// Check if a dependency name appears in text content (requirements.txt, pyproject.toml).
///
/// Case-insensitive match. Strips comments before checking.
fn check_text_dependency(content: &str, dep: &str) -> bool {
    let dep_lower = dep.to_lowercase();
    for line in content.lines() {
        // Strip comments and whitespace
        let stripped = line.split('#').next().unwrap_or("").trim().to_lowercase();
        if stripped.contains(&dep_lower) {
            return true;
        }
    }
    false
}

/// Check if a dependency appears in package.json (dependencies, devDependencies,
/// peerDependencies).
fn check_package_json(content: &str, dep: &str) -> bool {
    let data: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    for section in &["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(deps) = data.get(section) {
            if let Some(obj) = deps.as_object() {
                if obj.contains_key(dep) {
                    return true;
                }
            }
        }
    }

    false
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- check_text_dependency ---

    #[test]
    fn test_text_dep_basic() {
        let content = "flask==2.0\nrequests>=1.0\n";
        assert!(check_text_dependency(content, "flask"));
        assert!(check_text_dependency(content, "requests"));
        assert!(!check_text_dependency(content, "django"));
    }

    #[test]
    fn test_text_dep_case_insensitive() {
        let content = "Django>=4.0\n";
        assert!(check_text_dependency(content, "django"));
    }

    #[test]
    fn test_text_dep_with_comment() {
        // "flask" in a comment line should NOT match (comment stripped)
        let content = "# flask is great\nrequests>=1.0\n";
        assert!(!check_text_dependency(content, "flask"));
        // Inline comment stripped too
        let content2 = "requests>=1.0  # also uses flask\n";
        assert!(!check_text_dependency(content2, "flask"));
    }

    #[test]
    fn test_text_dep_pyproject() {
        let content = r#"
[project]
dependencies = [
    "fastapi>=0.100",
    "uvicorn",
]
"#;
        assert!(check_text_dependency(content, "fastapi"));
        assert!(check_text_dependency(content, "uvicorn"));
        assert!(!check_text_dependency(content, "django"));
    }

    #[test]
    fn test_text_dep_empty() {
        assert!(!check_text_dependency("", "flask"));
    }

    // --- check_package_json ---

    #[test]
    fn test_package_json_dependencies() {
        let content = r#"{
            "dependencies": {
                "express": "^4.18",
                "cors": "^2.8"
            }
        }"#;
        assert!(check_package_json(content, "express"));
        assert!(check_package_json(content, "cors"));
        assert!(!check_package_json(content, "react"));
    }

    #[test]
    fn test_package_json_dev_dependencies() {
        let content = r#"{
            "devDependencies": {
                "jest": "^29.0"
            }
        }"#;
        assert!(check_package_json(content, "jest"));
        assert!(!check_package_json(content, "express"));
    }

    #[test]
    fn test_package_json_peer_dependencies() {
        let content = r#"{
            "peerDependencies": {
                "react": "^18.0"
            }
        }"#;
        assert!(check_package_json(content, "react"));
    }

    #[test]
    fn test_package_json_invalid() {
        assert!(!check_package_json("not json", "express"));
        assert!(!check_package_json("", "express"));
    }

    #[test]
    fn test_package_json_missing_sections() {
        let content = r#"{"name": "my-app"}"#;
        assert!(!check_package_json(content, "express"));
    }

    // --- detect_frameworks ---

    #[test]
    fn test_detect_frameworks_nonexistent_dir() {
        let result = detect_frameworks("/tmp/nonexistent-ecko-test-dir-xyz");
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_frameworks_empty_dir() {
        let dir = std::env::temp_dir().join("ecko-test-fingerprint-empty");
        let _ = std::fs::create_dir_all(&dir);
        let result = detect_frameworks(dir.to_str().unwrap());
        assert!(result.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_frameworks_requirements_txt() {
        let dir = std::env::temp_dir().join("ecko-test-fingerprint-req");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("requirements.txt"), "django>=4.0\nrequests>=2.0\n").unwrap();
        let result = detect_frameworks(dir.to_str().unwrap());
        assert!(result.contains("django"));
        assert!(!result.contains("flask"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_frameworks_package_json() {
        let dir = std::env::temp_dir().join("ecko-test-fingerprint-pkg");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("package.json"),
            r#"{"dependencies": {"react": "^18.0", "next": "^14.0"}}"#,
        )
        .unwrap();
        let result = detect_frameworks(dir.to_str().unwrap());
        assert!(result.contains("react"));
        assert!(result.contains("nextjs"));
        assert!(!result.contains("express"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_detect_frameworks_multiple_files() {
        let dir = std::env::temp_dir().join("ecko-test-fingerprint-multi");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("requirements.txt"), "fastapi>=0.100\n").unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"dependencies": {"vue": "^3.0"}}"#,
        )
        .unwrap();
        let result = detect_frameworks(dir.to_str().unwrap());
        assert!(result.contains("fastapi"));
        assert!(result.contains("vue"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- read_file_safe ---

    #[test]
    fn test_read_file_safe_nonexistent() {
        assert_eq!(read_file_safe("/tmp/nonexistent-ecko-xyz"), "");
    }

    #[test]
    fn test_read_file_safe_truncation() {
        let dir = std::env::temp_dir().join("ecko-test-fingerprint-trunc");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("big.txt");
        let content = "x".repeat(MAX_FILE_SIZE + 1000);
        std::fs::write(&path, &content).unwrap();
        let result = read_file_safe(path.to_str().unwrap());
        assert_eq!(result.len(), MAX_FILE_SIZE);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- MARKERS coverage ---

    #[test]
    fn test_all_seven_frameworks() {
        let frameworks: HashSet<&str> = MARKERS.iter().map(|&(fw, _, _)| fw).collect();
        assert!(frameworks.contains("django"));
        assert!(frameworks.contains("flask"));
        assert!(frameworks.contains("fastapi"));
        assert!(frameworks.contains("express"));
        assert!(frameworks.contains("nextjs"));
        assert!(frameworks.contains("react"));
        assert!(frameworks.contains("vue"));
        assert_eq!(frameworks.len(), 7);
    }
}
