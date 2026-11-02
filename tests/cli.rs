use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures");
    p.push(name);
    p.to_string_lossy().into_owned()
}

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_bioblitz-checklist"))
        .args(args)
        .output()
        .expect("run binary");
    (
        out.status.success(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn markdown_report_counts_like_inaturalist() {
    let (ok, md, err) = run(&[
        &fixture("day1.csv"),
        &fixture("day2.csv"),
        "--known",
        &fixture("previous.txt"),
        "--title",
        "Hilltop Preserve BioBlitz 2025",
    ]);
    assert!(ok, "{err}");
    assert!(md.starts_with("# Hilltop Preserve BioBlitz 2025\n"));
    assert!(md.contains("6 species and 9 taxa from 15 iNaturalist observations by 4 observers."));
    assert!(md.contains("| Left out: duplicate rows | 1 |"));
    assert!(md.contains("| Left out: casual or captive/cultivated | 1 |"));
    assert!(md.contains("| Left out: no identification | 1 |"));
    assert!(md.contains("| Research grade observations | 9 (60%) |"));
    assert!(md.contains("| Plants | 3 | 4 | 7 |"));
    assert!(md.contains("### Not on the previous list (3)"));
    assert!(md.contains("### Sensitive taxa (1)"));
    // Subspecies rolled up; covered genus-only IDs dropped; leaf genus kept.
    assert!(md.contains("- *Asclepias tuberosa* — butterfly milkweed · 2 obs · 2 observers · NEW"));
    assert!(!md.contains("*Quercus* (genus"));
    assert!(!md.contains("*Bombus* (genus"));
    assert!(md.contains("*Carex* (genus, not identified further) — sedges"));
    assert!(md.contains("###### Corvidae — 0 species, 1 taxon"));
    assert!(!md.contains("Rosa rugosa"));
    // Taxonomic order: Fagales before Gentianales within Magnoliopsida.
    let fag = md.find("##### Fagales").unwrap();
    let gent = md.find("##### Gentianales").unwrap();
    assert!(fag < gent);
    // No usernames or coordinates leak into the report.
    assert!(!md.contains("user1") && !md.contains("41.1"));
}

#[test]
fn overlapping_exports_count_once() {
    let (_, once, _) = run(&[&fixture("day1.csv")]);
    let (_, twice, _) = run(&[&fixture("day1.csv"), &fixture("day1.csv")]);
    let strip = |s: &str| -> String {
        s.lines()
            .filter(|l| !l.contains("Rows read") && !l.contains("duplicate rows"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&once), strip(&twice));
    assert!(twice.contains("| Left out: duplicate rows | 16 |"));
}

#[test]
fn csv_output() {
    let (ok, csv, err) = run(&[
        &fixture("day1.csv"),
        &fixture("day2.csv"),
        "-f",
        "csv",
        "-k",
        &fixture("previous.txt"),
    ]);
    assert!(ok, "{err}");
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(
        lines[0],
        "group,kingdom,phylum,class,order,family,genus,scientific_name,rank,common_name,observations,research_grade,observers,first_observed,last_observed,new_for_site,sensitive"
    );
    assert_eq!(lines.len(), 10);
    assert!(lines.contains(
        &"Birds,Animalia,Chordata,Aves,Passeriformes,Turdidae,Turdus,Turdus migratorius,species,American Robin,3,3,3,2025-06-07,2025-06-08,no,no"
    ));
    assert!(lines.contains(
        &"Plants,Plantae,Tracheophyta,Liliopsida,Asparagales,Orchidaceae,Cypripedium,Cypripedium acaule,species,pink lady's slipper,1,1,1,2025-06-07,2025-06-07,yes,yes"
    ));
}

#[test]
fn html_to_file_with_filters() {
    let dir = std::env::temp_dir().join(format!("bbc-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("report.html");
    let (ok, stdout, err) = run(&[
        &fixture("day1.csv"),
        "-o",
        out.to_str().unwrap(),
        "--research-only",
        "-x",
        "Erysiphaceae",
        "-g",
        "class,family",
        "--top",
        "0",
    ]);
    assert!(ok, "{err}");
    assert!(stdout.is_empty());
    let html = std::fs::read_to_string(&out).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<h3>Aves — 1 species</h3>"));
    assert!(html.contains("<h4>Turdidae — 1 species</h4>"));
    assert!(html.contains("<i>Cypripedium acaule</i> — pink lady's slipper"));
    assert!(!html.contains("Erysiphe"));
    assert!(!html.contains("Carex"));
    assert!(!html.contains("Most observed"));
}

#[test]
fn export_without_rank_columns_groups_by_iconic_taxon() {
    let dir = std::env::temp_dir().join(format!("bbc-plain-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("plain.csv");
    std::fs::write(
        &f,
        "id,observed_on,user_id,quality_grade,scientific_name,common_name,iconic_taxon_name\n\
         1,2025-06-07,1,research,Quercus alba,white oak,Plantae\n\
         2,2025-06-07,2,needs_id,Quercus,oaks,Plantae\n\
         3,2025-06-07,2,research,Turdus migratorius,American Robin,Aves\n",
    )
    .unwrap();
    let (ok, md, err) = run(&[f.to_str().unwrap(), "-g", "none"]);
    std::fs::remove_dir_all(&dir).ok();
    assert!(ok, "{err}");
    assert!(err.contains("no taxon rank columns"));
    assert!(md.contains("### Birds — 1 species"));
    assert!(md.contains("### Plants — 1 species"));
    assert!(md.contains("2 species and 2 taxa from 3"));
}

#[test]
fn errors_are_reported() {
    let (ok, _, err) = run(&["/nonexistent/export.csv"]);
    assert!(!ok);
    assert!(err.starts_with("error:"));
    let (ok, _, err) = run(&[&fixture("previous.txt")]);
    assert!(!ok);
    assert!(err.contains("scientific_name"));
    let (ok, help, _) = run(&["--help"]);
    assert!(ok && help.contains("USAGE"));
}
