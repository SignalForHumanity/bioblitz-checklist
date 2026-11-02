//! Reading iNaturalist observation exports and "known species" lists.

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Read;

/// Ranks above species that the iNaturalist export can include as
/// "taxon extras" columns (`taxon_<rank>_name`), from coarse to fine.
pub const RANKS: [&str; 16] = [
    "kingdom",
    "phylum",
    "subphylum",
    "superclass",
    "class",
    "subclass",
    "superorder",
    "order",
    "suborder",
    "superfamily",
    "family",
    "subfamily",
    "supertribe",
    "tribe",
    "subtribe",
    "genus",
];

/// Position of a rank name in [`RANKS`].
pub fn rank_index(rank: &str) -> Option<usize> {
    let r = rank.trim().to_ascii_lowercase();
    RANKS.iter().position(|x| *x == r)
}

pub type Lineage = [Option<String>; RANKS.len()];

/// One observation row, reduced to the fields the checklist needs.
#[derive(Debug, Clone, Default)]
pub struct Observation {
    pub id: Option<String>,
    pub observed_on: String,
    pub observer: String,
    pub quality: String,
    pub captive: bool,
    pub scientific_name: String,
    pub common_name: String,
    pub iconic: String,
    pub lineage: Lineage,
    /// Species binomial (infraspecific taxa rolled up), if identified to
    /// species or below.
    pub species: Option<String>,
    pub sensitive: bool,
}

/// Result of reading one export file.
#[derive(Debug, Default)]
pub struct Export {
    pub observations: Vec<Observation>,
    /// True if the file has the "taxon extras" rank columns.
    pub has_ranks: bool,
}

fn header_map(headers: &csv::StringRecord) -> HashMap<String, usize> {
    headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let h = h.trim_start_matches('\u{feff}').trim().to_ascii_lowercase();
            (h, i)
        })
        .collect()
}

/// Reduce a scientific name to a lowercase matching key: "genus epithet" for
/// species and below (authorship and infraspecific parts dropped), or the
/// single uninomial for coarser taxa.
pub fn name_key(name: &str) -> String {
    let mut words = name
        .split_whitespace()
        .map(|w| w.trim_start_matches('×'))
        .filter(|w| !w.is_empty() && !w.eq_ignore_ascii_case("x"));
    let first = match words.next() {
        Some(w) => w,
        None => return String::new(),
    };
    match words.next() {
        Some(second) if second.chars().next().is_some_and(|c| c.is_lowercase()) => {
            format!("{} {}", first, second).to_lowercase()
        }
        _ => first.to_lowercase(),
    }
}

/// Infer a species binomial from a bare scientific name when the export has
/// no rank columns.
fn infer_species(name: &str) -> Option<String> {
    let mut words = name.split_whitespace();
    let genus = words.next()?;
    let epithet = words.next()?;
    if epithet.chars().next().is_some_and(|c| c.is_lowercase()) {
        Some(format!("{} {}", genus, epithet))
    } else {
        None
    }
}

/// Parse an iNaturalist observation export (CSV) from a reader.
pub fn read_export<R: Read>(reader: R) -> Result<Export, String> {
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(reader);
    let headers = rdr
        .headers()
        .map_err(|e| format!("cannot read CSV header: {e}"))?
        .clone();
    let map = header_map(&headers);
    if !map.contains_key("scientific_name") {
        return Err(
            "no `scientific_name` column; is this an iNaturalist observation export?".to_string(),
        );
    }
    let col = |name: &str| map.get(name).copied();
    let c_id = col("id").or_else(|| col("uuid"));
    let c_date = col("observed_on");
    let c_user = col("user_id").or_else(|| col("user_login"));
    let c_quality = col("quality_grade");
    let c_captive = col("captive_cultivated");
    let c_sci = col("scientific_name");
    let c_common = col("common_name");
    let c_iconic = col("iconic_taxon_name");
    let c_geo = col("taxon_geoprivacy");
    let c_species = col("taxon_species_name");
    let c_hybrid = col("taxon_hybrid_name");
    let rank_cols: Vec<Option<usize>> = RANKS
        .iter()
        .map(|r| col(&format!("taxon_{r}_name")))
        .collect();
    let has_ranks = c_species.is_some() || rank_cols.iter().any(Option::is_some);

    let mut out = Export {
        observations: Vec::new(),
        has_ranks,
    };
    for (n, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| format!("CSV error at data row {}: {e}", n + 1))?;
        let get = |c: Option<usize>| -> String {
            c.and_then(|i| rec.get(i))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        let opt = |c: Option<usize>| -> Option<String> {
            let s = get(c);
            if s.is_empty() { None } else { Some(s) }
        };
        let scientific_name = get(c_sci);
        let mut lineage: Lineage = Default::default();
        for (slot, c) in lineage.iter_mut().zip(&rank_cols) {
            *slot = opt(*c);
        }
        let species = if has_ranks {
            opt(c_species)
                .or_else(|| opt(c_hybrid))
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        } else {
            infer_species(&scientific_name)
        };
        let geo = get(c_geo).to_ascii_lowercase();
        out.observations.push(Observation {
            id: opt(c_id),
            observed_on: get(c_date),
            observer: get(c_user),
            quality: get(c_quality).to_ascii_lowercase(),
            captive: get(c_captive).eq_ignore_ascii_case("true"),
            scientific_name,
            common_name: get(c_common),
            iconic: get(c_iconic),
            lineage,
            species,
            sensitive: geo == "obscured" || geo == "private",
        });
    }
    Ok(out)
}

