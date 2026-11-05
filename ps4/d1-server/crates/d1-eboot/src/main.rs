// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-eboot` - one binary for static eboot analysis.
//!
//! ```text
//! d1-eboot segments                                     # the per-segment address map
//! d1-eboot descriptors --index                          # every anchored descriptor, by tag
//! d1-eboot descriptors --tag 0x80801803                 # walk a schema tree
//! d1-eboot descriptors --tag 0x80801796 --raw           # hexdump one descriptor
//! d1-eboot disasm 0x261be0 --to 0x262100                # prologue-anchored range
//! d1-eboot disasm 0x5719f2 --snap --count 40            # snap back to the enclosing prologue
//! d1-eboot xref 0x56ffb0                                # ALL FOUR reference kinds + controls
//! d1-eboot xref 0x26a3888 --kind riprel --show
//! d1-eboot field-xref --class c_activity_loader --global 0x267af08 --offset 8
//!                                                       # who READS/WRITES object+8 -
//!                                                       # the technique `xref` is BLIND to
//! d1-eboot reloc 0x25a330 0x261fb0                      # who points here
//! d1-eboot reloc --slot 0x1924ec8 --span 0x40           # what a vtable region points at
//! d1-eboot rtti --tsv                                   # 2,633 class names, vtables, hierarchy
//! d1-eboot rtti --name investment                       # find a class by (sub)string
//! d1-eboot atlas --tsv                                  # 35,834 function rows, generated
//! d1-eboot atlas --addr 0x574220                        # one function, everything known about it
//! d1-eboot nametab --tsv                                # the ~172 {index, name_hash} enum tables
//! d1-eboot strings --hash 0xb084dd2b                    # FNV-1 name-hash oracle (NOT FNV-1a)
//! d1-eboot callgraph                                    # 151,630 edges: call AND jmp
//! d1-eboot catalogue                                    # BAP message catalogue: does the
//!                                                       # client want a reply AT ALL?
//! d1-eboot catalogue --policies                         # 33 policy families; +0x30 is the answer
//! d1-eboot ladder                                       # the 35-rung boot/activity ladder,
//!                                                       # from the client's own RTTI
//! d1-eboot ladder --json                                # what `d1-trace dash` consumes
//! d1-eboot ladder --rung 28 --slot update               # one rung's handler, annotated
//! d1-eboot controls                                     # every pinned control, PASS/FAIL
//! ```
//!
//! Every subcommand takes `--json`. Exit status `2` means a **positive control failed** and the
//! output must not be cited.

use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use d1_eboot::atlas;
use d1_eboot::catalogue;
use d1_eboot::descriptors;
use d1_eboot::disasm::{self, Anchor};
use d1_eboot::dynsym;
use d1_eboot::fieldxref;
use d1_eboot::functions;
use d1_eboot::image::{addresses_apply, default_eboot, Image, PINNED_EBOOT_SHA256};
use d1_eboot::ladder;
use d1_eboot::nametab;
use d1_eboot::reloc;
use d1_eboot::rtti;
use d1_eboot::strings;
use d1_eboot::xref::{self, Control, RefKind};

fn parse_int(s: &str) -> std::result::Result<u64, String> {
    let t = s.trim();
    let r = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16)
    } else {
        t.parse::<u64>()
    };
    r.map_err(|e| format!("{t:?}: {e}"))
}

#[derive(Parser)]
#[command(name = "d1-eboot", about = "Static analysis of the Destiny 1 PS4 eboot", version)]
struct Cli {
    /// Image to analyse. Defaults to $D1_EBOOT, else the selected build (capture-rig/builds.json, $D1_BUILD).
    #[arg(long, global = true)]
    eboot: Option<String>,
    /// Machine-readable output.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Export names, vtables, inheritance and RSAT layouts without reference-build addresses.
    WireCatalog {
        /// Optional private investment-message definition table (content, not the executable).
        #[arg(long)]
        definitions: Option<String>,
    },
    /// Program segments and the per-segment bias (ATLAS §4.1).
    Segments,
    /// Anchored RSAT/investment schema descriptors.
    Descriptors {
        /// Walk this tag's schema tree. Repeatable.
        #[arg(long, value_parser = parse_int)]
        tag: Vec<u64>,
        /// List every descriptor found, by tag.
        #[arg(long)]
        index: bool,
        /// With --index --json: include every descriptor's raw field records. This is the one
        /// call `capture-rig/eboot-descriptors.py` (now a wrapper) makes for a whole-image walk.
        #[arg(long)]
        with_fields: bool,
        /// Hexdump the descriptor object instead of walking it.
        #[arg(long)]
        raw: bool,
    },
    /// Prologue-anchored disassembly of one range.
    Disasm {
        #[arg(value_parser = parse_int)]
        start: u64,
        /// Exclusive end vaddr.
        #[arg(long, value_parser = parse_int)]
        to: Option<u64>,
        /// Instruction count instead of an end address.
        #[arg(long)]
        count: Option<usize>,
        /// Snap back to the nearest real prologue before decoding.
        #[arg(long)]
        snap: bool,
        /// Decode from `start` even though it is not a prologue. Read ATLAS §4.5 first.
        #[arg(long)]
        force: bool,
    },
    /// Cross-references: call / jmp / rip-relative / relocation addend.
    Xref {
        #[arg(value_parser = parse_int)]
        target: u64,
        /// Comma-separated: call,jmp,riprel,rela - default all four.
        #[arg(long, default_value = "all")]
        kind: String,
        /// Extra independent control, `KIND:TARGET:SITE`. Repeatable.
        #[arg(long)]
        control: Vec<String>,
        /// Run with NO positive control. The report is then marked `is_evidence: false`.
        #[arg(long)]
        no_control: bool,
        /// Disassemble each confirmed site in context.
        #[arg(long)]
        show: bool,
        #[arg(long, default_value_t = 12)]
        context: usize,
    },
    /// Field-level cross-references: who READS and who WRITES `object + offset`.
    ///
    /// The complement to `xref`, which is blind to register-relative field access by
    /// construction. Give it an RTTI class, a singleton global, or both.
    FieldXref {
        /// RTTI class whose methods hold the object, e.g. c_activity_loader.
        #[arg(long)]
        class: Option<String>,
        /// Singleton global the accessor returns, e.g. 0x267af08.
        #[arg(long, value_parser = parse_int)]
        global: Option<u64>,
        /// Byte offset of the field within the object.
        #[arg(long, value_parser = parse_int)]
        offset: u64,
        /// Hops of argument forwarding to follow (the object handed on in RDI).
        #[arg(long, default_value_t = 1)]
        depth: usize,
        /// Also print displacement matches whose base register is NOT proven to be the object.
        #[arg(long)]
        all: bool,
        /// List the candidate functions and how each was resolved.
        #[arg(long)]
        candidates: bool,
        /// Run with NO pinned control. The report is then marked `is_evidence: false`.
        #[arg(long)]
        no_control: bool,
    },
    /// Relocation lookups.
    Reloc {
        /// Addends to look up: who points at these.
        #[arg(value_parser = parse_int)]
        targets: Vec<u64>,
        /// Walk a slot region instead: what does this vtable point at.
        #[arg(long, value_parser = parse_int)]
        slot: Option<u64>,
        #[arg(long, value_parser = parse_int, default_value_t = 0x20)]
        span: u64,
    },
    /// C++ RTTI: class names, hierarchy, vtables and vtable slots.
    Rtti {
        /// Only classes whose demangled name contains this substring (case-insensitive).
        #[arg(long)]
        name: Option<String>,
        /// Only the class owning this typeinfo or vtable pointer.
        #[arg(long, value_parser = parse_int)]
        ti: Option<u64>,
        /// Show each vtable's slots.
        #[arg(long)]
        slots: bool,
        /// One tab-separated row per class - the generated artifact.
        #[arg(long)]
        tsv: bool,
    },
    /// The generated function atlas: one row per prologue-anchored function.
    Atlas {
        /// NID candidate dictionary for import naming. Defaults to capture-rig/nid-candidates.txt.
        #[arg(long)]
        dict: Vec<String>,
        /// One tab-separated row per function - the generated artifact.
        #[arg(long)]
        tsv: bool,
        /// Everything known about the function containing this address.
        #[arg(long, value_parser = parse_int)]
        addr: Option<u64>,
        /// Coverage summary only.
        #[arg(long)]
        coverage: bool,
    },
    /// The compiled `{index, name_hash}` enum tables (~172 of them).
    Nametab {
        /// The registrar to scan call sites of.
        #[arg(long, value_parser = parse_int, default_value_t = d1_eboot::nametab::REGISTRAR)]
        registrar: u64,
        #[arg(long)]
        tsv: bool,
    },
    /// The string corpus and the FNV-1 name-hash oracle.
    Strings {
        /// Resolve these FNV-1 hashes back to corpus spellings. Repeatable.
        #[arg(long, value_parser = parse_int)]
        hash: Vec<u64>,
        /// Hash these spellings (FNV-1, and FNV-1 of the lowercased form).
        #[arg(long)]
        name: Vec<String>,
        /// Report where each hash occurs as a 32-bit immediate in code.
        #[arg(long)]
        sites: bool,
        /// Counts by string class.
        #[arg(long)]
        summary: bool,
    },
    /// The direct call graph: one `caller<TAB>callee` edge per line, `call` and `jmp` alike.
    Callgraph,
    /// `this`-preserving call edges: caller passed its OWN `this` in RDI.
    Thiscalls,
    /// The 638 PLT imports: NID, GOT slot, PLT stub, and a name if a dictionary confirms it.
    Imports {
        /// File of candidate spellings, one per line. NID is one-way, so wrong guesses are free.
        #[arg(long)]
        dict: Vec<String>,
        #[arg(long)]
        tsv: bool,
    },
    /// The complete BAP / investment message catalogue - the "does the client want a reply"
    /// oracle. Port of `capture-rig/message-catalogue.py`; with no section flag it prints
    /// everything. Controls always run first; exit 2 means they failed and NOTHING below the
    /// controls block was printed (the prototype exited 1 - the crate convention wins here).
    Catalogue {
        /// Controls only (CI-friendly). Like the prototype, this suppresses every section even
        /// when other section flags are also given.
        #[arg(long)]
        controls: bool,
        /// The 63 BAP envelope message classes.
        #[arg(long)]
        envelopes: bool,
        /// 33 policy families + 13 push policies.
        #[arg(long)]
        policies: bool,
        /// 101 investment ids joined to their schemas.
        #[arg(long)]
        investment: bool,
        /// RSAT schema records + the record-base rule.
        #[arg(long)]
        schemas: bool,
        /// Cross-check d1-bap's TYPE_* constants.
        #[arg(long)]
        d1bap: bool,
        /// Write the machine-readable everything-blob to this path (the prototype's `--json OUT`).
        /// The global `--json` prints the same blob to stdout instead.
        #[arg(long)]
        json_out: Option<String>,
        /// investment-message-definitions.bin (CONTENT, not eboot - pkg 303 entry 890).
        #[arg(long)]
        defs: Option<String>,
        /// d1-bap lib.rs for the TYPE_* cross-check.
        #[arg(long)]
        d1bap_src: Option<String>,
    },
    /// The world-controller boot/activity ladder, read out of the client's own RTTI - the port
    /// of `capture-rig/state-machine.py` (same outputs, same controls, no subprocesses). With no
    /// flags: the whole 35-rung table. `--json` is the shape `d1-trace dash` consumes.
    Ladder {
        /// Controls only, nothing else printed; exit 2 on failure.
        #[arg(long)]
        controls: bool,
        /// Read this rung's handler functions, annotated.
        #[arg(long)]
        rung: Option<u32>,
        /// With --rung, read only this slot.
        #[arg(long, value_parser = ["enter", "exit", "slot10", "slot40", "update", "update_guard"])]
        slot: Option<String>,
        /// Read one function by address (hex ok), annotated.
        #[arg(long = "fn", value_parser = parse_int)]
        function: Option<u64>,
        /// Atlas TSV for names and extents (default capture-rig/eboot-atlas.tsv).
        #[arg(long)]
        atlas_tsv: Option<String>,
        /// Diagnostic-catalogue TSV for string refs (default capture-rig/diagnostic-catalogue.tsv).
        #[arg(long)]
        diag_tsv: Option<String>,
    },
    /// Run every pinned positive control and report PASS/FAIL.
    Controls,
}

