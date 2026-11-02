# bioblitz-checklist

Turn an iNaturalist observation export into the species checklist and summary
report you hand to a landowner after a bioblitz: grouped by kingdom, class,
order and family, counted the way iNaturalist counts species, with highlights
and "new for this site" flags. Markdown, printable HTML, or CSV. Works
offline.

Repository: https://github.com/SignalForHumanity/bioblitz-checklist ·
Why this exists: [PROBLEM.md](PROBLEM.md)

## The problem

After a bioblitz the organizer owes the property owner or manager a report.
iNaturalist shows the species on screen but cannot export a species list,
let alone one in taxonomic order with counts per group. Organizers download
thousands of observation rows and wrestle with Excel: deduplicating,
rolling subspecies up, deciding whether a genus-only observation counts,
and building pivot tables per family. On the iNaturalist forum
([Dec 2025](https://forum.inaturalist.org/t/seeking-ideas-for-bioblitz-report-to-property-owners-managers/73882),
[Oct 2025](https://forum.inaturalist.org/t/exporting-species-lists/71563),
and many earlier threads) people keep asking for exactly this.

## Who it's for

Bioblitz organizers at land trusts, nature centers, parks, naturalist and
native plant societies, schools and universities, and iNaturalist project
admins writing a species reference for a place.

## Install

Build with Rust (no prebuilt binaries are published yet):

```sh
cargo install --git https://github.com/SignalForHumanity/bioblitz-checklist
```

## Usage

1. On iNaturalist, open the export page:
   <https://www.inaturalist.org/observations/export>. Pick your project
   (or place and dates). Under columns, **tick "Taxon extras"** (the rank
   columns: kingdom, phylum, class, order, family, genus, species ...).
2. Download and unzip the CSV.
3. Run:

```sh
bioblitz-checklist observations-12345.csv -t "Hilltop Preserve BioBlitz 2025" -o report.html
```

Open `report.html` in a browser and print it, or save as PDF. Use
`-o report.md` for Markdown (pastes into Google Docs, Word via Pandoc, or a
web page) and `-o checklist.csv` for a spreadsheet.

See [examples/hilltop-report.md](examples/hilltop-report.md) for the output
on the test fixture.

### Options

```
-o, --output FILE      Write to FILE instead of standard output
-f, --format FORMAT    md, html or csv (default: from -o extension, else md)
-t, --title TEXT       Report title
-g, --group-by RANKS   Heading ranks, coarse to fine (default:
                       kingdom,class,order,family; "none" for a flat list)
-k, --known FILE       Previous species list for the site; species not on
                       it are flagged NEW
-x, --exclude NAMES    Drop these taxa at any rank, e.g. -x "Homo sapiens,Erysiphaceae"
    --research-only    Count only research-grade observations
    --include-casual   Also count casual and captive/cultivated observations
    --top N            Number of most-observed taxa to list (default 10)
```

Several exports can be given at once (for example one per day); an
observation that appears in more than one is counted once.

### What is new for this site?

`--known` takes any of:

- last year's iNaturalist export for the same property;
- a GBIF species list: on gbif.org search occurrences for your county or
  property, then *Download → Species list* (tab-separated);
- a plain text file with one scientific name per line (authorship is
  ignored).

Species not on that list are flagged **NEW** and listed under Highlights.

## What the report contains

- **Summary**: observations, species, taxa, observers (a count only), dates,
  research-grade share, and what was left out and why.
- **By group**: species, taxa and observations for plants, birds, insects,
  fungi and so on.
- **Highlights**: species not on the previous list; *sensitive taxa*
  (iNaturalist hides their locations, usually because they are threatened);
  most observed; how many were recorded only once.
- **Checklist**: every counted taxon under rank headings with species counts,
  common name, observations, observers, and flags (not yet research grade,
  NEW, sensitive).

### How species are counted

Like iNaturalist's project species count:

- Subspecies, varieties and forms count under their species.
- An observation identified only to genus or family counts as a taxon only if
  nothing under it was identified to species (an unidentified *Quercus* does
  not add a taxon if *Quercus alba* was found; an unidentified *Carex* does if
  no sedge species was).
- "Species" counts species only; "taxa" adds those coarser leaf taxa.
- Casual and captive/cultivated observations are left out unless you pass
  `--include-casual`.

## Data sources

Only the CSV files you give it: iNaturalist's observation export
(https://www.inaturalist.org/observations/export) and optionally a previous
list or a GBIF species-list download. No network access, no account, no API
key.

## Privacy

The report never prints usernames, coordinates or place descriptions. It
counts observers but does not list them. Sensitive taxa are listed by name
only, but a report that names a property still places them there, more
precisely than iNaturalist's obscured locations do. The report says so: give
it to the land manager, and delete the sensitive taxa before publishing it.

## Limitations

- Sorting is pseudo-taxonomic (alphabetical within each rank), as on
  iNaturalist. It is not a phylogenetic sequence such as eBird's.
- It reports what the export says. It does not check identifications,
  conservation status lists, or whether a species is native; the
  "sensitive" flag only reflects iNaturalist's taxon geoprivacy.
- Without the "Taxon extras" columns it can only group by iconic group
  (plants, birds ...), and genus-only identifications are matched by name.
- "New for this site" is only as good as the list you compare against.
- Dates are taken as written in `observed_on` (YYYY-MM-DD).

## Development

```sh
cargo test
```

Tests use small fixture exports in `tests/fixtures/` and need no network.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE),
at your option.
