//! bioblitz-checklist: turn iNaturalist observation exports into a
//! taxonomically grouped species checklist and summary report.

mod checklist;
mod input;
mod render;

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use checklist::Options;
use render::ReportConfig;

const USAGE: &str = "\
bioblitz-checklist: species checklist and summary from iNaturalist exports

USAGE:
    bioblitz-checklist [OPTIONS] <EXPORT.csv>...

Export observations at https://www.inaturalist.org/observations/export and
tick the \"Taxon extras\" columns so the checklist can be grouped by rank.
Several exports may be given; observations appearing twice are counted once.

OPTIONS:
    -o, --output FILE      Write to FILE instead of standard output
    -f, --format FORMAT    md, html or csv (default: from -o extension, else md)
    -t, --title TEXT       Report title (default: \"Species checklist\")
    -g, --group-by RANKS   Comma-separated heading ranks, coarse to fine
                           (default: kingdom,class,order,family; \"none\" for a
                           flat list)
    -k, --known FILE       Previous species list for the site (iNaturalist
                           export, GBIF species list, or one name per line);
                           species not on it are flagged as new
    -x, --exclude NAMES    Drop observations of these taxa (any rank); comma
                           separated, may be repeated
        --research-only    Count only research-grade observations
        --include-casual   Also count casual and captive/cultivated observations
        --top N            Number of most-observed taxa to list (default 10)
    -h, --help             Show this help
    -V, --version          Show version
";

#[derive(Debug, PartialEq)]
enum Format {
    Markdown,
    Html,
    Csv,
}

#[derive(Debug)]
struct Args {
    inputs: Vec<PathBuf>,
    output: Option<PathBuf>,
    format: Format,
    title: String,
    group_by: Vec<usize>,
    known: Option<PathBuf>,
    opts: Options,
    top: usize,
}

enum Parsed {
    Run(Box<Args>),
    Exit(String),
}

fn parse_format(s: &str) -> Result<Format, String> {
    match s.to_ascii_lowercase().as_str() {
        "md" | "markdown" => Ok(Format::Markdown),
        "html" | "htm" => Ok(Format::Html),
        "csv" => Ok(Format::Csv),
        other => Err(format!("unknown format `{other}` (use md, html or csv)")),
    }
}

fn parse_group_by(s: &str) -> Result<Vec<usize>, String> {
    if s.trim().eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let mut v = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let i = input::rank_index(part).ok_or_else(|| {
            format!(
                "unknown rank `{part}`; choose from {}",
                input::RANKS.join(", ")
            )
        })?;
        v.push(i);
    }
    v.sort_unstable();
    v.dedup();
    if v.len() > 4 {
        return Err("at most 4 heading ranks are supported".to_string());
    }
    Ok(v)
}

fn parse_args(raw: &[String]) -> Result<Parsed, String> {
    let mut inputs = Vec::new();
    let mut output = None;
    let mut format = None;
    let mut title = "Species checklist".to_string();
    let mut group_by = parse_group_by("kingdom,class,order,family")?;
    let mut known = None;
    let mut opts = Options::default();
    let mut top = 10;

    let mut it = raw.iter();
    while let Some(a) = it.next() {
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ => (a.as_str(), None),
        };
        let mut value = |name: &str| -> Result<String, String> {
            inline
                .clone()
                .or_else(|| it.next().cloned())
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match flag {
            "-h" | "--help" => return Ok(Parsed::Exit(USAGE.to_string())),
            "-V" | "--version" => {
                return Ok(Parsed::Exit(format!(
                    "bioblitz-checklist {}\n",
                    env!("CARGO_PKG_VERSION")
                )));
            }
            "-o" | "--output" => output = Some(PathBuf::from(value(flag)?)),
            "-f" | "--format" => format = Some(parse_format(&value(flag)?)?),
            "-t" | "--title" => title = value(flag)?,
            "-g" | "--group-by" => group_by = parse_group_by(&value(flag)?)?,
            "-k" | "--known" => known = Some(PathBuf::from(value(flag)?)),
            "-x" | "--exclude" => opts.exclude.extend(
                value(flag)?
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from),
            ),
            "--research-only" => opts.research_only = true,
            "--include-casual" => opts.include_casual = true,
            "--top" => {
                top = value(flag)?
                    .parse()
                    .map_err(|_| "--top needs a whole number".to_string())?
            }
            s if s.starts_with('-') && s.len() > 1 => {
                return Err(format!("unknown option `{s}`; see --help"));
            }
            _ => inputs.push(PathBuf::from(a)),
        }
    }
    if inputs.is_empty() {
        return Err("no export file given; see --help".to_string());
    }
    let format = match format {
        Some(f) => f,
        None => output
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|e| parse_format(&e.to_string_lossy()).ok())
            .unwrap_or(Format::Markdown),
    };
    Ok(Parsed::Run(Box::new(Args {
        inputs,
        output,
        format,
        title,
        group_by,
        known,
        opts,
        top,
    })))
}