/// The investment message-definition table is shipped CONTENT, not eboot (pkg 303 entry 890).
const DEFAULT_DEFS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../capture-rig/schemas/investment-message-definitions.bin"
);
const DEFAULT_D1BAP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../d1-bap/src/lib.rs");

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("d1-eboot: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let path = cli
        .eboot
        .clone()
        .unwrap_or_else(default_eboot);
    let image = Image::open(&path).with_context(|| format!("opening {path}"))?;

    match &cli.cmd {
        Cmd::WireCatalog { definitions } => {
            let definitions = definitions
                .as_ref()
                .map(|path| -> Result<_> {
                    let bytes =
                        std::fs::read(path).context("reading private service definitions")?;
                    catalogue::investment_defs(&bytes).map_err(anyhow::Error::msg)
                })
                .transpose()?;
            let r = catalogue::Reader::new(&image)?;
            let rtti = rtti::harvest(&image)?;
            let idx = descriptors::index(&image)?;
            anyhow::ensure!(
                !rtti.types.is_empty() && !idx.is_empty(),
                "empty RTTI or RSAT export"
            );
            let anchors: Vec<_> = idx.iter().map(|(_, d)| d.anchor).collect();
            // These builders derive addresses from this image. Do not call Data::build here:
            // its policy/header tables and controls still contain 01.29-only addresses.
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "format_version": 1,
                    "eboot_sha256": image.sha256(),
                    "rtti": rtti,
                    "schemas": descriptors::dump_with_fields(&image, &idx),
                    "envelopes": catalogue::envelope_classes(&r),
                    "instances": catalogue::schema_records(&r, &anchors),
                    "investment_definitions": definitions,
                }))?
            );
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Segments => cmd_segments(&image, cli.json),
        Cmd::Descriptors { tag, index, with_fields, raw } => {
            cmd_descriptors(&image, tag, *index, *with_fields, *raw, cli.json)
        }
        Cmd::Disasm { start, to, count, snap, force } => {
            cmd_disasm(&image, *start, *to, *count, *snap, *force, cli.json)
        }
        Cmd::Xref { target, kind, control, no_control, show, context } => {
            cmd_xref(&image, *target, kind, control, *no_control, *show, *context, cli.json)
        }
        Cmd::FieldXref { class, global, offset, depth, all, candidates, no_control } => cmd_field_xref(
            &image,
            class.as_deref(),
            *global,
            *offset as i64,
            *depth,
            *all,
            *candidates,
            *no_control,
            cli.json,
        ),
        Cmd::Reloc { targets, slot, span } => cmd_reloc(&image, targets, *slot, *span, cli.json),
        Cmd::Rtti { name, ti, slots, tsv } => {
            cmd_rtti(&image, name.as_deref(), *ti, *slots, *tsv, cli.json)
        }
        Cmd::Atlas { dict, tsv, addr, coverage } => {
            cmd_atlas(&image, dict, *tsv, *addr, *coverage, cli.json)
        }
        Cmd::Nametab { registrar, tsv } => cmd_nametab(&image, *registrar, *tsv, cli.json),
        Cmd::Strings { hash, name, sites, summary } => {
            cmd_strings(&image, hash, name, *sites, *summary, cli.json)
        }
        Cmd::Imports { dict, tsv } => cmd_imports(&image, dict, *tsv, cli.json),
        Cmd::Callgraph => cmd_callgraph(&image),
        Cmd::Thiscalls => {
            let ix = functions::build(&image)?;
            let mut e = ix.this_calls.clone();
            e.sort_unstable();
            e.dedup();
            println!("caller\tcallee");
            for (a, b) in e {
                println!("{a:#x}\t{b:#x}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Catalogue {
            controls,
            envelopes,
            policies,
            investment,
            schemas,
            d1bap,
            json_out,
            defs,
            d1bap_src,
        } => cmd_catalogue(
            &image,
            *controls,
            *envelopes,
            *policies,
            *investment,
            *schemas,
            *d1bap,
            json_out.as_deref(),
            defs.as_deref(),
            d1bap_src.as_deref(),
            cli.json,
        ),
        Cmd::Ladder { controls, rung, slot, function, atlas_tsv, diag_tsv } => cmd_ladder(
            &image,
            *controls,
            *rung,
            slot.as_deref(),
            *function,
            atlas_tsv.as_deref(),
            diag_tsv.as_deref(),
            cli.json,
        ),
        Cmd::Controls => cmd_controls(&image, cli.json),
    }
}

