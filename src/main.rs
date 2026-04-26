use std::collections::{HashMap, HashSet};
use std::io::{self, BufWriter, Write};
use std::process::exit;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

trait AnsiColor {
    fn magenta(self)   -> String; fn yellow(self)    -> String;
    fn grey(self)      -> String; fn blue(self)      -> String;
    fn cyan(self)      -> String; fn black(self)     -> String;
    fn b_black(self)   -> String; fn b_red(self)     -> String;
    fn b_yellow(self)  -> String; fn b_magenta(self) -> String;
    fn b_cyan(self)    -> String; fn bold(self)      -> String;
    fn underlined(self) -> String;
}

macro_rules! impl_ansi {
    ($t:ty) => {
        impl AnsiColor for $t {
            fn magenta(self)    -> String { format!("\x1b[35m{}\x1b[0;39;49m", self) }
            fn yellow(self)     -> String { format!("\x1b[33m{}\x1b[0;39;49m", self) }
            fn grey(self)       -> String { format!("\x1b[37m{}\x1b[0;39;49m", self) }
            fn blue(self)       -> String { format!("\x1b[34m{}\x1b[0;39;49m", self) }
            fn cyan(self)       -> String { format!("\x1b[36m{}\x1b[0;39;49m", self) }
            fn black(self)      -> String { format!("\x1b[30m{}\x1b[0;39;49m", self) }
            fn b_black(self)    -> String { format!("\x1b[90m{}\x1b[0;39;49m", self) }
            fn b_red(self)      -> String { format!("\x1b[91m{}\x1b[0;39;49m", self) }
            fn b_yellow(self)   -> String { format!("\x1b[93m{}\x1b[0;39;49m", self) }
            fn b_magenta(self)  -> String { format!("\x1b[95m{}\x1b[0;39;49m", self) }
            fn b_cyan(self)     -> String { format!("\x1b[96m{}\x1b[0;39;49m", self) }
            fn bold(self)       -> String { format!("\x1b[1m{}\x1b[0;39;49m", self) }
            fn underlined(self) -> String { format!("\x1b[4m{}\x1b[0;39;49m", self) }
        }
    };
}

impl_ansi!(&str);
impl_ansi!(String);

#[derive(serde::Deserialize)]
struct IndexEntry {
    vers: String,
    features: HashMap<String, Vec<String>>,
    // features2 holds dep:-syntax features (Cargo 1.60+); merged with features for display
    #[serde(default)]
    features2: HashMap<String, Vec<String>>,
    yanked: bool,
}

const FILTER_ALL: &str = "all";
const FILTER_ND: &str = "nd";

// Returns the path suffix for a crate in the sparse index (e.g. "re/qw/reqwest")
fn index_path(name: &str) -> String {
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{name}", &name[..1]),
        _ => format!("{}/{}/{name}", &name[..2], &name[2..4]),
    }
}

fn cargo_home() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("CARGO_HOME") {
        return Some(p.into());
    }
    #[cfg(windows)]
    let home = std::env::var("USERPROFILE").ok()?;
    #[cfg(not(windows))]
    let home = std::env::var("HOME").ok()?;
    Some(std::path::PathBuf::from(home).join(".cargo"))
}

fn index_cache_dir() -> Option<std::path::PathBuf> {
    let index_base = cargo_home()?.join("registry").join("index");
    let index_dir = std::fs::read_dir(&index_base)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("index.crates.io-")
        })?
        .path();
    Some(index_dir.join("cache"))
}

// Reads the crate's entry from the local Cargo registry cache (~/.cargo/registry/index/...)
// Cache files may have a binary header — we handle both raw NDJSON and headered formats in parse_entries.
fn read_local_cache(path: &str) -> Option<Vec<u8>> {
    std::fs::read(index_cache_dir()?.join(path)).ok()
}

// Writes raw NDJSON bytes to the cargo cache so future lookups skip the network entirely.
fn write_local_cache(path: &str, bytes: &[u8]) {
    let Some(cache_dir) = index_cache_dir() else {
        return;
    };
    let file_path = cache_dir.join(path);
    if let Some(parent) = file_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(file_path, bytes);
}

