//! Aggregating observations into a species checklist, counted the way
//! iNaturalist counts species ("leaf taxa").

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::input::{Lineage, Observation, RANKS, name_key};

/// Filters and comparison data for building a checklist.
#[derive(Debug, Default, Clone)]
pub struct Options {
    /// Keep casual and captive/cultivated observations.
    pub include_casual: bool,
    /// Keep only research-grade observations.
    pub research_only: bool,
    /// Taxon names (any rank) whose observations are dropped.
    pub exclude: Vec<String>,
    /// Name keys of taxa already known from the site.
    pub known: Option<HashSet<String>>,
}

/// One row of the checklist.
#[derive(Debug, Clone)]
pub struct Taxon {
    pub name: String,
    /// "species", a rank from [`RANKS`], or "" when unknown.
    pub rank: String,
    pub lineage: Lineage,
    pub iconic: String,
    pub common_name: String,
    pub observations: usize,
    pub research_grade: usize,
    pub observers: usize,
    pub first_observed: String,
    pub last_observed: String,
    pub sensitive: bool,
    /// `Some(true)` if not on the known list; `None` without a known list.
    pub new_for_site: Option<bool>,
}

impl Taxon {
    pub fn is_species(&self) -> bool {
        self.rank == "species"
    }
}

/// Counts for one iconic group (plants, birds, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupCount {
    pub label: String,
    pub species: usize,
    pub taxa: usize,
    pub observations: usize,
}

#[derive(Debug, Default, Clone)]
pub struct Summary {
    pub rows_read: usize,
    pub duplicates: usize,
    pub excluded_casual: usize,
    pub excluded_quality: usize,
    pub excluded_taxa: usize,
    pub unidentified: usize,
    pub observations: usize,
    pub research_grade: usize,
    pub observers: usize,
    pub first_observed: String,
    pub last_observed: String,
    pub species: usize,
    pub taxa: usize,
    pub new_species: Option<usize>,
    pub groups: Vec<GroupCount>,
}

#[derive(Debug, Clone)]
pub struct Checklist {
    /// Leaf taxa in pseudo-taxonomic order.
    pub taxa: Vec<Taxon>,
    pub summary: Summary,
    pub has_ranks: bool,
}

/// Plain-language label for an iNaturalist iconic taxon.
pub fn group_label(iconic: &str) -> String {
    match iconic {
        "Plantae" => "Plants",
        "Fungi" => "Fungi and lichens",
        "Aves" => "Birds",
        "Insecta" => "Insects",
        "Arachnida" => "Spiders and other arachnids",
        "Mammalia" => "Mammals",
        "Reptilia" => "Reptiles",
        "Amphibia" => "Amphibians",
        "Actinopterygii" => "Ray-finned fishes",
        "Mollusca" => "Mollusks",
        "Animalia" => "Other animals",
        "Chromista" => "Kelp, diatoms and allies",
        "Protozoa" => "Protozoans",
        "" => "Unknown group",
        other => other,
    }
    .to_string()
}

#[derive(Default)]
struct Acc {
    name: String,
    rank: String,
    lineage: Lineage,
    iconic: String,
    exact_common: String,
    any_common: String,
    observations: usize,
    research_grade: usize,
    observers: HashSet<String>,
    first: String,
    last: String,
    sensitive: bool,
}

fn matches_exclude(o: &Observation, exclude: &[String]) -> bool {
    exclude.iter().any(|x| {
        o.scientific_name.eq_ignore_ascii_case(x)
            || o.species
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case(x))
            || o.lineage
                .iter()
                .flatten()
                .any(|n| n.eq_ignore_ascii_case(x))
    })
}

/// Rank of a coarse (not species) identification, from its lineage.
fn coarse_rank(o: &Observation) -> String {
    o.lineage
        .iter()
        .zip(RANKS)
        .rev()
        .find(|(n, _)| {
            n.as_deref()
                .is_some_and(|n| n.eq_ignore_ascii_case(&o.scientific_name))
        })
        .map(|(_, r)| r.to_string())
        .unwrap_or_default()
}

fn update_dates(first: &mut String, last: &mut String, date: &str) {
    if date.is_empty() {
        return;
    }
    if first.is_empty() || date < first.as_str() {
        *first = date.to_string();
    }
    if last.is_empty() || date > last.as_str() {
        *last = date.to_string();
    }
}

