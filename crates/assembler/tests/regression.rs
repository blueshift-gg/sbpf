use {
    serde::{Deserialize, Serialize},
    std::{collections::BTreeMap, env, fs, path::PathBuf},
};

#[derive(Debug, Deserialize, Serialize)]
struct Manifest {
    #[serde(default)]
    cases: BTreeMap<String, Case>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Case {
    file: String,
    #[serde(default)]
    hash: String,
}

#[derive(Debug)]
enum IssueKind {
    HashMismatch,
    AssemblerError,
}
#[derive(Debug)]
struct Issue {
    kind: IssueKind,
    name: String,
    file: String,
    expected: Option<String>,
    actual: Option<String>,
    note: Option<String>,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read_manifest() -> Manifest {
    let manifest_path = fixtures_dir().join("index.toml");
    let content = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", manifest_path.display(), e));
    toml::from_str(&content).expect("failed to parse fixtures/index.toml")
}

fn read_source(case_file: &str) -> String {
    let path = fixtures_dir().join(case_file);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e))
}

fn write_manifest(manifest: &Manifest) {
    let manifest_path = fixtures_dir().join("index.toml");
    let content = toml::to_string_pretty(manifest)
        .unwrap_or_else(|e| panic!("failed to serialize manifest: {:?}", e));
    fs::write(&manifest_path, content)
        .unwrap_or_else(|e| panic!("failed to write {}: {}", manifest_path.display(), e));
}

fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[test]
fn test_regression() {
    let mut manifest = read_manifest();
    assert!(
        !manifest.cases.is_empty(),
        "fixtures/index.toml must define at least one case"
    );

    let update_hashes = env::var("UPDATE_HASHES")
        .ok()
        .filter(|v| v == "1")
        .is_some();
    let mut any_missing_or_mismatch = false;

    let mut issues: Vec<Issue> = Vec::new();

    let mut updated_entries = 0usize;
    for (name, case) in manifest.cases.iter_mut() {
        let source = read_source(&case.file);
        let actual = match sbpf_assembler::assemble(&source) {
            Ok(bytes) => hash_bytes(&bytes),
            Err(e) => {
                any_missing_or_mismatch = true;
                issues.push(Issue {
                    kind: IssueKind::AssemblerError,
                    name: name.clone(),
                    file: case.file.clone(),
                    expected: None,
                    actual: None,
                    note: Some(format!("assembler failed: {:?}", e)),
                });
                continue;
            }
        };

        if actual != case.hash {
            if update_hashes && case.hash.is_empty() {
                // Only update the hash if it's empty.
                case.hash = actual.clone();
                updated_entries += 1;
            } else {
                any_missing_or_mismatch = true;
                issues.push(Issue {
                    kind: IssueKind::HashMismatch,
                    name: name.clone(),
                    file: case.file.clone(),
                    expected: Some(case.hash.clone()),
                    actual: Some(actual.clone()),
                    note: None,
                });
            }
        }
    }

    if update_hashes && updated_entries > 0 {
        // Update the manifest.
        write_manifest(&manifest);
    }

    if any_missing_or_mismatch {
        // Print report.
        let mut mismatched = 0usize;
        let mut asmerr = 0usize;

        eprintln!("\n===== Regression Report =====");
        for issue in &issues {
            match issue.kind {
                IssueKind::HashMismatch => {
                    mismatched += 1;
                    eprintln!(
                        "[Mismatch] case='{}' file='{}' expected={} actual={}",
                        issue.name,
                        issue.file,
                        issue.expected.as_deref().unwrap_or("<none>"),
                        issue.actual.as_deref().unwrap_or("<none>")
                    );
                }
                IssueKind::AssemblerError => {
                    asmerr += 1;
                    eprintln!(
                        "[Assembler Error] case='{}' file='{}' note={}",
                        issue.name,
                        issue.file,
                        issue.note.as_deref().unwrap_or("")
                    );
                }
            }
        }
        eprintln!(
            "===== Summary: total={} mismatched={} assembler-error={} =====\n",
            issues.len(),
            mismatched,
            asmerr
        );

        // Fail the test.
        panic!("regressions detected ({}).", issues.len());
    }
}
