// SPDX-License-Identifier: AGPL-3.0-or-later
//! The handoff directory of recipe R, written in process by FSL's writer
//! (FSL FR-517) from FSL's own recipe inputs (`examples/checked-handoff` of the
//! checkout Cargo resolved for the `filament-spec-language` dependency, found
//! with `cargo metadata`) and the one unit this repository authored itself,
//! `tests/fixtures/checked-handoff/unit-instance-bound.native`, for which FSL
//! has no equivalent. No FSL file is copied here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use filament_contract_model::{
    CheckedPackageEvidence, ClockConfiguration, ClockExactNumber, NativeJoinBudget,
};
use filament_spec_language::checked_handoff::{evidence, write, CheckedHandoffRequest, Corpus};
use filament_spec_language::emit_checked_v2::{ClockSelection, Request};
use fsl_package::NativeJoinLimits;

/// The directory of the units this repository authored.
const OWN_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/checked-handoff"
);

/// FSL's `examples/checked-handoff`, in the checkout of the dependency that
/// Cargo resolved.
pub fn fsl_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let output = Command::new(env!("CARGO"))
            .args(["metadata", "--format-version", "1", "--offline"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("cargo metadata runs");
        assert!(output.status.success(), "cargo metadata failed");
        let metadata: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("cargo metadata JSON");
        let manifest = metadata["packages"]
            .as_array()
            .expect("packages")
            .iter()
            .find(|package| package["name"] == "filament-spec-language")
            .and_then(|package| package["manifest_path"].as_str())
            .expect("the filament-spec-language dependency");
        Path::new(manifest)
            .parent()
            .expect("a manifest directory")
            .join("examples/checked-handoff")
    })
}

/// The unit `U` of dimension `Money` that FSL's model declares `Amount` and
/// `Total` in, which every unit written for that model declares itself.
#[allow(dead_code)] // Only the tests that write their own unit declare it.
pub const MONEY_UNIT: &str = "dimension Money;\nunit U : Money = rational(1, 1);\n";

/// A unit of the recipe: FSL's, or this repository's own instance-bound unit.
fn unit_path(unit: &str) -> PathBuf {
    if unit == "unit-instance-bound.native" {
        Path::new(OWN_DIR).join(unit)
    } else {
        fsl_dir().join(unit)
    }
}

/// The text of the recipe unit `unit`.
#[allow(dead_code)] // Only the tests that edit the recipe unit read its text.
pub fn unit_text(unit: &str) -> String {
    std::fs::read_to_string(unit_path(unit)).expect("recipe unit")
}

fn recipe(kind: &str) -> (&'static str, Vec<(&'static str, ClockConfiguration)>) {
    match kind {
        "event_position" => (
            "unit-event-position.native",
            ["Full", "Partial", "Due", "DueLate"]
                .into_iter()
                .map(|clause| {
                    (
                        clause,
                        ClockConfiguration::EventPosition {
                            sequence_authority: "events-authority".into(),
                        },
                    )
                })
                .collect(),
        ),
        "instance_bound" => (
            "unit-instance-bound.native",
            vec![(
                "Settle",
                ClockConfiguration::EventPosition {
                    sequence_authority: "events-authority".into(),
                },
            )],
        ),
        "fixed_sample" => (
            "unit-fixed-sample.native",
            vec![(
                "DueSample",
                ClockConfiguration::FixedSample {
                    epoch: ClockExactNumber::Integer {
                        decimal: "0".into(),
                    },
                    period: ClockExactNumber::Rational {
                        numerator: "1".into(),
                        denominator: "10".into(),
                    },
                    unit: "milliseconds".into(),
                },
            )],
        ),
        _ => (
            "unit-timestamped-event.native",
            vec![(
                "DueTimestamp",
                ClockConfiguration::TimestampedEvent {
                    timestamp_unit: "rfc3339".into(),
                },
            )],
        ),
    }
}

/// The finite native-join ceilings every test pass states for itself: 2^20
/// work steps and 2^20 logical storage units, the values FSL's own handoff
/// example uses (FSL FR-531 gives a fixture no default).
const NATIVE_JOIN: NativeJoinLimits = NativeJoinLimits::new(1 << 20, 1 << 20);

/// A new native-join budget of [`NATIVE_JOIN`] for one pass.
pub fn join_budget() -> NativeJoinBudget {
    NATIVE_JOIN.budget()
}

/// The request for the package of `kind`.
pub fn request(kind: &str, corpus: Corpus) -> CheckedHandoffRequest {
    let (unit, clocks) = recipe(kind);
    CheckedHandoffRequest {
        emit: Request {
            source: unit_path(unit),
            package_name: "r".into(),
            models: vec![fsl_dir().join("model.semantic-ir.json")],
            libraries: vec![(
                "agent-ix/checked-handoff-library".into(),
                fsl_dir().join("library.native"),
            )],
            execution_points: None,
            temporal_selections: clocks
                .into_iter()
                .map(|(clause, clock)| ClockSelection {
                    clause: clause.into(),
                    clock,
                })
                .collect(),
            out: None,
        },
        corpus,
        native_join: NATIVE_JOIN,
    }
}

/// One written handoff: its files by relative path and the evidence the
/// writer assembled for it.
pub struct Handoff {
    _holder: tempfile::TempDir,
    /// The handoff directory itself.
    #[allow(dead_code)] // Only the realizability fixture compiles the directory back.
    pub root: PathBuf,
    /// Every file below the handoff root.
    pub files: BTreeMap<String, Vec<u8>>,
    /// The evidence for reading `checked-package.json`.
    pub evidence: CheckedPackageEvidence,
}

/// Write the handoff of `kind` with `corpus` into a fresh directory and read
/// every file back.
#[allow(dead_code)] // The dynamic-protocol tests (TC-050 to TC-055) write their own source.
pub fn written(kind: &str, corpus: Corpus) -> Handoff {
    let holder = tempfile::tempdir().expect("temporary directory");
    let out = holder.path().join("handoff");
    let asked = request(kind, corpus);
    finish(holder, out, asked)
}

/// Write a handoff from `source` while retaining the other inputs of `kind`.
#[allow(dead_code)] // Only TC-150 needs a second handoff; other test binaries include this module.
pub fn written_source(kind: &str, source: &str, corpus: Corpus) -> Handoff {
    let holder = tempfile::tempdir().expect("temporary directory");
    let source_path = holder.path().join("unit.native");
    std::fs::write(&source_path, source).expect("write source fixture");
    let mut asked = request(kind, corpus);
    asked.emit.source = source_path;
    let out = holder.path().join("handoff");
    finish(holder, out, asked)
}

/// Write a handoff from an explicitly adjusted request.
#[allow(dead_code)] // Only TC-150 needs an alternate temporal selection; other test binaries include this module.
pub fn written_request(asked: CheckedHandoffRequest) -> Handoff {
    let holder = tempfile::tempdir().expect("temporary directory");
    let out = holder.path().join("handoff");
    finish(holder, out, asked)
}

fn finish(holder: tempfile::TempDir, out: PathBuf, asked: CheckedHandoffRequest) -> Handoff {
    write(&asked, &out).expect("the handoff is written");
    let files = tree(&out);
    let evidence = evidence(&asked).expect("evidence");
    Handoff {
        _holder: holder,
        root: out,
        files,
        evidence,
    }
}

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).expect("read directory") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let name = path
                    .strip_prefix(root)
                    .expect("below root")
                    .display()
                    .to_string();
                files.insert(name, std::fs::read(&path).expect("read file"));
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}