/// Build a checklist from observations read from one or more exports.
pub fn build(observations: &[Observation], has_ranks: bool, opts: &Options) -> Checklist {
    let mut s = Summary {
        rows_read: observations.len(),
        ..Default::default()
    };
    let mut seen_ids = HashSet::new();
    let mut accs: BTreeMap<String, Acc> = BTreeMap::new();
    let mut observers = HashSet::new();
    let mut group_obs: HashMap<String, usize> = HashMap::new();

    for o in observations {
        if let Some(id) = &o.id
            && !seen_ids.insert(id.clone())
        {
            s.duplicates += 1;
            continue;
        }
        if !opts.include_casual && (o.captive || o.quality == "casual") {
            s.excluded_casual += 1;
            continue;
        }
        if opts.research_only && o.quality != "research" {
            s.excluded_quality += 1;
            continue;
        }
        if matches_exclude(o, &opts.exclude) {
            s.excluded_taxa += 1;
            continue;
        }
        let (name, rank) = match &o.species {
            Some(sp) => (sp.clone(), "species".to_string()),
            None if o.scientific_name.is_empty() => {
                s.unidentified += 1;
                continue;
            }
            None => (o.scientific_name.clone(), coarse_rank(o)),
        };
        s.observations += 1;
        *group_obs.entry(group_label(&o.iconic)).or_default() += 1;
        if o.quality == "research" {
            s.research_grade += 1;
        }
        update_dates(&mut s.first_observed, &mut s.last_observed, &o.observed_on);
        if !o.observer.is_empty() {
            observers.insert(o.observer.clone());
        }

        // Keyed by iconic group too, so homonyms such as Pieris (a plant
        // genus and a butterfly genus) are not merged.
        let a = accs
            .entry(format!("{}\u{0}{}", o.iconic, name.to_lowercase()))
            .or_default();
        if a.name.is_empty() {
            a.name = name;
            a.rank = rank;
        }
        for (slot, v) in a.lineage.iter_mut().zip(&o.lineage) {
            if slot.is_none() {
                slot.clone_from(v);
            }
        }
        if a.iconic.is_empty() {
            a.iconic.clone_from(&o.iconic);
        }
        if !o.common_name.is_empty() {
            if o.scientific_name.eq_ignore_ascii_case(&a.name) && a.exact_common.is_empty() {
                a.exact_common.clone_from(&o.common_name);
            }
            if a.any_common.is_empty() && a.rank != "species" {
                a.any_common.clone_from(&o.common_name);
            }
        }
        a.observations += 1;
        if o.quality == "research" {
            a.research_grade += 1;
        }
        if !o.observer.is_empty() {
            a.observers.insert(o.observer.clone());
        }
        update_dates(&mut a.first, &mut a.last, &o.observed_on);
        a.sensitive |= o.sensitive;
    }
    s.observers = observers.len();

    let all: Vec<Acc> = accs.into_values().collect();
    let mut taxa: Vec<Taxon> = all
        .iter()
        .filter(|a| is_leaf(a, &all))
        .map(|a| {
            let new_for_site = opts.known.as_ref().map(|k| !k.contains(&name_key(&a.name)));
            Taxon {
                name: a.name.clone(),
                rank: a.rank.clone(),
                lineage: a.lineage.clone(),
                iconic: a.iconic.clone(),
                common_name: if a.exact_common.is_empty() {
                    a.any_common.clone()
                } else {
                    a.exact_common.clone()
                },
                observations: a.observations,
                research_grade: a.research_grade,
                observers: a.observers.len(),
                first_observed: a.first.clone(),
                last_observed: a.last.clone(),
                sensitive: a.sensitive,
                new_for_site,
            }
        })
        .collect();
    taxa.sort_by_cached_key(sort_key);

    s.species = taxa.iter().filter(|t| t.is_species()).count();
    s.taxa = taxa.len();
    if opts.known.is_some() {
        s.new_species = Some(
            taxa.iter()
                .filter(|t| t.is_species() && t.new_for_site == Some(true))
                .count(),
        );
    }
    s.groups = group_counts(&taxa, &group_obs);

    Checklist {
        taxa,
        summary: s,
        has_ranks,
    }
}