/// The `state-machine.py` port. Behaviour parity is pinned by `tests/ladder.rs` against the
/// prototype's own outputs, byte for byte; the one deliberate deviation (frame-pointer-omitted
/// handler functions decode instead of silently printing an empty body) is documented in
/// `ladder.rs`'s module docs.
#[allow(clippy::too_many_arguments)]
fn cmd_ladder(
    image: &Image,
    controls_only: bool,
    rung: Option<u32>,
    slot: Option<&str>,
    function: Option<u64>,
    atlas_tsv: Option<&str>,
    diag_tsv: Option<&str>,
    json: bool,
) -> Result<ExitCode> {
    let atlas_path = atlas_tsv.unwrap_or(ladder::DEFAULT_ATLAS_TSV);
    let diag_path = diag_tsv.unwrap_or(ladder::DEFAULT_DIAG_TSV);
    let annotations = || {
        ladder::load_annotations(std::path::Path::new(atlas_path), std::path::Path::new(diag_path))
            .with_context(|| format!("reading annotation TSVs ({atlas_path}, {diag_path})"))
    };

    if controls_only {
        let ann = annotations()?;
        let (text, ok, _) = ladder::controls(image, &ann)?;
        print!("{text}");
        return Ok(if ok { ExitCode::SUCCESS } else { ExitCode::from(2) });
    }

    if let Some(addr) = function {
        let ann = annotations()?;
        let text = ladder::read_function(image, addr, &ann, 4000, false).map_err(|e| {
            anyhow::anyhow!(
                "{e}\n  (an arbitrary --fn address is decoded only from a real prologue or a \
                 known function start; the prototype printed a silently empty body here. Use \
                 `d1-eboot disasm --snap` to find the enclosing function.)"
            )
        })?;
        print!("{text}");
        return Ok(ExitCode::SUCCESS);
    }

    let handlers = ladder::load_handlers(image)?;
    ladder::require_sane(&handlers).map_err(|e| anyhow::anyhow!(e))?;

    if let Some(r) = rung {
        let Some(h) = handlers.iter().find(|h| h.rung == Some(r)) else {
            eprintln!("no handler for rung {r}");
            return Ok(ExitCode::from(2));
        };
        println!(
            "rung {r}  {}\n  class {}\n  vtable {:#x}",
            h.state.as_deref().unwrap_or("None"),
            h.class,
            h.vtable
        );
        let ann = annotations()?;
        let wanted: Vec<&str> = match slot {
            Some(s) => vec![s],
            None => ladder::RUNG_ORDER.to_vec(),
        };
        for s in wanted {
            let addr = h.slots.by_name(s);
            if addr != 0 && h.overridden.by_name(s) {
                println!("\n\n######## rung {r} :: {s} ########");
                print!("{}", ladder::read_function(image, addr, &ann, 4000, true)?);
            } else if slot.is_some() {
                println!("  {s}: not overridden (inherits the base)");
            }
        }
        return Ok(ExitCode::SUCCESS);
    }

    if json {
        println!("{}", ladder::to_json_py(&handlers));
        return Ok(ExitCode::SUCCESS);
    }
    print!("{}", ladder::render_table(&handlers));
    Ok(ExitCode::SUCCESS)
}