fn run(mut args: Args) -> Result<(), String> {
    let mut observations = Vec::new();
    let mut has_ranks = true;
    for path in &args.inputs {
        let f = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let e = input::read_export(std::io::BufReader::new(f))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if !e.has_ranks {
            has_ranks = false;
            eprintln!(
                "note: {} has no taxon rank columns; grouping by iconic group only. \
                 Tick \"Taxon extras\" when exporting for a full taxonomic checklist.",
                path.display()
            );
        }
        observations.extend(e.observations);
    }
    if let Some(k) = &args.known {
        let text = fs::read_to_string(k).map_err(|e| format!("{}: {e}", k.display()))?;
        let set = input::read_known(&text);
        if set.is_empty() {
            return Err(format!("{}: no names found in known list", k.display()));
        }
        args.opts.known = Some(set);
    }
    let c = checklist::build(&observations, has_ranks, &args.opts);
    let cfg = ReportConfig {
        title: args.title.clone(),
        group_by: args.group_by.clone(),
        top: args.top,
    };
    let out = match args.format {
        Format::Markdown => render::markdown(&render::report(&c, &cfg)),
        Format::Html => render::html(&render::report(&c, &cfg), &cfg.title),
        Format::Csv => render::csv(&c)?,
    };
    match &args.output {
        Some(p) => fs::write(p, out).map_err(|e| format!("{}: {e}", p.display()))?,
        None => std::io::stdout()
            .write_all(out.as_bytes())
            .map_err(|e| e.to_string())?,
    }
    eprintln!(
        "{} species, {} taxa from {} observations.",
        c.summary.species, c.summary.taxa, c.summary.observations
    );
    Ok(())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&raw) {
        Ok(Parsed::Exit(msg)) => {
            print!("{msg}");
            ExitCode::SUCCESS
        }
        Ok(Parsed::Run(args)) => match run(*args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Result<Parsed, String> {
        parse_args(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn parses_options() {
        let Parsed::Run(a) = args(&[
            "a.csv",
            "-o",
            "out.html",
            "--group-by=family,order",
            "-x",
            "Erysiphaceae, Homo sapiens",
            "--research-only",
            "b.csv",
        ])
        .unwrap() else {
            panic!("expected run")
        };
        assert_eq!(a.inputs.len(), 2);
        assert_eq!(a.format, Format::Html);
        assert_eq!(a.group_by, vec![7, 10]);
        assert_eq!(a.opts.exclude, vec!["Erysiphaceae", "Homo sapiens"]);
        assert!(a.opts.research_only);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(args(&[]).is_err());
        assert!(args(&["a.csv", "--bogus"]).is_err());
        assert!(args(&["a.csv", "-g", "clade"]).is_err());
        assert!(args(&["a.csv", "-f", "pdf"]).is_err());
        assert!(args(&["a.csv", "--top"]).is_err());
        assert!(matches!(args(&["-h"]), Ok(Parsed::Exit(_))));
    }

    #[test]
    fn group_by_none_is_flat() {
        assert!(parse_group_by("none").unwrap().is_empty());
    }
}