/// A taxon counts unless another recorded taxon is nested inside it, which is
/// how iNaturalist counts species: an observation of "Quercus" adds a taxon
/// only if no oak species was recorded.
fn is_leaf(a: &Acc, all: &[Acc]) -> bool {
    if a.rank == "species" {
        return true;
    }
    let lower = a.name.to_lowercase();
    if let Some(ri) = RANKS.iter().position(|r| *r == a.rank) {
        !all.iter().any(|b| {
            !std::ptr::eq(a, b)
                && (a.lineage[0].is_none()
                    || b.lineage[0].is_none()
                    || a.lineage[0] == b.lineage[0])
                && b.lineage[ri]
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(&a.name))
                && depth(b) > ri
        })
    } else {
        // No rank columns: treat "Quercus" as covered by any "Quercus x".
        let prefix = format!("{lower} ");
        !all.iter().any(|b| {
            b.iconic.eq_ignore_ascii_case(&a.iconic) && b.name.to_lowercase().starts_with(&prefix)
        })
    }
}

fn depth(a: &Acc) -> usize {
    if a.rank == "species" {
        RANKS.len()
    } else if let Some(i) = RANKS.iter().position(|r| *r == a.rank) {
        i
    } else {
        // A rank with no export column (subgenus, section, complex ...) sits
        // just below the finest rank filled in its lineage.
        a.lineage
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |i| i + 1)
    }
}

/// Pseudo-taxonomic order: by each rank name in turn (missing ranks last),
/// then by name, so genera and species sit together under their families.
fn sort_key(t: &Taxon) -> (Vec<(bool, String)>, String) {
    (
        t.lineage
            .iter()
            .map(|n| match n {
                Some(n) => (false, n.to_lowercase()),
                None => (true, String::new()),
            })
            .collect(),
        t.name.to_lowercase(),
    )
}

