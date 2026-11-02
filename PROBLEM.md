---
slug: bioblitz-checklist
title: CLI that turns an iNaturalist observation export into a printable, taxonomically grouped bioblitz species checklist and report
verdict: build
---

## Problem

A bioblitz is a short, intensive event where volunteers record every species
they can find on a property (a park, preserve, farm, school ground). Nearly
all of them now record on iNaturalist. Afterwards the organizer owes the
landowner or land manager a report: how many species, which groups, a species
list in taxonomic order, what was new for the site, and what was notable.

iNaturalist cannot produce that list. Organizers export thousands of
observation rows and then fight spreadsheets: removing duplicate species,
rolling subspecies up to species, deciding whether a genus-only observation
counts, grouping by family and order, counting per group. Sources:

- iNaturalist Forum, "Seeking ideas for Bioblitz report to property
  owners/managers" (Dec 2025),
  https://forum.inaturalist.org/t/seeking-ideas-for-bioblitz-report-to-property-owners-managers/73882 :
  the organizer runs events with "4000 observations of 1000 species", and wants
  more than a URL to hand over. A reply asks for "a simple text list in hard
  copy, ordered taxonomically with relevant headings ... together with the
  total number of species and perhaps a breakdown of numbers in higher-level
  taxa ... there's no easy way to generate such a list on iNaturalist". The
  organizer's workaround is Excel deduplication ("That's not what I call
  easy"), and on "something not seen THERE before": "With 1000+ species it's a
  near impossible task without some kind of automation."
- "Exporting species lists" (Oct 2025),
  https://forum.inaturalist.org/t/exporting-species-lists/71563 : "I'm
  currently in a hell of my own making with pivot tables", wants the species
  list "with all desired taxonomic ranks" and with many taxa excluded.
- "Species in Taxonomic Sequence" (2021),
  https://forum.inaturalist.org/t/species-in-taxonomic-sequence/26064 , and
  "How to get a taxonomically organized species list from a project?" (2023),
  https://forum.inaturalist.org/t/how-to-get-a-taxonomically-organized-species-list-from-a-project/40452 ,
  plus at least four more threads asking to export a species list rather than
  observations (9162, 25394, 52196, 3782).

## Who benefits

Bioblitz organizers at land trusts, nature centers, park departments, native
plant and naturalist societies, school science programs and university
classes; project admins making a species reference for a town or preserve.
They find it through the iNaturalist forum (third-party-tools tag), where
these questions recur and where tool links are routinely posted as answers.
They run it on the export they already download: one command, Windows/macOS/
Linux binary, no account or network needed.

## Existing solutions

- **iNaturalist project "Species" tab and `/observations/taxa` view**: on
  screen only, no export; the taxonomic view is buggy with project filters
  (open bug https://forum.inaturalist.org/t/project-id-does-not-work-on-observations-taxa/3934)
  and refuses large queries ("too many taxa to render").
- **iNaturalist observation export** (https://www.inaturalist.org/observations/export):
  has all rank columns as "taxon extras", but one row per observation.
  This is the input people fight with in Excel.
- **pisum's stirfry species counts page**
  (https://jumear.github.io/stirfry/iNatAPIv1_observations_species_counts.html)
  and **kildor's species tool** (https://kildor.name/inat/species): browser
  tools that give a flat species CSV via the API. Users must build API query
  URLs; output has no higher ranks or grouping ("I'd still have to go in and
  add higher divisions above genus manually", thread 71563), no report, no
  comparison against a prior list.
- **Project stats slideshow**: fun on screen, not a document.
- **mandrewj/bioblitz** (https://github.com/mandrewj/bioblitz): a Next.js
  dashboard deployed on Vercel for one lab's views; needs a developer and
  hosting.
- **benkeen/inat-curated-species-list**: builds a curated checklist from
  expert identifiers via the API for a website; different job, JS setup.
- **ollybolly/iNaturalist_Bioblitz_photo_presentations**,
  **NatureServe/inat-nps-download**, forum R scripts: require R skills.
- **Excel pivot tables / deduplication**: what organizers do now; slow, and
  they get leaf-taxon counting and subspecies roll-up wrong.
- crates.io: `inaturalist` (API bindings) only; nothing for checklists.

## Why build anything

The need recurs on the forum from 2019 to late 2025 and the answers are
workarounds. None of the existing tools takes the export the organizer already
has and produces the document they need: a grouped, counted, taxonomically
ordered checklist with a summary, iNaturalist-style species counting, and
"new for this site" flags against a previous list or a GBIF species list. The
browser tools need API URL skills and stop at a flat list; the R and JS tools
need programming.

## Smallest useful intervention

One CLI command, `bioblitz-checklist`, that reads one or more iNaturalist
observation CSV exports and writes a Markdown, self-contained printable HTML,
or CSV checklist:

- summary: observations, observers (count only), dates, species, taxa,
  research-grade share, per-group counts (plants, birds, insects ...);
- full checklist grouped under configurable rank headings (default kingdom,
  class, order, family), each heading with its species count;
- counting like iNaturalist: subspecies roll up to species; genus- or
  family-only identifications count only when no species under them was found;
- highlights: most observed species, species iNaturalist auto-obscures
  (taxon geoprivacy, usually threatened taxa), and with `--known` a list of
  species not on a previous checklist (prior export, GBIF species-list
  download, or a plain text list);
- filters: research grade only, exclude taxa by name at any rank, casual and
  captive observations excluded by default.

## Success criterion

- On a fixture export, species and taxon counts match a hand count that
  follows iNaturalist's leaf-taxon rule, including subspecies and
  genus-only rows.
- A 5,000-row export becomes a printable report in under a second, with one
  command and no network.
- Re-feeding overlapping exports does not change any count.

## Maintenance

Low. No network, no API, no server. What can break it: iNaturalist renaming
export columns (the parser matches by header name and degrades to grouping by
iconic taxon if rank columns are missing), or changes in GBIF species-list
column names (`--known` also accepts a plain list). Dependencies: the `csv`
crate only.

## Also considered

- winlink-checkin-tally (tally Winlink net check-in forms for net managers): KGBurton/Winlink_Checkins and existing net-manager templates and tallies already cover it.
- lpfm-issues-programs-list (quarterly issues/programs list for volunteer LPFM stations): LPFM stations are exempt per REC Networks' FAQ, and Radio3K QuIPList serves full-power stations.
- her-nutrition-ranking (rank a food bank inventory by HER nutrition guidelines / SWAP): WellSCAN (UConn Rudd Center) and food-bank ordering systems already provide free rankings.

## Decision

Build. Demand is sourced and recurring, the input is a file organizers
already download, existing tools stop at flat lists or need programming, and
the tool is small, offline and cheap to maintain. It is distinct from the
earlier rejected `inat-species-timeseries` idea (yearly counts), which forum
answers already covered.