/// Columns tried, in order, when reading a known-species list as CSV.
const KNOWN_COLUMNS: [&str; 6] = [
    "taxon_species_name",
    "species",
    "scientific_name",
    "scientificname",
    "acceptedscientificname",
    "name",
];

/// Read a list of previously known taxa. Accepts an iNaturalist export, a
/// GBIF species-list download (comma or tab separated), any CSV with a
/// recognised name column, or a plain text file with one name per line.
pub fn read_known(text: &str) -> HashSet<String> {
    let text = text.trim_start_matches('\u{feff}');
    let first_line = text.lines().next().unwrap_or("");
    let delim = if first_line.contains('\t') {
        Some(b'\t')
    } else if first_line.contains(',') {
        Some(b',')
    } else {
        None
    };
    if let Some(d) = delim {
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(d)
            .flexible(true)
            .from_reader(text.as_bytes());
        if let Ok(h) = rdr.headers() {
            let map = header_map(h);
            let cols: Vec<usize> = KNOWN_COLUMNS
                .iter()
                .filter_map(|c| map.get(*c).copied())
                .collect();
            if !cols.is_empty() {
                let mut set = HashSet::new();
                for rec in rdr.records().flatten() {
                    // First non-empty recognised column wins, so a GBIF row
                    // identified only to genus still contributes its name.
                    if let Some(name) = cols
                        .iter()
                        .filter_map(|&i| rec.get(i))
                        .map(str::trim)
                        .find(|s| !s.is_empty())
                    {
                        set.insert(name_key(name));
                    }
                }
                return set;
            }
        }
    }
    text.lines()
        .map(|l| l.split([',', '\t']).next().unwrap_or("").trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(name_key)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_key_strips_authorship_and_infraspecific() {
        assert_eq!(name_key("Quercus alba L."), "quercus alba");
        assert_eq!(
            name_key("Odocoileus virginianus borealis"),
            "odocoileus virginianus"
        );
        assert_eq!(name_key("Asteraceae"), "asteraceae");
        assert_eq!(name_key("Quercus × bebbiana"), "quercus bebbiana");
        assert_eq!(
            name_key("Quercus ×bebbiana C.K.Schneid."),
            "quercus bebbiana"
        );
        assert_eq!(name_key("Amanita (Pers.) Hook."), "amanita");
        assert_eq!(name_key("  "), "");
    }

    #[test]
    fn reads_export_with_bom_and_ranks() {
        let csv = "\u{feff}id,observed_on,user_id,quality_grade,captive_cultivated,scientific_name,common_name,iconic_taxon_name,taxon_geoprivacy,taxon_genus_name,taxon_species_name\n\
                   1,2025-05-01,7,research,false,Quercus alba,white oak,Plantae,,Quercus,Quercus alba\n\
                   2,2025-05-01,8,needs_id,true,Quercus,oaks,Plantae,obscured,Quercus,\n";
        let e = read_export(csv.as_bytes()).unwrap();
        assert!(e.has_ranks);
        assert_eq!(e.observations.len(), 2);
        let a = &e.observations[0];
        assert_eq!(a.species.as_deref(), Some("Quercus alba"));
        assert_eq!(a.lineage[15].as_deref(), Some("Quercus"));
        assert!(!a.captive && !a.sensitive);
        let b = &e.observations[1];
        assert!(b.species.is_none());
        assert!(b.captive && b.sensitive);
    }

    #[test]
    fn infers_species_without_rank_columns() {
        let csv = "scientific_name\nQuercus alba\nQuercus\nPapilio glaucus glaucus\n";
        let e = read_export(csv.as_bytes()).unwrap();
        assert!(!e.has_ranks);
        let s: Vec<_> = e.observations.iter().map(|o| o.species.clone()).collect();
        assert_eq!(
            s,
            vec![
                Some("Quercus alba".to_string()),
                None,
                Some("Papilio glaucus".to_string())
            ]
        );
    }

    #[test]
    fn rejects_non_inat_csv() {
        assert!(read_export("a,b\n1,2\n".as_bytes()).is_err());
    }

    #[test]
    fn known_list_formats() {
        let plain = "# old list\nQuercus alba\nAcer rubrum L.\n\n";
        let s = read_known(plain);
        assert!(s.contains("quercus alba") && s.contains("acer rubrum"));
        assert_eq!(s.len(), 2);

        let gbif = "taxonKey\tscientificName\ttaxonRank\tspecies\n\
                    1\tQuercus alba L.\tSPECIES\tQuercus alba\n\
                    2\tAmanita Pers.\tGENUS\t\n";
        let s = read_known(gbif);
        assert!(s.contains("quercus alba"));
        assert!(s.contains("amanita"));

        let inat = "id,scientific_name,taxon_species_name\n1,Bombus impatiens,Bombus impatiens\n";
        assert!(read_known(inat).contains("bombus impatiens"));
    }
}
