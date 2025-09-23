use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use blake3;
use serde::{Deserialize, Serialize};

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
    #[serde(default = "default_runs")]
    runs: u32,
}

#[derive(Debug)]
enum IssueKind {
    HashMismatch,
    NonDeterministic,
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

fn default_runs() -> u32 {
    10
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
        let mut first_hash: Option<String> = None;
        let mut nondeterministic = false;
        let mut assembler_failed = false;

        for _ in 0..case.runs.max(1) {
            let bytes = match sbpf_assembler::assemble(&source) {
                Ok(b) => b,
                Err(e) => {
                    assembler_failed = true;
                    any_missing_or_mismatch = true;
                    issues.push(Issue {
                        kind: IssueKind::AssemblerError,
                        name: name.clone(),
                        file: case.file.clone(),
                        expected: None,
                        actual: None,
                        note: Some(format!("assembler failed: {:?}", e)),
                    });
                    break;
                }
            };
            let h = hash_bytes(&bytes);
            if let Some(prev) = &first_hash {
                if &h != prev {
                    nondeterministic = true;
                    issues.push(Issue {
                        kind: IssueKind::NonDeterministic,
                        name: name.clone(),
                        file: case.file.clone(),
                        expected: Some(prev.clone()),
                        actual: Some(h.clone()),
                        note: Some("bytecode hash varied across runs".to_string()),
                    });
                    break;
                }
            } else {
                first_hash = Some(h);
            }
        }

        if assembler_failed {
            // Already recorded. Skip to next case.
            continue;
        }

        let actual = match first_hash {
            Some(h) => h,
            None => {
                any_missing_or_mismatch = true;
                issues.push(Issue {
                    kind: IssueKind::AssemblerError,
                    name: name.clone(),
                    file: case.file.clone(),
                    expected: None,
                    actual: None,
                    note: Some("no hash computed".to_string()),
                });
                continue;
            }
        };

        if nondeterministic {
            any_missing_or_mismatch = true;
            continue;
        }

        if actual != case.hash {
            if update_hashes {
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
        let mut nondet = 0usize;
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
                IssueKind::NonDeterministic => {
                    nondet += 1;
                    eprintln!(
                        "[Non-deterministic] case='{}' file='{}' note={}",
                        issue.name,
                        issue.file,
                        issue.note.as_deref().unwrap_or("")
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
            "===== Summary: total={} mismatched={} non-deterministic={} assembler-error={} =====\n",
            issues.len(),
            mismatched,
            nondet,
            asmerr
        );

        // Fail the test.
        panic!("regressions detected ({}).", issues.len());
    }
}
