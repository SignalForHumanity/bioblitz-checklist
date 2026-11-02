//! Turning a checklist into Markdown, printable HTML or CSV.

use crate::checklist::{Checklist, Taxon, group_label};
use crate::input::RANKS;

/// Report settings.
#[derive(Debug, Clone)]
pub struct ReportConfig {
    pub title: String,
    /// Indices into [`RANKS`] used as checklist headings, coarse to fine.
    pub group_by: Vec<usize>,
    /// How many most-observed taxa to list.
    pub top: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Span {
    Text(String),
    /// A scientific name shown in italics.
    Sci(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(usize, Vec<Span>),
    Para(Vec<Span>),
    Table {
        head: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    List {
        ordered: bool,
        items: Vec<Vec<Span>>,
    },
}

fn text(s: impl Into<String>) -> Span {
    Span::Text(s.into())
}

/// Format a count with thousands separators.
pub fn num(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", num(n), if n == 1 { one } else { many })
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Genus and species names are italicised by convention; higher ranks not.
fn name_span(t: &Taxon) -> Span {
    if t.is_species() || t.rank == "genus" || (t.rank.is_empty() && t.name.contains(' ')) {
        Span::Sci(t.name.clone())
    } else {
        Span::Text(t.name.clone())
    }
}

fn taxon_item(t: &Taxon, details: bool) -> Vec<Span> {
    let mut v = vec![name_span(t)];
    if !t.is_species() {
        if t.rank.is_empty() {
            v.push(text(" (not identified to species)"));
        } else {
            v.push(text(format!(" ({}, not identified further)", t.rank)));
        }
    }
    if !t.common_name.is_empty() {
        v.push(text(format!(" — {}", t.common_name)));
    }
    if details {
        let mut d = format!(
            " · {} · {}",
            plural(t.observations, "obs", "obs"),
            plural(t.observers, "observer", "observers")
        );
        if t.research_grade == 0 {
            d.push_str(" · not yet research grade");
        }
        if t.new_for_site == Some(true) {
            d.push_str(" · NEW");
        }
        if t.sensitive {
            d.push_str(" · sensitive");
        }
        v.push(text(d));
    } else {
        v.push(text(format!(" · {}", plural(t.observations, "obs", "obs"))));
    }
    v
}

/// Heading values for one taxon under the configured grouping.
fn heading_values(c: &Checklist, cfg: &ReportConfig, t: &Taxon) -> Vec<Option<String>> {
    if !c.has_ranks {
        return vec![Some(group_label(&t.iconic))];
    }
    cfg.group_by.iter().map(|&i| t.lineage[i].clone()).collect()
}

fn heading_labels(c: &Checklist, cfg: &ReportConfig) -> Vec<String> {
    if !c.has_ranks {
        vec!["group".to_string()]
    } else {
        cfg.group_by.iter().map(|&i| RANKS[i].to_string()).collect()
    }
}

/// Build the report as a list of blocks.
pub fn report(c: &Checklist, cfg: &ReportConfig) -> Vec<Block> {
    let s = &c.summary;
    let mut out = vec![Block::Heading(1, vec![text(cfg.title.clone())])];
    out.push(Block::Para(vec![text(format!(
        "{} and {} from {} by {}.",
        plural(s.species, "species", "species"),
        plural(s.taxa, "taxon", "taxa"),
        plural(
            s.observations,
            "iNaturalist observation",
            "iNaturalist observations"
        ),
        plural(s.observers, "observer", "observers"),
    ))]));

    // Summary
    out.push(Block::Heading(2, vec![text("Summary")]));
    let mut rows = vec![
        vec!["Rows read".to_string(), num(s.rows_read)],
        vec!["Observations counted".to_string(), num(s.observations)],
        vec!["Species".to_string(), num(s.species)],
        vec![
            "Taxa (species plus coarser identifications with no species under them)".to_string(),
            num(s.taxa),
        ],
        vec!["Observers".to_string(), num(s.observers)],
    ];
    if !s.first_observed.is_empty() {
        let dates = if s.first_observed == s.last_observed {
            s.first_observed.clone()
        } else {
            format!("{} to {}", s.first_observed, s.last_observed)
        };
        rows.push(vec!["Dates observed".to_string(), dates]);
    }
    if let Some(pct) = (s.research_grade * 100 + s.observations / 2).checked_div(s.observations) {
        rows.push(vec![
            "Research grade observations".to_string(),
            format!("{} ({pct}%)", num(s.research_grade)),
        ]);
    }
    if let Some(n) = s.new_species {
        rows.push(vec!["Species not on the previous list".to_string(), num(n)]);
    }
    for (n, label) in [
        (s.duplicates, "Left out: duplicate rows"),
        (s.excluded_casual, "Left out: casual or captive/cultivated"),
        (s.excluded_quality, "Left out: not research grade"),
        (s.excluded_taxa, "Left out: excluded taxa"),
        (s.unidentified, "Left out: no identification"),
    ] {
        if n > 0 {
            rows.push(vec![label.to_string(), num(n)]);
        }
    }
    out.push(Block::Table {
        head: vec!["".to_string(), "".to_string()],
        rows,
    });

    if !s.groups.is_empty() {
        out.push(Block::Heading(3, vec![text("By group")]));
        out.push(Block::Table {
            head: ["Group", "Species", "Taxa", "Observations"]
                .map(String::from)
                .to_vec(),
            rows: s
                .groups
                .iter()
                .map(|g| {
                    vec![
                        g.label.clone(),
                        num(g.species),
                        num(g.taxa),
                        num(g.observations),
                    ]
                })
                .collect(),
        });
    }

    // Highlights
    out.push(Block::Heading(2, vec![text("Highlights")]));
    if s.new_species.is_some() {
        let new: Vec<_> = c
            .taxa
            .iter()
            .filter(|t| t.is_species() && t.new_for_site == Some(true))
            .collect();
        out.push(Block::Heading(
            3,
            vec![text(format!(
                "Not on the previous list ({})",
                num(new.len())
            ))],
        ));
        if new.is_empty() {
            out.push(Block::Para(vec![text(
                "Every species found is already on the previous list.",
            )]));
        } else {
            out.push(Block::List {
                ordered: false,
                items: new.iter().map(|t| taxon_item(t, false)).collect(),
            });
        }
    }
    let sensitive: Vec<_> = c.taxa.iter().filter(|t| t.sensitive).collect();
    if !sensitive.is_empty() {
        out.push(Block::Heading(
            3,
            vec![text(format!("Sensitive taxa ({})", num(sensitive.len())))],
        ));
        out.push(Block::Para(vec![text(
            "iNaturalist hides the exact location of these taxa, usually because they are \
             threatened or collected. This report ties them to this property, which is more \
             precise than iNaturalist shows: share it only with the land manager, and remove \
             these names before publishing the report or posting it online.",
        )]));
        out.push(Block::List {
            ordered: false,
            items: sensitive.iter().map(|t| taxon_item(t, false)).collect(),
        });
    }
    if cfg.top > 0 && !c.taxa.is_empty() {
        let mut top: Vec<&Taxon> = c.taxa.iter().collect();
        top.sort_by(|a, b| {
            b.observations
                .cmp(&a.observations)
                .then_with(|| a.name.cmp(&b.name))
        });
        top.truncate(cfg.top);
        out.push(Block::Heading(3, vec![text("Most observed")]));
        out.push(Block::List {
            ordered: true,
            items: top.iter().map(|t| taxon_item(t, false)).collect(),
        });
    }
    let singles = c.taxa.iter().filter(|t| t.observations == 1).count();
    if singles > 0 {
        out.push(Block::Para(vec![text(format!(
            "{} recorded only once.",
            plural(singles, "taxon was", "taxa were")
        ))]));
    }

    // Checklist
    out.push(Block::Heading(2, vec![text("Checklist")]));
    out.push(Block::Para(vec![text(
        "Ordered by taxonomy, alphabetically within each rank. Subspecies and varieties are \
         counted under their species. A genus or family appears only when nothing under it \
         was identified to species.",
    )]));
    let labels = heading_labels(c, cfg);
    let mut rows: Vec<(Vec<Option<String>>, &Taxon)> = c
        .taxa
        .iter()
        .map(|t| (heading_values(c, cfg, t), t))
        .collect();
    rows.sort_by_cached_key(|(h, _)| {
        h.iter()
            .map(|v| match v {
                Some(v) => (false, v.to_lowercase()),
                None => (true, String::new()),
            })
            .collect::<Vec<_>>()
    });
    let mut prev: Vec<Option<String>> = Vec::new();
    let mut items: Vec<Vec<Span>> = Vec::new();
    for (i, (h, t)) in rows.iter().enumerate() {
        let first_diff = (0..h.len())
            .find(|&l| prev.get(l) != Some(&h[l]) || i == 0)
            .unwrap_or(h.len());
        if first_diff < h.len() {
            if !items.is_empty() {
                out.push(Block::List {
                    ordered: false,
                    items: std::mem::take(&mut items),
                });
            }
            for level in first_diff..h.len() {
                let under: Vec<&Taxon> = rows
                    .iter()
                    .filter(|(h2, _)| h2[..=level] == h[..=level])
                    .map(|(_, t)| *t)
                    .collect();
                let sp = under.iter().filter(|t| t.is_species()).count();
                let mut count = plural(sp, "species", "species");
                if under.len() != sp {
                    count = format!("{count}, {}", plural(under.len(), "taxon", "taxa"));
                }
                let label = match &h[level] {
                    Some(v) => v.clone(),
                    None => format!("{} not determined", capitalize(&labels[level])),
                };
                out.push(Block::Heading(
                    (level + 3).min(6),
                    vec![text(format!("{label} — {count}"))],
                ));
            }
        }
        items.push(taxon_item(t, true));
        prev = h.clone();
    }
    if !items.is_empty() {
        out.push(Block::List {
            ordered: false,
            items,
        });
    }
    out
}

fn md_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '*' | '_' | '|' | '\\' | '`' | '[' | ']' | '<' | '>' | '#'
        ) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

fn md_spans(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|s| match s {
            Span::Text(t) => md_escape(t),
            Span::Sci(t) => format!("*{}*", md_escape(t)),
        })
        .collect()
}

/// Render blocks as Markdown.
pub fn markdown(blocks: &[Block]) -> String {
    let mut o = String::new();
    for b in blocks {
        match b {
            Block::Heading(l, s) => {
                o.push_str(&format!("{} {}\n\n", "#".repeat(*l), md_spans(s)));
            }
            Block::Para(s) => o.push_str(&format!("{}\n\n", md_spans(s))),
            Block::Table { head, rows } => {
                let line = |cells: &[String]| {
                    format!(
                        "| {} |\n",
                        cells
                            .iter()
                            .map(|c| md_escape(c))
                            .collect::<Vec<_>>()
                            .join(" | ")
                    )
                };
                o.push_str(&line(head));
                o.push_str(&format!("|{}\n", "---|".repeat(head.len())));
                for r in rows {
                    o.push_str(&line(r));
                }
                o.push('\n');
            }
            Block::List { ordered, items } => {
                for (i, it) in items.iter().enumerate() {
                    if *ordered {
                        o.push_str(&format!("{}. {}\n", i + 1, md_spans(it)));
                    } else {
                        o.push_str(&format!("- {}\n", md_spans(it)));
                    }
                }
                o.push('\n');
            }
        }
    }
    o
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn html_spans(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|s| match s {
            Span::Text(t) => html_escape(t),
            Span::Sci(t) => format!("<i>{}</i>", html_escape(t)),
        })
        .collect()
}

const CSS: &str = "body{font-family:Georgia,serif;max-width:46em;margin:2em auto;padding:0 1em;\
line-height:1.4;color:#111}table{border-collapse:collapse;margin:.5em 0 1em}\
td,th{border:1px solid #999;padding:.2em .6em;text-align:left}\
td:not(:first-child){text-align:right}ul,ol{margin-top:.2em}li{margin:.1em 0}\
h3,h4,h5,h6{margin:.9em 0 .2em;break-after:avoid}h2{border-bottom:1px solid #999}\
@media print{body{margin:0;max-width:none;font-size:10.5pt}}";

/// Render blocks as a self-contained, printable HTML page.
pub fn html(blocks: &[Block], title: &str) -> String {
    let mut o = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>{}</title>\n<style>{CSS}</style>\n</head>\n<body>\n",
        html_escape(title)
    );
    for b in blocks {
        match b {
            Block::Heading(l, s) => o.push_str(&format!("<h{l}>{}</h{l}>\n", html_spans(s))),
            Block::Para(s) => o.push_str(&format!("<p>{}</p>\n", html_spans(s))),
            Block::Table { head, rows } => {
                o.push_str("<table>\n");
                if head.iter().any(|h| !h.is_empty()) {
                    o.push_str("<tr>");
                    for h in head {
                        o.push_str(&format!("<th>{}</th>", html_escape(h)));
                    }
                    o.push_str("</tr>\n");
                }
                for r in rows {
                    o.push_str("<tr>");
                    for c in r {
                        o.push_str(&format!("<td>{}</td>", html_escape(c)));
                    }
                    o.push_str("</tr>\n");
                }
                o.push_str("</table>\n");
            }
            Block::List { ordered, items } => {
                let tag = if *ordered { "ol" } else { "ul" };
                o.push_str(&format!("<{tag}>\n"));
                for it in items {
                    o.push_str(&format!("<li>{}</li>\n", html_spans(it)));
                }
                o.push_str(&format!("</{tag}>\n"));
            }
        }
    }
    o.push_str("</body>\n</html>\n");
    o
}

/// Rank columns written to the CSV output.
const CSV_RANKS: [&str; 6] = ["kingdom", "phylum", "class", "order", "family", "genus"];

/// Render the checklist as CSV, one row per counted taxon.
pub fn csv(c: &Checklist) -> Result<String, String> {
    let mut w = csv::Writer::from_writer(Vec::new());
    let mut head: Vec<&str> = vec!["group"];
    head.extend(CSV_RANKS);
    head.extend([
        "scientific_name",
        "rank",
        "common_name",
        "observations",
        "research_grade",
        "observers",
        "first_observed",
        "last_observed",
        "new_for_site",
        "sensitive",
    ]);
    w.write_record(&head).map_err(|e| e.to_string())?;
    let yes_no = |b: bool| if b { "yes" } else { "no" }.to_string();
    for t in &c.taxa {
        let mut r = vec![group_label(&t.iconic)];
        for rank in CSV_RANKS {
            let i = RANKS.iter().position(|x| *x == rank).unwrap_or(0);
            r.push(t.lineage[i].clone().unwrap_or_default());
        }
        r.extend([
            t.name.clone(),
            t.rank.clone(),
            t.common_name.clone(),
            t.observations.to_string(),
            t.research_grade.to_string(),
            t.observers.to_string(),
            t.first_observed.clone(),
            t.last_observed.clone(),
            t.new_for_site.map(yes_no).unwrap_or_default(),
            yes_no(t.sensitive),
        ]);
        w.write_record(&r).map_err(|e| e.to_string())?;
    }
    let bytes = w.into_inner().map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_note_warns_against_publishing_names() {
        use crate::checklist::{Options, build};
        use crate::input::Observation;
        let o = Observation {
            id: Some("1".into()),
            scientific_name: "Cypripedium acaule".into(),
            species: Some("Cypripedium acaule".into()),
            sensitive: true,
            ..Default::default()
        };
        let c = build(&[o], false, &Options::default());
        let cfg = ReportConfig {
            title: "t".into(),
            group_by: vec![],
            top: 0,
        };
        let md = markdown(&report(&c, &cfg));
        assert!(md.contains("remove these names before publishing"));
    }

    #[test]
    fn number_formatting() {
        assert_eq!(num(0), "0");
        assert_eq!(num(999), "999");
        assert_eq!(num(1000), "1,000");
        assert_eq!(num(1234567), "1,234,567");
    }

    #[test]
    fn escaping() {
        assert_eq!(md_escape("a*b_c|"), "a\\*b\\_c\\|");
        assert_eq!(html_escape("<a & \"b\">"), "&lt;a &amp; &quot;b&quot;&gt;");
    }

    #[test]
    fn renders_blocks() {
        let b = vec![
            Block::Heading(2, vec![text("T")]),
            Block::List {
                ordered: true,
                items: vec![vec![Span::Sci("Quercus alba".into()), text(" — oak")]],
            },
        ];
        assert_eq!(markdown(&b), "## T\n\n1. *Quercus alba* — oak\n\n");
        let h = html(&b, "x");
        assert!(h.contains("<h2>T</h2>"));
        assert!(h.contains("<ol>\n<li><i>Quercus alba</i> — oak</li>\n</ol>"));
    }
}