fn fetch_bytes(path: &str) -> Option<Vec<u8>> {
    ureq::get(format!("https://index.crates.io/{path}"))
        .header(
            "User-Agent",
            concat!("cargo-feat/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .ok()
        .and_then(|mut r| r.body_mut().read_to_vec().ok())
}

fn json_lines(bytes: &[u8]) -> Vec<&[u8]> {
    bytes
        .split(|&b| b == b'\n')
        .filter(|line| line.first() == Some(&b'{'))
        .collect()
}

// For latest: scans from the end and stops at the first stable non-yanked entry (O(1) parses in practice).
// For an explicit version: scans forward and stops at the first match.
fn find_entry(lines: &[&[u8]], explicit_version: Option<&str>) -> Option<IndexEntry> {
    if let Some(ver) = explicit_version {
        lines.iter().find_map(|line| {
            let entry: IndexEntry = simd_json::from_slice(&mut line.to_vec()).ok()?;
            (entry.vers == ver).then_some(entry)
        })
    } else {
        lines.iter().rev().find_map(|line| {
            let entry: IndexEntry = simd_json::from_slice(&mut line.to_vec()).ok()?;
            (!entry.yanked && !entry.vers.contains('-')).then_some(entry)
        })
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|s| s.as_str()) == Some("feat") {
        args.remove(0);
    }

    if args.is_empty() {
        println!("{}", "—— Thanks for using cargo-feat ˎˊ˗".magenta());
        println!(
            "\t{}",
            "Usage for this program is really simple, Instead of looking for the\n\tfeatures of a specific crate manually you can just use this tool\n\tand it will list you all the features (or the non-default only based on your choice)\n\tof that specific crate! It is really easy and simple!\n\n\tArgument(s) marked with a \"*\" is/are required for the command to work."
                .grey()
        );
        println!(
            "\n\t{}\n\t{} {} {} {} {} {} {} {} {} {}\n\n\t{}\n\t{} {} {} {}",
            "— Base command usage —".magenta(),
            "|".magenta(),
            "$".yellow(),
            "feat".b_black(),
            "*<crate name>".grey().bold(),
            "<version>".grey().bold(),
            "<all|nd (not default)>".grey().bold(),
            "[--internals]".grey().bold(),
            "[--include-internals|-ii]".grey().bold(),
            "[--deps]".grey().bold(),
            "[--json]".grey().bold(),
            "— Example Usage —".magenta(),
            "|".magenta(),
            "$".yellow(),
            "feat".b_black(),
            "reqwest".grey().bold(),
        );
        return;
    }

    let crate_name = args.first().unwrap().trim().replace("_", "-");

    let mut feat_filter = FILTER_ALL.to_string();
    let mut explicit_version: Option<String> = None;
    let mut show_internals = false;
    let mut include_internals = false;
    let mut output_json = false;
    let mut show_deps = false;
    for arg in args.iter().skip(1) {
        let s = arg.trim();
        if s == "--internals" {
            show_internals = true;
        } else if s == "--include-internals" || s == "-ii" {
            include_internals = true;
        } else if s == "--json" {
            output_json = true;
        } else if s == "--deps" {
            show_deps = true;
        } else if s == FILTER_ALL || s == FILTER_ND {
            feat_filter = s.to_string();
        } else {
            explicit_version = Some(s.to_string());
        }
    }

    let path = index_path(&crate_name);

    // Fast path: local Cargo registry cache (populated by cargo build/add/update, or prior cargo-feat run)
    // Slow path: fetch from network, then persist to cache so the next run is fast
    let bytes = read_local_cache(&path)
        .or_else(|| {
            let data = fetch_bytes(&path)?;
            write_local_cache(&path, &data);
            Some(data)
        })
        .unwrap_or_else(|| {
            eprintln!(
                "{}{} {}{} {}",
                "<".b_black(),
                "Uh".yellow(),
                "oh".b_red(),
                ">".b_black(),
                format!("Crate \"{crate_name}\" not found on crates.io").yellow()
            );
            exit(103);
        });

    let lines = json_lines(&bytes);

    if lines.is_empty() {
        eprintln!(
            "{}{} {}{} {}",
            "<".b_black(),
            "Uh".yellow(),
            "oh".b_red(),
            ">".b_black(),
            format!("Crate \"{crate_name}\" not found on crates.io").yellow()
        );
        exit(103);
    }

    let entry = find_entry(&lines, explicit_version.as_deref()).unwrap_or_else(|| {
        if explicit_version.is_some() {
            let ver = explicit_version.as_deref().unwrap_or("");
            eprintln!(
                "{}{} {}{} {}\n- Version \"{ver}\" not found for crate \"{crate_name}\"",
                "<".b_black(),
                "Uh".yellow(),
                "oh".b_red(),
                ">".b_black(),
                "The specified version does not exist on crates.io".yellow(),
            );
            exit(104);
        } else {
            eprintln!(
                "{}{} {}{} {}",
                "<".b_black(),
                "Uh".yellow(),
                "oh".b_red(),
                ">".b_black(),
                format!("No stable version found for crate \"{crate_name}\"").yellow()
            );
            exit(105);
        }
    });

    let mut all_features = entry.features;
    all_features.extend(entry.features2);

    let mut w = BufWriter::new(io::stdout());

    if output_json {
        let mut sorted: Vec<(&String, &Vec<String>)> = all_features.iter().collect();
        sorted.sort_by_key(|(k, _)| k.as_str());
        let mut out = String::from("{");
        for (i, (k, v)) in sorted.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push('"');
            out.push_str(k);
            out.push_str("\": [");
            for (j, s) in v.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                out.push('"');
                out.push_str(s);
                out.push('"');
            }
            out.push(']');
        }
        out.push('}');
        let _ = writeln!(w, "{}", out);
        return;
    }

    let mut features: Vec<(String, Vec<String>)> = all_features.into_iter().collect();

    if features.is_empty() {
        let _ = writeln!(
            w,
            "{} {} {} {} {}",
            "—".bold().yellow(),
            crate_name.b_magenta().bold(),
            "crate does exist,".b_yellow(),
            "but has no features".b_yellow(),
            "—".bold().yellow()
        );
        return;
    }

    let _ = writeln!(
        w,
        "{} {}{} {} {}",
        "—".bold().yellow(),
        crate_name.b_magenta().bold(),
        "'s".b_yellow(),
        "features are in the following list".b_yellow(),
        "—".bold().yellow()
    );

    features.sort_unstable_by(|(a, _), (b, _)| {
        if a == "default" {
            return std::cmp::Ordering::Less;
        }
        if b == "default" {
            return std::cmp::Ordering::Greater;
        }
        a.cmp(b)
    });

    let default_features_set: HashSet<&str> = features
        .first()
        .filter(|(k, _)| k == "default")
        .map(|(_, v)| v.iter().map(String::as_str).collect())
        .unwrap_or_default();

    for (key, val) in features.iter() {
        let is_internal = key.starts_with("__");
        if is_internal && !include_internals {
            continue;
        }

        if key == "default" {
            if feat_filter == FILTER_ND {
                continue;
            }
            if val.is_empty() {
                let _ = writeln!(
                    w,
                    "\t{} {} \n\t     {}",
                    "★".b_yellow(),
                    key.clone().b_magenta().bold().underlined(),
                    "none".blue()
                );
            } else {
                let _ = writeln!(
                    w,
                    "\t{} {} \n\t     {}",
                    "★".b_yellow(),
                    key.clone().b_magenta().bold().underlined(),
                    val.join("\n\t     ").blue()
                );
            }
            continue;
        }

        if is_internal {
            let _ = writeln!(w, "\t{} {}", "—".grey(), key.clone().grey().bold());
            if show_deps && !val.is_empty() {
                let _ = writeln!(w, "\t     {}", val.join("\n\t     ").grey());
            }
            continue;
        }

        let internals_annotation = if show_internals {
            let internal_deps: Vec<&str> = val
                .iter()
                .filter(|s| s.starts_with("__"))
                .map(String::as_str)
                .collect();
            if !internal_deps.is_empty() {
                let inner = internal_deps.join(", ");
                format!(
                    " {}{}{}",
                    "[[".black().bold(),
                    inner.black().bold(),
                    "]]".black().bold()
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let _ = writeln!(
            w,
            "\t{} {}{}{}",
            "—".b_magenta(),
            key.clone().b_cyan().bold(),
            internals_annotation,
            if default_features_set.contains(key.as_str()) {
                format!(
                    " {}{}{}",
                    "(".b_yellow(),
                    "default".bold().b_magenta(),
                    ")".b_yellow()
                )
            } else {
                "".into()
            }
        );

        if show_deps && !val.is_empty() {
            let _ = writeln!(w, "\t     {}", val.join("\n\t     ").cyan());
        }
    }
}