#[allow(clippy::too_many_arguments)]
fn cmd_catalogue(
    image: &Image,
    controls: bool,
    envelopes: bool,
    policies: bool,
    investment: bool,
    schemas: bool,
    d1bap: bool,
    json_out: Option<&str>,
    defs_path: Option<&str>,
    d1bap_src: Option<&str>,
    json_stdout: bool,
) -> Result<ExitCode> {
    let every = !(controls
        || envelopes
        || policies
        || investment
        || schemas
        || d1bap
        || json_out.is_some()
        || json_stdout);

    let defs_path = defs_path.unwrap_or(DEFAULT_DEFS);
    let defs_bytes = std::fs::read(defs_path)
        .with_context(|| format!("reading investment definitions {defs_path}"))?;
    let defs = catalogue::investment_defs(&defs_bytes).map_err(|e| anyhow::anyhow!(e))?;
    let d1bap_source = std::fs::read_to_string(d1bap_src.unwrap_or(DEFAULT_D1BAP)).ok();

    let reader = catalogue::Reader::new(image)?;
    let data = catalogue::Data::build(image, Some(defs), d1bap_source.as_deref())?;

    let (text, ok, _rows) = catalogue::render_controls(image, &reader, &data);
    print!("{text}");
    if !ok {
        return Ok(ExitCode::from(2));
    }
    // The prototype's control flow, kept exactly: --controls suppresses everything else.
    if controls && !every {
        return Ok(ExitCode::SUCCESS);
    }

    if envelopes || every {
        print!("{}", catalogue::render_envelopes(&data));
    }
    if policies || every {
        print!("{}", catalogue::render_policies(&data));
    }
    if schemas || every {
        print!("{}", catalogue::render_schemas(&data));
    }
    if investment || every {
        print!("{}", catalogue::render_investment(&reader, &data));
    }
    if d1bap || every {
        print!("{}", catalogue::render_d1bap(&data));
    }

    if json_out.is_some() || json_stdout {
        let blob = catalogue::json_blob(image, &reader, &data);
        if let Some(path) = json_out {
            std::fs::write(path, serde_json::to_string_pretty(&blob)?)?;
            println!("wrote {path}");
        }
        if json_stdout {
            println!("{}", serde_json::to_string_pretty(&blob)?);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// ⚠ Both edge kinds, always. A `call`-only graph is missing every tail call, which is exactly the
/// two-session bug in ATLAS §4.2 (`0x56FFB0` has ZERO `call` sites and is entered by `jmp`).
fn cmd_imports(image: &Image, dicts: &[String], tsv: bool, json: bool) -> Result<ExitCode> {
    let mut imps = dynsym::imports(image)?;
    let mut cands: Vec<String> = Vec::new();
    for d in dicts {
        cands.extend(std::fs::read_to_string(d)?.lines().map(|l| l.to_string()));
    }
    let n = cands.len();
    let hit = dynsym::resolve(&mut imps, cands);

    // ⭐ POSITIVE CONTROL - and my first attempt at it was aimed wrong, which is ATLAS §4.2b
    // exactly. I fed these four spellings into the IMPORT set expecting them to resolve; they
    // resolved 0/4, because `__cxa_pure_virtual` and the `__cxxabiv1` typeinfo vtables are bound by
    // `R_X86_64_64` in the RELA table (data references) and never appear in `DT_SCE_JMPREL` at all.
    // The oracle was fine; the question was wrong. The control belongs against the FULL symbol
    // table, so that is where it is now.
    let table = dynsym::by_nid(image)?;
    let controls = [
        "__cxa_pure_virtual",
        "_ZTVN10__cxxabiv117__class_type_infoE",
        "_ZTVN10__cxxabiv120__si_class_type_infoE",
        "_ZTVN10__cxxabiv121__vmi_class_type_infoE",
    ];
    let ctl_ok = controls.iter().filter(|c| dynsym::index_of(&table, c).is_some()).count();
    // ⚠ stderr, not stdout: --tsv/--json output must be machine-readable, and a control banner on
    // stdout becomes the header row of whatever parses it.
    eprintln!(
        "POSITIVE CONTROL (NID oracle vs the FULL dynsym table, not the PLT subset): {ctl_ok}/4 {}",
        if ctl_ok == 4 { "PASS" } else { "*** FAIL - do not trust any resolution below ***" }
    );
    if json {
        println!("{}", serde_json::to_string(&imps)?);
        return Ok(ExitCode::SUCCESS);
    }
    if tsv {
        println!("nid	raw	got_slot	plt_stub	name");
        for i in &imps {
            println!(
                "{}	{}	{:#x}	{}	{}",
                i.nid,
                i.raw,
                i.got_slot,
                i.plt_stub.map(|s| format!("{s:#x}")).unwrap_or_default(),
                i.name.clone().unwrap_or_default()
            );
        }
        return Ok(ExitCode::SUCCESS);
    }
    println!(
        "{} imports · {} PLT stubs located · dictionary {} candidates · {} resolved ({:.0}%)",
        imps.len(),
        imps.iter().filter(|i| i.plt_stub.is_some()).count(),
        n,
        hit,
        100.0 * hit as f64 / imps.len() as f64
    );
    for i in imps.iter().filter(|i| i.name.is_some()) {
        println!("  {:#x}  {}  {}", i.plt_stub.unwrap_or(0), i.nid, i.name.clone().unwrap());
    }
    let unresolved = imps.iter().filter(|i| i.name.is_none()).count();
    println!("
{unresolved} unresolved - their spellings are not in any dictionary supplied.");
    Ok(ExitCode::SUCCESS)
}

fn cmd_callgraph(image: &Image) -> Result<ExitCode> {
    let ix = functions::build(image)?;
    let mut edges: Vec<(u64, u64, &str)> = Vec::with_capacity(ix.calls.len() + ix.tailcalls.len());
    edges.extend(ix.calls.iter().map(|&(a, b)| (a, b, "call")));
    edges.extend(ix.tailcalls.iter().map(|&(a, b)| (a, b, "jmp")));
    edges.sort_unstable();
    edges.dedup();
    println!("caller\tcallee\tkind");
    for (a, b, k) in edges {
        println!("{a:#x}\t{b:#x}\t{k}");
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_rtti(
    image: &Image,
    name: Option<&str>,
    ti: Option<u64>,
    slots: bool,
    tsv: bool,
    json: bool,
) -> Result<ExitCode> {
    let r = rtti::harvest(image)?;
    let by_ti: std::collections::HashMap<u64, &rtti::TypeInfo> =
        r.types.iter().map(|t| (t.ti, t)).collect();
    let want = |t: &rtti::TypeInfo| {
        name.map(|n| t.name.to_ascii_lowercase().contains(&n.to_ascii_lowercase())).unwrap_or(true)
            && ti.map(|a| t.ti == a || t.vtables.contains(&a)).unwrap_or(true)
    };
    let sel: Vec<&rtti::TypeInfo> = r.types.iter().filter(|t| want(t)).collect();

    if json {
        println!("{}", serde_json::to_string(&sel)?);
        return Ok(ExitCode::SUCCESS);
    }
    if tsv {
        println!("typeinfo\tkind\tname\tbases\tvtables\tslots\tmangled");
        for t in &sel {
            let n: usize = r
                .vtables
                .iter()
                .filter(|v| v.ti == t.ti)
                .map(|v| v.slots.len())
                .sum();
            println!(
                "{:#x}\t{:?}\t{}\t{}\t{}\t{}\t{}",
                t.ti,
                t.kind,
                t.name,
                t.bases
                    .iter()
                    .map(|b| by_ti.get(b).map(|x| x.name.clone()).unwrap_or(format!("{b:#x}")))
                    .collect::<Vec<_>>()
                    .join(" | "),
                t.vtables.iter().map(|v| format!("{v:#x}")).collect::<Vec<_>>().join(" "),
                n,
                t.mangled
            );
        }
        return Ok(ExitCode::SUCCESS);
    }

    eprintln!(
        "{} typeinfos ({} with a vtable), {} vtables - ABI symbols resolved by NID: {}",
        r.types.len(),
        r.types.iter().filter(|t| !t.vtables.is_empty()).count(),
        r.vtables.len(),
        r.abi_syms
            .iter()
            .map(|(k, v)| format!("{}={v}", k.rsplit("__cxxabiv1").next().unwrap_or(k)))
            .collect::<Vec<_>>()
            .join(" ")
    );
    for t in &sel {
        println!("{:#010x}  {:?}  {}", t.ti, t.kind, t.name);
        for b in &t.bases {
            println!("      base  {:#010x}  {}", b, by_ti.get(b).map(|x| x.name.as_str()).unwrap_or("?"));
        }
        for v in r.vtables.iter().filter(|v| v.ti == t.ti) {
            println!(
                "      vtable {:#010x}  offset_to_top {}  {} slots",
                v.vptr,
                v.offset_to_top,
                v.slots.len()
            );
            if slots {
                for (i, f) in v.slots.iter().enumerate() {
                    if *f == 0 {
                        println!("        [{i:3}] __cxa_pure_virtual");
                    } else {
                        println!("        [{i:3}] {f:#010x}");
                    }
                }
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Dictionaries consulted by default, in order. Missing files are skipped silently - the NID hash
/// is one-way, so a smaller dictionary resolves fewer imports and never a wrong one.
///
/// `ps4_names.txt` is shadPS4's symbol-name database (GPL-2.0, and we ship a fork - licence
/// compatible per `design/licensing.md`). With it, 552 of 638 imports resolve; without it, 366.
const DEFAULT_DICTS: [&str; 2] = [
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../../capture-rig/nid-candidates.txt"),
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../shadPS4-main/scripts/ps4_names.txt"),
];

fn cmd_atlas(
    image: &Image,
    dicts: &[String],
    tsv: bool,
    addr: Option<u64>,
    coverage: bool,
    json: bool,
) -> Result<ExitCode> {
    let mut cands: Vec<String> = Vec::new();
    let paths: Vec<String> = if dicts.is_empty() {
        DEFAULT_DICTS.iter().map(|s| s.to_string()).collect()
    } else {
        dicts.to_vec()
    };
    for d in &paths {
        if let Ok(t) = std::fs::read_to_string(d) {
            cands.extend(t.lines().map(|l| l.to_string()));
        }
    }
    let a = atlas::build(image, &cands)?;
    if let Some(va) = addr {
        // ⛔ Was `Index::function_of`, which returns the greatest start at or below `va` and never
        // consults an end - so `atlas --addr 0x25c0c58` (a DATA address, in another segment
        // entirely) confidently answered `function 0x129f850 ..0x129f868`. `containing` checks the
        // extent and says nothing rather than name a function that does not contain the address.
        let c = a.index.containing(va);
        let Some(f) = c.start else {
            bail!(
                "{va:#x} is not inside any known function - not in an executable segment, or \
                 before the first function start. (This used to answer with the nearest preceding \
                 function, which is how a data address got a function name.)"
            );
        };
        let r = a.rows.iter().find(|r| r.start == f).unwrap();
        if c.how == d1_eboot::functions::Containment::Unproven {
            println!(
                "⚠ UNPROVEN: {va:#x} is inside {f:#x}'s extent but past where its body decode \
                 stopped ({:#x}) - a data island. Containment is plausible, not proven.",
                r.end
            );
        }
        let callers = a.index.callers();
        println!(
            "function      {:#x}  ..{:#x}  ({} bytes, {} insns)   [{}]",
            r.start,
            r.end,
            r.end - r.start,
            r.insns,
            if r.anchored == d1_eboot::functions::Anchored::Prologue {
                "prologue-anchored"
            } else {
                "frame-pointer-omitted (found as a direct call target, not a `55 48 89 e5`)"
            }
        );
        println!("name          {}   [{}]", if r.name.is_empty() { "-" } else { &r.name }, r.name_source);
        if !r.class.is_empty() {
            println!("class         {}  slot {}  (in {} vtables)", r.class, r.vslot.unwrap_or(0), r.vt_shared);
        }
        if !r.source_file.is_empty() {
            println!("source        {}", r.source_file);
        }
        if !r.imports.is_empty() {
            println!("imports       {}", r.imports.join(" | "));
        }
        if !r.strings.is_empty() {
            println!("strings       {}", r.strings.join(" | "));
        }
        println!("callers       {}  callees {}", r.n_callers, r.n_callees);
        println!("warn          {}", if r.warn().is_empty() { "-".into() } else { r.warn() });
        if let Some(c) = callers.get(&r.start) {
            let show: Vec<String> = c.iter().take(12).map(|x| format!("{x:#x}")).collect();
            println!("called by     {}{}", show.join(" "), if c.len() > 12 { " …" } else { "" });
        }
        return Ok(ExitCode::SUCCESS);
    }

    let cov = a.coverage();
    if coverage || json {
        if json {
            println!("{}", serde_json::to_string(&cov)?);
        } else {
            println!("{cov:#?}");
        }
        return Ok(ExitCode::SUCCESS);
    }
    if tsv {
        println!("{}", atlas::TSV_HEADER);
        for r in &a.rows {
            println!("{}", atlas::tsv_row(r));
        }
        return Ok(ExitCode::SUCCESS);
    }
    println!(
        "{} prologue-anchored functions, {} named ({:.1}%)\n\
         + {} frame-pointer-omitted functions (direct call targets, NOT in the denominator), {} named\n\
         {} rows total, {} named ({:.1}%)  [rtti-slot {} · rtti-vptr {} · source-file {} · string-ref {} · name-hash {} · schema-tag {} · thunk {} · import {}]",
        cov.prologue,
        cov.prologue_named,
        100.0 * cov.prologue_named as f64 / cov.prologue as f64,
        cov.call_target,
        cov.call_target_named,
        cov.total,
        cov.named,
        100.0 * cov.named as f64 / cov.total as f64,
        cov.rtti,
        cov.rtti_vptr,
        cov.source_file,
        cov.string_ref,
        cov.name_hash,
        cov.schema_tag,
        cov.thunk,
        cov.import
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_nametab(image: &Image, registrar: u64, tsv: bool, json: bool) -> Result<ExitCode> {
    let mut corpus = strings::Corpus::build(image, 3)?;
    corpus.add_names(rtti::harvest(image)?.types.into_iter().map(|t| t.name));
    let tables = nametab::harvest(image, &corpus, registrar)?;
    if json {
        println!("{}", serde_json::to_string(&tables)?);
        return Ok(ExitCode::SUCCESS);
    }
    if tsv {
        println!("table\tsite\tfunction\tdeclared\trecovered\tindex\thash\tname");
        for t in &tables {
            for e in &t.entries {
                println!(
                    "{}\t{:#x}\t{:#x}\t{}\t{}\t{}\t{:#010x}\t{}",
                    t.table.map(|x| format!("{x:#x}")).unwrap_or_default(),
                    t.site,
                    t.function,
                    t.declared_count.unwrap_or(0),
                    t.entries.len(),
                    e.index,
                    e.hash,
                    e.names.join(" | ")
                );
            }
        }
        return Ok(ExitCode::SUCCESS);
    }
    let entries: usize = tables.iter().map(|t| t.entries.len()).sum();
    let resolved: usize =
        tables.iter().flat_map(|t| &t.entries).filter(|e| !e.names.is_empty()).count();
    println!(
        "{} registrar call sites · {} tables recovered whole · {} entries · {} resolved to a corpus name ({:.0}%)",
        tables.len(),
        tables.iter().filter(|t| t.complete).count(),
        entries,
        resolved,
        100.0 * resolved as f64 / entries.max(1) as f64
    );
    for t in tables.iter().filter(|t| !t.entries.is_empty()) {
        println!(
            "\n{}  fn {:#x}  site {:#x}  {}/{} entries",
            t.table.map(|x| format!("{x:#x}")).unwrap_or_else(|| "?".into()),
            t.function,
            t.site,
            t.entries.len(),
            t.declared_count.unwrap_or(0)
        );
        for e in &t.entries {
            println!("  [{:3}] {:#010x}  {}", e.index, e.hash, e.names.join(" | "));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_strings(
    image: &Image,
    hashes: &[u64],
    names: &[String],
    sites: bool,
    summary: bool,
    json: bool,
) -> Result<ExitCode> {
    let mut corpus = strings::Corpus::build(image, 3)?;
    corpus.add_names(rtti::harvest(image)?.types.into_iter().map(|t| t.name));
    if summary || (hashes.is_empty() && names.is_empty()) {
        let mut by = std::collections::BTreeMap::new();
        for s in &corpus.strings {
            *by.entry(format!("{:?}", s.kind)).or_insert(0usize) += 1;
        }
        if json {
            println!("{}", serde_json::to_string(&by)?);
        } else {
            println!("{} NUL-terminated strings", corpus.strings.len());
            for (k, v) in by {
                println!("  {k:<12} {v}");
            }
        }
        return Ok(ExitCode::SUCCESS);
    }
    let mut targets: Vec<u32> = hashes.iter().map(|h| *h as u32).collect();
    for n in names {
        let a = strings::fnv1(n.as_bytes());
        let b = strings::fnv1_lower(n);
        println!("fnv1({n:?}) = {a:#010x}{}", if b != a { format!("   fnv1(lower) = {b:#010x}") } else { String::new() });
        targets.push(a);
        if b != a {
            targets.push(b);
        }
    }
    let idx = if sites { Some(functions::build(image)?) } else { None };
    for h in targets {
        let cands = corpus.resolve(h);
        println!("{h:#010x}  {}", if cands.is_empty() { "- no corpus spelling".into() } else { cands.join(" | ") });
        if let Some(ix) = &idx {
            let s: Vec<String> = ix
                .imms
                .iter()
                .filter(|&&(_, v)| v == h)
                .map(|&(f, _)| format!("{f:#x}"))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            println!("    immediate in {} function(s): {}", s.len(), s.join(" "));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_segments(image: &Image, json: bool) -> Result<ExitCode> {
    if json {
        println!("{}", serde_json::to_string_pretty(&image.segments())?);
        return Ok(ExitCode::SUCCESS);
    }
    println!("{}", image.path().display());
    println!("  {:>10}  {:>6}  {:>10}  {:>10}  {:>10}  {:>10}  {}", "type", "flags", "file", "vaddr", "filesz", "memsz", "delta");
    for s in image.segments() {
        println!(
            "  {:#010x}  {:#06x}  {:#010x}  {:#010x}  {:#010x}  {:#010x}  {:#x}{}",
            s.p_type,
            s.flags,
            s.offset,
            s.vaddr,
            s.filesz,
            s.memsz,
            s.bias(),
            if s.is_load() && s.memsz > s.filesz {
                format!("   (BSS from vaddr {:#x})", s.vaddr + s.filesz)
            } else {
                String::new()
            }
        );
    }
    println!("\n⚠ the bias is PER SEGMENT (ATLAS §4.1) - 0x4000 is the TEXT case, not a constant");
    Ok(ExitCode::SUCCESS)
}

fn cmd_descriptors(
    image: &Image,
    tags: &[u64],
    index: bool,
    with_fields: bool,
    raw: bool,
    json: bool,
) -> Result<ExitCode> {
    let idx = descriptors::index(image)?;
    if !json {
        eprintln!("{} descriptors indexed from {}", idx.len(), image.path().display());
    }
    if index {
        if json && with_fields {
            println!("{}", serde_json::to_string(&descriptors::dump_with_fields(image, &idx))?);
        } else if json {
            let v: Vec<_> = idx.iter().map(|(_, d)| d).collect();
            println!("{}", serde_json::to_string(&v)?);
        } else {
            for (_, d) in idx.iter() {
                println!(
                    "{:#010x}  anchor {:#010x}  fields {:3}  struct {}",
                    d.tag, d.anchor, d.field_count, d.struct_size
                );
            }
        }
        return Ok(ExitCode::SUCCESS);
    }

    let tags: Vec<u64> = if tags.is_empty() { vec![0x80801825] } else { tags.to_vec() };
    let mut problems = Vec::new();
    let mut walks = Vec::new();
    for &tag in &tags {
        let tag = tag as u32;
        if raw {
            let Some(d) = idx.get(tag) else {
                println!("{tag:#010x} not found");
                continue;
            };
            let base = d.anchor - 0x50;
            for o in (0..0x100u64).step_by(16) {
                let row = image.bytes(base + o, 16)?;
                let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
                println!("  {:#010x}  {}", base + o, hex.join(" "));
            }
        } else {
            let w = descriptors::walk(image, &idx, tag);
            problems.extend(w.problems.clone());
            if json {
                walks.push(w);
            } else {
                print!("{}", descriptors::render(&w));
            }
        }
        if !json {
            println!();
        }
    }
    if json {
        println!("{}", serde_json::to_string(&walks)?);
    }
    if !problems.is_empty() && !json {
        eprintln!("PROBLEMS:");
        for p in &problems {
            eprintln!("  {:#010x} field[{}]: {}", p.tag, p.field, p.why);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_disasm(
    image: &Image,
    start: u64,
    to: Option<u64>,
    count: Option<usize>,
    snap: bool,
    force: bool,
    json: bool,
) -> Result<ExitCode> {
    let anchor = match (snap, force) {
        (true, _) => Anchor::Snap,
        (false, true) => Anchor::Forced,
        (false, false) => Anchor::Prologue,
    };
    // A count is served by decoding a generous range and truncating.
    let end = to.unwrap_or_else(|| start + count.unwrap_or(32) as u64 * 15 + 16);
    let mut d = disasm::disasm_range(image, start, end, anchor).map_err(|e| {
        anyhow::anyhow!(
            "{e}\n  (a start that is not `55 48 89 e5` is refused - ATLAS §4.5. \
             Use --snap to anchor on the enclosing prologue, or --force if you know it is not \
             a function start.)"
        )
    })?;
    if let Some(n) = count {
        d.insns.truncate(n);
        d.truncated_at = None;
    }
    if json {
        println!("{}", serde_json::to_string(&d)?);
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(a) = d.anchor {
        if a != start {
            println!("; anchored on prologue {a:#x}");
        }
    }
    for i in &d.insns {
        let annot = match (i.branch_target, i.rip_target) {
            (Some(t), _) => format!("   ; -> {t:#x}"),
            (_, Some(t)) => format!("   ; [{t:#x}]"),
            _ => String::new(),
        };
        println!("0x{:08x}  {:<30} {:<8} {}{}", i.address, i.bytes, i.mnemonic, i.op_str, annot);
    }
    if let Some(t) = d.truncated_at {
        eprintln!(
            "warning: disassembly stopped at {t:#x} of {:#x} - a data island, or the range is \
             not code. NOT a clean read.",
            d.end
        );
        return Ok(ExitCode::from(1));
    }
    Ok(ExitCode::SUCCESS)
}

fn parse_control(s: &str) -> Result<Control> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        bail!("control must be KIND:TARGET:SITE, e.g. jmp:0x56ffb0:0x560d27");
    }
    let kind = RefKind::parse(parts[0]).ok_or_else(|| anyhow::anyhow!("unknown kind {:?}", parts[0]))?;
    Ok(Control {
        kind,
        target: parse_int(parts[1]).map_err(anyhow::Error::msg)?,
        site: parse_int(parts[2]).map_err(anyhow::Error::msg)?,
    })
}

#[allow(clippy::too_many_arguments)]
fn cmd_xref(
    image: &Image,
    target: u64,
    kind: &str,
    controls: &[String],
    no_control: bool,
    show: bool,
    context: usize,
    json: bool,
) -> Result<ExitCode> {
    let kinds: Vec<RefKind> = if kind == "all" {
        RefKind::ALL.to_vec()
    } else {
        kind.split(',')
            .map(|k| RefKind::parse(k.trim()).ok_or_else(|| anyhow::anyhow!("unknown kind {k:?}")))
            .collect::<Result<_>>()?
    };

    let mut ctls: Vec<Control> = controls.iter().map(|c| parse_control(c)).collect::<Result<_>>()?;
    if ctls.is_empty() && !no_control {
        // Default to the pinned independent control for each kind being scanned. A run without a
        // control cannot be evidence of absence, so we never silently produce one.
        ctls = kinds.iter().filter_map(|&k| Control::pinned(k)).collect();
    }

    let report = xref::scan(image, target, &kinds, &ctls)?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        let confirmed: Vec<_> = report.confirmed().collect();
        println!(
            "target {:#x}: {} confirmed reference(s) over {} candidate window(s) [kinds: {}]",
            target,
            confirmed.len(),
            report.hits.len(),
            kinds.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(",")
        );
        for h in &report.hits {
            // ⭐ `containing.label()`, never `nearest_prologue`: `fn ?0x…` when the containing
            // function's decode never reached the site, `fn -` when nothing contains it. A wrong
            // name is worse than an honest unknown - that mis-attribution published a wrong
            // "this code is dead" verdict (see `functions::Containment`).
            let fnpart = format!("{:<13} ", h.containing.label());
            let mark = if h.confirmed { " " } else { "?" };
            println!("  {mark}{:<7} {fnpart}{:#09x}  {:<7} {}", h.kind.to_string(), h.site, h.mnemonic, h.op_str);
        }
        if let Some(start) = report.string_start_warning {
            println!(
                "\n⚠ AIM CHECK (ATLAS §4.2b): {target:#x} is INSIDE a printable string that starts \
                 at {start:#x}.\n  Nothing references a string interior - re-run against {start:#x} \
                 before reading 0 references as absence."
            );
        }
        println!();
        if report.controls.is_empty() {
            println!(
                "POSITIVE CONTROL: none supplied - 'not found' from this run is NOT evidence of absence"
            );
        }
        for c in &report.controls {
            println!(
                "POSITIVE CONTROL {:<7} {:#x} -> {:#x}: {}",
                c.control.kind.to_string(),
                c.control.target,
                c.control.site,
                if c.passed { "PASS" } else { "*** FAIL - results are NOT trustworthy ***" }
            );
        }
        if show {
            for h in report.confirmed().filter(|h| h.function.is_some()) {
                let fnv = h.function.unwrap();
                println!("\n=== fn {fnv:#09x} around {:#09x} ===", h.site);
                // ⚠ `fnv` may be frame-pointer-omitted, so `Anchor::Prologue` would REFUSE it -
                // which is how this printer would have crashed on the very sites the fix recovers.
                let d = disasm::disasm_range(
                    image,
                    fnv,
                    h.site + 64,
                    disasm::anchor_for_known_start(image, fnv),
                )?;
                let start = d.insns.len().saturating_sub(context * 2);
                for i in &d.insns[start..] {
                    let mark = if i.address == h.site { " <<<" } else { "" };
                    println!("0x{:07x}  {:<8} {}{}", i.address, i.mnemonic, i.op_str, mark);
                }
            }
        }
    }
    if !report.controls.is_empty() && !report.is_evidence {
        return Ok(ExitCode::from(2));
    }
    Ok(ExitCode::SUCCESS)
}

/// ⭐ The technique `xref` cannot do. See `historicaldocs/design/field-xref.md`.
#[allow(clippy::too_many_arguments)]
fn cmd_field_xref(
    image: &Image,
    class: Option<&str>,
    global: Option<u64>,
    offset: i64,
    depth: usize,
    all: bool,
    show_candidates: bool,
    no_control: bool,
    json: bool,
) -> Result<ExitCode> {
    if class.is_none() && global.is_none() {
        bail!("field-xref needs --class and/or --global to know which object to follow");
    }
    let index = functions::build(image)?;
    let r = rtti::harvest(image)?;
    if let Some(c) = class {
        if !r.types.iter().any(|t| t.name == c) {
            bail!(
                "no RTTI class named {c:?} - `d1-eboot rtti --name {}` lists the near misses",
                c.split("::").last().unwrap_or(c)
            );
        }
    }
    let mut report = fieldxref::scan(image, &index, &r, class, global, offset, depth)?;
    if !no_control {
        report = fieldxref::with_controls(image, &index, &r, report)?;
    }

    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "field {}{}+{:#x}: {} tracked access(es) over {} displacement match(es), \
             {} candidate function(s)",
            class.unwrap_or(""),
            global.map(|g| format!(" [{g:#x}]")).unwrap_or_default(),
            offset,
            report.tracked().count(),
            report.hits.len(),
            report.candidates.len()
        );
        if let Some(g) = global {
            println!(
                "accessors (materialise {g:#x} rip-relatively): {}",
                report
                    .accessors
                    .iter()
                    .map(|a| format!("{a:#x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        println!();
        println!("  {:<5} {:<10} {:<10} {:<6} {:<7} {}", "conf", "fn", "site", "acc", "base", "insn");
        for h in &report.hits {
            if !all && !(h.tracked || h.absolute) {
                continue;
            }
            let conf = if h.absolute {
                "ABS"
            } else if h.tracked {
                "[C]"
            } else {
                "[U]"
            };
            println!(
                "  {:<5} {:#010x} {:#010x} {:<6} {:<7} {}{}",
                conf,
                h.func,
                h.site,
                h.access.as_str(),
                h.base,
                h.text,
                if h.indexed { "   ⚠ INDEXED - array step, not a plain field" } else { "" }
            );
        }
        if !all {
            let untracked = report.hits.iter().filter(|h| !h.tracked && !h.absolute).count();
            if untracked > 0 {
                println!(
                    "\n  ({untracked} further displacement match(es) hidden: the base register is \
                     NOT proven to hold this object - `--all` to see them, and read them as [U])"
                );
            }
        }
        if show_candidates {
            println!("\ncandidates:");
            for c in &report.candidates {
                println!(
                    "  {:#010x}  {:<24} this={} {}{}",
                    c.func,
                    c.origins.iter().map(|o| o.as_str()).collect::<Vec<_>>().join(","),
                    if c.entry_this { "rdi" } else { "-  " },
                    c.class.clone().unwrap_or_default(),
                    c.vslot.map(|s| format!("::vfunc[{s}]")).unwrap_or_default()
                );
            }
        }
        if !report.escapes.is_empty() {
            println!("\n⭐ the FIELD ADDRESS escapes into another function here - for a field an");
            println!("   allocator fills in, this is the only place a \"writer\" appears at all:");
            for e in &report.escapes {
                println!(
                    "  {:#010x} {:#010x}  arg{} ({}) -> {:#x}   {}",
                    e.func, e.site, e.arg_index, e.reg, e.callee, e.text
                );
            }
        }
        let real: Vec<_> = report.truncated.iter().filter(|t| !t.after_terminal).collect();
        let pad = report.truncated.len() - real.len();
        if !real.is_empty() {
            println!(
                "\n⚠ INCOMPLETE: the decoder stopped on an undecodable byte inside {} candidate(s) \
                 BEFORE any terminal instruction ({}). Accesses after that point are NOT in this \
                 report.",
                real.len(),
                real.iter().map(|t| format!("{:#x}@{:#x}", t.func, t.site)).collect::<Vec<_>>().join(" ")
            );
        }
        if pad > 0 {
            println!(
                "\n({pad} further candidate(s) stopped decoding AFTER a `ret` - that is inter-function \
                 padding, not an incomplete scan.)"
            );
        }
        println!(
            "\n⚠ HONEST LIMIT: the base-register tracker is linear and branch-oblivious. `[C]` rows \
             have a base register proven to hold the object on the fall-through path; `[U]` rows \
             matched the DISPLACEMENT ONLY and the base may be an unrelated object."
        );
        println!();
        if report.controls.is_empty() {
            println!(
                "POSITIVE CONTROL: none run (--no-control) - 'not found' here is NOT evidence of absence"
            );
        }
        for c in &report.controls {
            println!(
                "{} CONTROL {} +{:#x}: {}  [{}]",
                if matches!(c.control.expect, fieldxref::Expect::Empty { .. }) {
                    "INVERTED"
                } else {
                    "POSITIVE"
                },
                c.control.class,
                c.control.offset,
                if c.passed { "PASS" } else { "*** FAIL - results are NOT trustworthy ***" },
                c.detail
            );
            println!("           {}", c.control.what);
        }
    }
    if !report.controls.is_empty() && !report.is_evidence {
        return Ok(ExitCode::from(2));
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_reloc(image: &Image, targets: &[u64], slot: Option<u64>, span: u64, json: bool) -> Result<ExitCode> {
    let t = reloc::tables(image)?;
    if !json {
        eprintln!(
            "DT_SCE_RELA at file {:#x} size {:#x} (derived from PT_DYNAMIC, not hardcoded)",
            t.rela_file_offset, t.rela_size
        );
    }
    if let Some(base) = slot {
        let m = reloc::by_slot(image)?;
        let rows: Vec<_> = (base..base + span)
            .step_by(8)
            .map(|v| (v, m.get(&v).copied().unwrap_or(0)))
            .collect();
        if json {
            println!("{}", serde_json::to_string(&rows)?);
        } else {
            for (v, a) in rows {
                println!("  slot {v:#010x} -> {a:#x}");
            }
        }
        return Ok(ExitCode::SUCCESS);
    }
    let m = reloc::by_addend(image)?;
    if json {
        let rows: Vec<_> = targets
            .iter()
            .map(|t| (format!("{t:#x}"), m.get(t).cloned().unwrap_or_default()))
            .collect();
        println!("{}", serde_json::to_string(&rows)?);
    } else {
        for t in targets {
            let hits = m.get(t).cloned().unwrap_or_default();
            println!(
                "{t:#010x}: {} slot(s) {}",
                hits.len(),
                hits.iter().map(|h| format!("{h:#x}")).collect::<Vec<_>>().join(" ")
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_controls(image: &Image, json: bool) -> Result<ExitCode> {
    let mut rows: Vec<(String, bool, String)> = Vec::new();

    let sha = image.sha256();
    let known = d1_build::builds()
        .ok()
        .and_then(|b| b.all.iter().find(|x| x.eboot_sha256 == sha).map(|x| x.id.clone()));
    rows.push((
        "build identity (sha256) is a registered build".into(),
        known.is_some(),
        format!("{sha} = {}", known.as_deref().unwrap_or("UNREGISTERED")),
    ));
    // Every row below is a historical constant of the 01.29 reference. On any other build they
    // are expected to fail until re-derived; this row says so up front instead of 20 red rows.
    rows.push((
        "fixed addresses apply to this image (reference, or re-derived)".into(),
        addresses_apply(&sha),
        if sha == PINNED_EBOOT_SHA256 { "01.29 reference".into() } else { "not re-derived".into() },
    ));

    let t = reloc::tables(image)?;
    rows.push((
        "DT_SCE_RELA derived == historical constant".into(),
        t.rela_file_offset == 0x1A0D100 + 0xBDD0 && t.rela_size == 0x1B0918,
        format!("file {:#x} size {:#x}", t.rela_file_offset, t.rela_size),
    ));
    rows.push((
        "R_X86_64_RELATIVE count".into(),
        reloc::relative(image)?.len() == 70_682,
        format!("{}", reloc::relative(image)?.len()),
    ));

    let idx = descriptors::index(image)?;
    rows.push(("anchored descriptor count == 4555".into(), idx.len() == 4555, format!("{}", idx.len())));

    for kind in RefKind::ALL {
        if let Some(c) = Control::pinned(kind) {
            let ok = xref::scan_kind(image, c.target, c.kind)?
                .into_iter()
                .any(|h| h.confirmed && h.site == c.site);
            rows.push((
                format!("xref {kind} control {:#x} -> {:#x}", c.target, c.site),
                ok,
                String::new(),
            ));
        }
    }

    // ── the `fn` column: FPO attribution, with its inverted half ────────────────────────────
    // ⛔⛔ THE MIS-ATTRIBUTION THAT PUBLISHED A WRONG VERDICT (fixed 2026-08-11). `xref`'s `fn`
    // column was `nearest_prologue(site)`, so a site inside one of the 11,446 frame-pointer-omitted
    // functions was credited to the preceding prologue-anchored one. `0x1D2030` READS the pool that
    // `0x1D1E80` WRITES; filed under the writer, the pool looked write-only and a session published
    // "this dispatch is unreachable dead code". It was live.
    {
        let mut fpo_ok = true;
        for (target, site) in [(0x25C_0C58u64, 0x1D2_0A1u64), (0x25C_0C70, 0x1D2_087)] {
            let h = xref::scan_kind(image, target, RefKind::RipRel)?
                .into_iter()
                .find(|h| h.confirmed && h.site == site);
            fpo_ok &= h.is_some_and(|h| {
                h.function == Some(0x1D2_030)
                    && h.containing.how == functions::Containment::Fpo
            });
        }
        rows.push((
            "xref fn column: 0x1d2087/0x1d20a1 attribute to the FPO reader 0x1d2030, NOT 0x1d1e80"
                .into(),
            fpo_ok,
            String::new(),
        ));
        // INVERTED: the mechanism that was wrong is still wrong here, so the row above is a real
        // discrimination and not decoration (CLAUDE.md: *invert every control*).
        rows.push((
            "INVERTED: nearest_prologue(0x1d20a1) still answers 0x1d1e80 - the control discriminates"
                .into(),
            image.nearest_prologue(0x1D2_0A1, 0x4000) == Some(0x1D1_E80)
                && image.nearest_prologue(0x1D2_087, 0x4000) == Some(0x1D1_E80),
            String::new(),
        ));
        // Positive control on the ordinary case, same instrument: 0x162E2B5's two references are
        // prologue-anchored in 0x56AF60, and 0x5719F2 is in 0x571940.
        let plain = xref::scan_kind(image, 0x162_E2B5, RefKind::RipRel)?;
        let plain_ok = plain.iter().filter(|h| h.confirmed).count() == 2
            && plain.iter().filter(|h| h.confirmed).all(|h| {
                h.function == Some(0x56A_F60)
                    && h.containing.how == functions::Containment::Prologue
            });
        rows.push((
            "xref fn column: prologue-anchored attribution unchanged (0x162e2b5 -> 0x56af60 x2)"
                .into(),
            plain_ok,
            String::new(),
        ));
        // ⭐ A wrong name is worse than an honest unknown - and the end-blind shortcut still gives
        // one, which is what makes "None" a measurement.
        let data = image.containing_function(0x25C_0C58);
        let starts = image.functions().starts();
        rows.push((
            "honest unknown: 0x25c0c58 (data) is in NO function -> `fn -`, though function_of still \
             names 0x129f850"
                .into(),
            data.how == functions::Containment::None
                && data.label() == "fn -"
                && functions::Index::function_of(&starts, 0x25C_0C58) == Some(0x129_F850),
            data.label(),
        ));
    }

    // ⭐ ATLAS §4.2's two-session bug, as a live assertion: 0x56FFB0 is reached ONLY by tail call.
    let calls = xref::scan_kind(image, 0x56FFB0, RefKind::Call)?;
    rows.push((
        "0x56ffb0 has ZERO `call rel32` sites (call-only scanners report it unreachable)".into(),
        calls.iter().filter(|h| h.confirmed).count() == 0,
        String::new(),
    ));

    // ── field-xref: the technique `xref` is blind to ────────────────────────────────────────
    // ⭐⭐ THE AXIS: *find register-relative field access that an absolute-address xref cannot see,
    // and DON'T just report every `[reg+N]` walked past.* Three positives that only a working
    // tracker satisfies, plus an INVERTED case at an offset the object does not have that only a
    // *discriminating* tracker satisfies. Each verified to FAIL by breaking its mechanism on
    // purpose before shipping - see `historicaldocs/design/field-xref.md` §5.
    {
        let fx_index = functions::build(image)?;
        let fx_rtti = rtti::harvest(image)?;
        for c in fieldxref::controls(image, &fx_index, &fx_rtti)? {
            let kind = if matches!(c.control.expect, fieldxref::Expect::Empty { .. }) {
                "INVERTED"
            } else {
                "field-xref"
            };
            rows.push((format!("{kind}: {}", c.control.what), c.passed, c.detail));
        }
        // The premise itself, asserted: the absolute address of the SAME field does not lead to
        // the access field-xref finds. If this stops holding, field-xref needs a new reason to
        // exist - a finding, not a regression to silence.
        let abs = xref::scan_kind(image, 0x267_AF10, RefKind::RipRel)?;
        let sites: Vec<u64> = abs.iter().filter(|h| h.confirmed).map(|h| h.site).collect();
        rows.push((
            "field-xref premise: absolute xref of 0x267af10 is BLIND to 0x340240 (but not dead)"
                .into(),
            !sites.contains(&0x340_240) && !sites.is_empty(),
            format!("{} absolute site(s)", sites.len()),
        ));
    }

    // BSS must be refused, not served a plausible byte (ATLAS: `0x263D0F8` is BSS).
    rows.push((
        "vaddr 0x263d0f8 is refused as BSS".into(),
        matches!(image.file_offset(0x263D0F8), Err(d1_eboot::ImageError::Bss { .. })),
        String::new(),
    ));

    // ── the map/atlas controls ──────────────────────────────────────────────────────────────
    // ⭐ THE DENOMINATOR. Every coverage claim in design/eboot-map.md is a fraction of this
    // number, so it is pinned: a silent change here would move every published percentage.
    let prologues = functions::prologues(image);
    rows.push((
        "prologue-anchored function count == 35834 (the coverage denominator)".into(),
        prologues.len() == 35_834,
        format!("{}", prologues.len()),
    ));

    // The NID oracle. `dynsym.rs` derives the three __cxxabiv1 class_type_info vtable symbols from
    // their C++ spellings rather than hardcoding `pZ9WXcClPO8` - this asserts the derivation still
    // lands, which is what stops a wrong constant outliving the reading that made it (ATLAS §4.3).
    let nids = dynsym::by_nid(image)?;
    for (spelling, want) in [
        (rtti::TI_BASE, 458u32),
        (rtti::TI_SI, 459),
        (rtti::TI_VMI, 501),
        (rtti::PURE_VIRTUAL, 511),
    ] {
        let got = dynsym::index_of(&nids, spelling);
        rows.push((
            format!("NID oracle resolves {} -> sym {want}", spelling.rsplit("11").next().unwrap_or(spelling)),
            got == Some(want),
            format!("{} = sym {:?}", dynsym::nid(spelling), got),
        ));
    }

    let r = rtti::harvest(image)?;
    rows.push((
        "RTTI typeinfo count == 2633, ALL named".into(),
        r.types.len() == 2633 && r.types.iter().all(|t| !t.mangled.is_empty()),
        format!("{} types, {} named", r.types.len(), r.types.iter().filter(|t| !t.mangled.is_empty()).count()),
    ));
    // Independent control on the RTTI reading itself: eboot-map §7 and director-selectability.md
    // each name a class by hand, from separate sessions. Both must come back out of the harvest.
    let has = |n: &str| r.types.iter().any(|t| t.name == n);
    rows.push((
        "RTTI reproduces two hand-read class names (s_investment_utility_context, cui::c_director_activity)".into(),
        has("s_investment_utility_context") && has("cui::c_director_activity"),
        String::new(),
    ));
    // §6.4: "67 c_investment_bap_message_* classes", counted by an earlier session by hand.
    let bap = r
        .types
        .iter()
        .filter(|t| t.name.starts_with("c_investment_bap_message_"))
        .count();
    rows.push((
        "RTTI finds >= 67 c_investment_bap_message_* classes (THE-CHAIN §6.4)".into(),
        bap >= 67,
        format!("{bap}"),
    ));
    // ⭐ An INDEPENDENT control on the vtable walk: `historicaldocs/design/THE-CHAIN.md` §3.2 hand-traced the
    // availability chain through `s_investment_utility_context`'s vtable by **byte offset** -
    // "`vtable[0x80]` = `0x260A10`" and "`vtable+0xC8` = **`0x260BD0`**" - from a different
    // session, by a different method, without ever enumerating the vtable. Slot 0x80/8 = 16 and
    // 0xC8/8 = 25 must therefore hold exactly those functions. Nothing about this control comes
    // from this harvest, which is what makes it a control rather than a restatement.
    let vt = r.vtables.iter().find(|v| v.vptr == 0x18F5AF0);
    let slot_at = |byte: usize| vt.and_then(|v| v.slots.get(byte / 8).copied()).unwrap_or(0);
    rows.push((
        "vtable walk: 0x18f5af0 [+0x80] == 0x260a10 and [+0xc8] == 0x260bd0 (THE-CHAIN §3.2)".into(),
        slot_at(0x80) == 0x260A10 && slot_at(0xC8) == 0x260BD0,
        format!("[+0x80]={:#x} [+0xc8]={:#x}", slot_at(0x80), slot_at(0xC8)),
    ));

    // FNV-1 (NOT FNV-1a) - THE-CHAIN §4, the finding this project lost once.
    rows.push((
        "FNV-1 name hash: fnv1(\"platform_signin\") == 0xe9d46191".into(),
        strings::fnv1(b"platform_signin") == 0xE9D4_6191
            && strings::fnv1(b"content_check") == 0x783B_F8AD,
        String::new(),
    ));

    // The name-table harvester reproduces the table THE-CHAIN §4.1 decoded by hand (ATLAS §4.4d).
    let mut corpus = strings::Corpus::build(image, 3)?;
    corpus.add_names(r.types.iter().map(|t| t.name.clone()));
    let tabs = nametab::harvest(image, &corpus, nametab::REGISTRAR)?;
    let ctl = tabs.iter().find(|t| t.site == nametab::CONTROL_SITE);
    rows.push((
        "nametab reproduces the hand-decoded bootflow table at 0x274da80 (8 entries)".into(),
        ctl.is_some_and(|t| {
            t.table == Some(nametab::CONTROL_TABLE)
                && t.entries.len() == 8
                && t.entries[2].hash == 0xE9D4_6191
        }),
        format!("{} registrar sites, {} tables whole", tabs.len(), tabs.iter().filter(|t| t.complete).count()),
    ));

    // ⭐ THE FAMILY 4/4 CHECKSUM - THE-CHAIN §8.5t, adjudicated 2026-08-08 by measurement.
    // The wire `instance_type` is the checksum on the record whose SELF TAG is the schema handle
    // the live client resolves (0x8080102E). That record is a ONE-FIELD WRAPPER around the
    // three-field payload schema 0x8080102D - the same wrapping the account and profile use
    // (eboot-map §4.1). 0xAF776963 is the INNER record's checksum and never appears on the wire.
    let d4 = descriptors::index(image)?;
    let wrapper = d4.get(0x8080_102E);
    let inner = d4.get(0x8080_102D);
    let ck = |anchor: u64| image.u32(anchor - 0x50 + 8).unwrap_or(0);
    rows.push((
        "family 4/4 instance checksum: wrapper 0x8080102e -> 0xc4060975 (inner 0x8080102d -> 0xaf776963)".into(),
        wrapper.is_some_and(|w| w.field_count == 1 && w.struct_size == 0xF90 && ck(w.anchor) == 0xC406_0975)
            && inner.is_some_and(|i| i.field_count == 3 && i.struct_size == 0xF90 && ck(i.anchor) == 0xAF77_6963),
        format!(
            "wrapper {:#x}/{:#010x}  inner {:#x}/{:#010x}",
            wrapper.map(|w| w.anchor).unwrap_or(0),
            wrapper.map(|w| ck(w.anchor)).unwrap_or(0),
            inner.map(|i| i.anchor).unwrap_or(0),
            inner.map(|i| ck(i.anchor)).unwrap_or(0)
        ),
    ));

    // ── the message-catalogue controls (catalogue.rs, ported from message-catalogue.py) ─────
    // The investment layers need the shipped-content definitions file; when it is absent only
    // the eboot-resident rows run, and that absence is REPORTED rather than silently passed.
    {
        let defs = std::fs::read(DEFAULT_DEFS)
            .ok()
            .and_then(|d| catalogue::investment_defs(&d).ok());
        let defs_present = defs.is_some();
        let reader = catalogue::Reader::new(image)?;
        let data = catalogue::Data::build(image, defs, None)?;
        let (_, _, cat_rows) = catalogue::render_controls(image, &reader, &data);
        for c in cat_rows {
            rows.push((c.name, c.passed, String::new()));
        }
        if !defs_present {
            rows.push((
                "catalogue investment defs file present (layer 4 rows above were SKIPPED)".into(),
                false,
                DEFAULT_DEFS.rsplit("capture-rig/").next().unwrap_or("").to_string(),
            ));
        }
    }

    // ── the ladder controls (ladder.rs, ported from state-machine.py) ───────────────────────
    // Annotation TSVs absent or unreadable is REPORTED as a failure, not silently passed - the
    // catalogue's defs-file precedent. "Cannot tell" must be the worst exit code.
    {
        let atlas_p = std::path::Path::new(ladder::DEFAULT_ATLAS_TSV);
        let diag_p = std::path::Path::new(ladder::DEFAULT_DIAG_TSV);
        match ladder::load_annotations(atlas_p, diag_p) {
            Ok(ann) => match ladder::controls(image, &ann) {
                Ok((_, _, lrows)) => {
                    for r in lrows {
                        rows.push((format!("ladder: {}", r.label), r.passed, r.detail));
                    }
                }
                Err(e) => rows.push(("ladder: controls ran to completion".into(), false, format!("{e}"))),
            },
            Err(e) => {
                rows.push(("ladder: annotation TSVs readable".into(), false, format!("{e}")))
            }
        }
    }

    let pass = rows.iter().all(|r| r.1);
    if json {
        let v: Vec<_> = rows
            .iter()
            .map(|(n, ok, d)| serde_json::json!({"control": n, "passed": ok, "detail": d}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({"passed": pass, "controls": v}))?);
    } else {
        for (n, ok, d) in &rows {
            println!("{}  {n}{}", if *ok { "PASS" } else { "FAIL" }, if d.is_empty() { String::new() } else { format!("  [{d}]") });
        }
        println!("\n{}", if pass { "ALL CONTROLS PASS" } else { "*** CONTROL FAILURE - this image is not the pinned build, or a reader regressed ***" });
    }
    Ok(if pass { ExitCode::SUCCESS } else { ExitCode::from(2) })
}