/// Species and taxa per group from the counted taxa; observations per group
/// from every counted observation, including ones under coarser IDs.
fn group_counts(taxa: &[Taxon], group_obs: &HashMap<String, usize>) -> Vec<GroupCount> {
    let mut m: HashMap<String, GroupCount> = HashMap::new();
    for t in taxa {
        let label = group_label(&t.iconic);
        let g = m.entry(label.clone()).or_insert(GroupCount {
            label,
            species: 0,
            taxa: 0,
            observations: 0,
        });
        g.taxa += 1;
        if t.is_species() {
            g.species += 1;
        }
    }
    for (label, n) in group_obs {
        m.entry(label.clone())
            .or_insert(GroupCount {
                label: label.clone(),
                species: 0,
                taxa: 0,
                observations: 0,
            })
            .observations = *n;
    }
    let mut v: Vec<GroupCount> = m.into_values().collect();
    v.sort_by(|a, b| b.taxa.cmp(&a.taxa).then_with(|| a.label.cmp(&b.label)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(id: &str, sci: &str, genus: Option<&str>, family: &str, quality: &str) -> Observation {
        let mut lineage: Lineage = Default::default();
        lineage[0] = Some("Plantae".into());
        lineage[10] = Some(family.into());
        lineage[15] = genus.map(String::from);
        let species = if sci.contains(' ') {
            Some(sci.split_whitespace().take(2).collect::<Vec<_>>().join(" "))
        } else {
            None
        };
        Observation {
            id: Some(id.into()),
            observed_on: format!("2025-05-0{}", id.len()),
            observer: format!("u{id}"),
            quality: quality.into(),
            captive: false,
            scientific_name: sci.into(),
            common_name: String::new(),
            iconic: "Plantae".into(),
            lineage,
            species,
            sensitive: false,
        }
    }

    #[test]
    fn counts_leaf_taxa_like_inaturalist() {
        let o = vec![
            obs("1", "Quercus alba", Some("Quercus"), "Fagaceae", "research"),
            obs(
                "2",
                "Quercus alba alba",
                Some("Quercus"),
                "Fagaceae",
                "needs_id",
            ),
            obs("3", "Quercus", Some("Quercus"), "Fagaceae", "needs_id"),
            obs("4", "Acer", Some("Acer"), "Sapindaceae", "needs_id"),
            obs("5", "Fagaceae", None, "Fagaceae", "needs_id"),
            obs("1", "Quercus alba", Some("Quercus"), "Fagaceae", "research"),
        ];
        let c = build(&o, true, &Options::default());
        let names: Vec<_> = c.taxa.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["Quercus alba", "Acer"]);
        assert_eq!(c.summary.species, 1);
        assert_eq!(c.summary.taxa, 2);
        assert_eq!(c.summary.duplicates, 1);
        assert_eq!(c.summary.observations, 5);
        assert_eq!(c.summary.research_grade, 1);
        assert_eq!(c.taxa[0].observations, 2);
        assert_eq!(c.taxa[1].rank, "genus");
    }

    #[test]
    fn filters_and_known_list() {
        let mut casual = obs("9", "Rosa rugosa", Some("Rosa"), "Rosaceae", "casual");
        casual.captive = true;
        let o = vec![
            obs("1", "Quercus alba", Some("Quercus"), "Fagaceae", "research"),
            obs("2", "Acer rubrum", Some("Acer"), "Sapindaceae", "needs_id"),
            casual,
            obs(
                "4",
                "Erysiphe",
                Some("Erysiphe"),
                "Erysiphaceae",
                "research",
            ),
        ];
        let opts = Options {
            research_only: true,
            exclude: vec!["erysiphaceae".into()],
            known: Some(["quercus alba".to_string()].into_iter().collect()),
            ..Default::default()
        };
        let c = build(&o, true, &opts);
        assert_eq!(c.summary.excluded_casual, 1);
        assert_eq!(c.summary.excluded_quality, 1);
        assert_eq!(c.summary.excluded_taxa, 1);
        assert_eq!(c.taxa.len(), 1);
        assert_eq!(c.taxa[0].new_for_site, Some(false));
        assert_eq!(c.summary.new_species, Some(0));
    }

    #[test]
    fn without_ranks_genus_covered_by_species() {
        let mk = |id: &str, sci: &str| Observation {
            id: Some(id.into()),
            scientific_name: sci.into(),
            species: sci.contains(' ').then(|| sci.to_string()),
            ..Default::default()
        };
        let o = vec![
            mk("1", "Quercus alba"),
            mk("2", "Quercus"),
            mk("3", "Acer"),
            mk("4", ""),
        ];
        let c = build(&o, false, &Options::default());
        let names: Vec<_> = c.taxa.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["Acer", "Quercus alba"]);
        assert_eq!(c.summary.unidentified, 1);
    }

    #[test]
    fn homonym_genera_in_different_kingdoms_stay_apart() {
        // Pieris is both a plant genus (Ericaceae) and a butterfly genus.
        let plant = obs("1", "Pieris", Some("Pieris"), "Ericaceae", "needs_id");
        let mut bug = obs("2", "Pieris rapae", Some("Pieris"), "Pieridae", "research");
        bug.lineage[0] = Some("Animalia".into());
        bug.iconic = "Insecta".into();
        let mut bug_genus = obs("3", "Pieris", Some("Pieris"), "Pieridae", "needs_id");
        bug_genus.lineage[0] = Some("Animalia".into());
        bug_genus.iconic = "Insecta".into();
        let c = build(&[plant, bug, bug_genus], true, &Options::default());
        let names: Vec<_> = c
            .taxa
            .iter()
            .map(|t| (t.name.as_str(), t.observations))
            .collect();
        assert_eq!(names, vec![("Pieris rapae", 1), ("Pieris", 1)]);
        assert_eq!(c.taxa[1].lineage[10].as_deref(), Some("Ericaceae"));
        assert_eq!(c.summary.taxa, 2);
    }

    #[test]
    fn homonyms_without_rank_columns_stay_apart() {
        let mk = |id: &str, sci: &str, iconic: &str| Observation {
            id: Some(id.into()),
            scientific_name: sci.into(),
            iconic: iconic.into(),
            species: sci.contains(' ').then(|| sci.to_string()),
            ..Default::default()
        };
        let o = vec![
            mk("1", "Pieris", "Plantae"),
            mk("2", "Pieris rapae", "Insecta"),
        ];
        let c = build(&o, false, &Options::default());
        assert_eq!(c.summary.taxa, 2);
    }

    #[test]
    fn identification_at_unlisted_rank_covers_its_genus() {
        // A subgenus/section/complex ID has no rank column of its own; it is
        // finer than the genus, so the genus-only ID must not count as well.
        let mut sub = obs(
            "1",
            "Bombus (Pyrobombus)",
            Some("Bombus"),
            "Apidae",
            "needs_id",
        );
        sub.species = None;
        let genus = obs("2", "Bombus", Some("Bombus"), "Apidae", "needs_id");
        let c = build(&[sub, genus], true, &Options::default());
        let names: Vec<_> = c.taxa.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["Bombus (Pyrobombus)"]);
    }

    #[test]
    fn group_labels() {
        assert_eq!(group_label("Aves"), "Birds");
        assert_eq!(group_label("Weird"), "Weird");
    }
}
