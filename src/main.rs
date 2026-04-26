use std::process::exit;

use colorize::AnsiColor;
use mimalloc::MiMalloc;

use ahash::RandomState;
use hashbrown::{HashMap as HHashMap, HashSet};

type BrownMap<K, V> = HHashMap<K, V, RandomState>;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(serde::Deserialize)]
struct IndexEntry {
    vers: String,
    features: BrownMap<String, Vec<String>>,
    // features2 holds dep:-syntax features (Cargo 1.60+); merged with features for display
    #[serde(default)]
    features2: BrownMap<String, Vec<String>>,
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
        .find(|e| e.file_name().to_string_lossy().starts_with("index.crates.io-"))?
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
    let Some(cache_dir) = index_cache_dir() else { return };
    let file_path = cache_dir.join(path);
    if let Some(parent) = file_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(file_path, bytes);
}

fn fetch_bytes(path: &str) -> Option<Vec<u8>> {
    ureq::get(format!("https://index.crates.io/{path}"))
        .header("User-Agent", concat!("cargo-feat/", env!("CARGO_PKG_VERSION")))
        .call()
        .ok()
        .and_then(|mut r| r.body_mut().read_to_vec().ok())
}

// Only parse lines that start with '{' — skips binary cache headers and blank lines
fn parse_entries(bytes: &[u8]) -> Vec<IndexEntry> {
    bytes
        .split(|&b| b == b'\n')
        .filter(|line| line.first() == Some(&b'{'))
        .filter_map(|line| simd_json::from_slice(&mut line.to_vec()).ok())
        .collect()
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
            "\n\t{}\n\t{} {} {} {} {} {}\n\n\t{}\n\t{} {} {} {}",
            "— Base command usage —".magenta(),
            "|".magenta(),
            "$".yellow(),
            "feat".b_black(),
            "*<crate name>".grey().bold(),
            "<version>".grey().bold(),
            "<all|nd (not default)>".grey().bold(),
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
    for arg in args.iter().skip(1) {
        let s = arg.trim();
        if s == FILTER_ALL || s == FILTER_ND {
            feat_filter = s.to_string();
        } else {
            explicit_version = Some(s.to_string());
        }
    }

    let path = index_path(&crate_name);

    // Fast path: local Cargo registry cache (populated by cargo build/add/update, or prior cargo-feat run)
    // Slow path: fetch from network, then persist to cache so the next run is fast
    let bytes = read_local_cache(&path).or_else(|| {
        let data = fetch_bytes(&path)?;
        write_local_cache(&path, &data);
        Some(data)
    }).unwrap_or_else(|| {
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

    let entries = parse_entries(&bytes);

    if entries.is_empty() {
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

    // Index lines are in chronological order — last non-yanked stable entry = newest stable version
    let crate_version = explicit_version.unwrap_or_else(|| {
        entries
            .iter()
            .rev()
            .find(|e| !e.yanked && !e.vers.contains('-'))
            .map(|e| e.vers.clone())
            .unwrap_or_else(|| {
                eprintln!(
                    "{}{} {}{} {}",
                    "<".b_black(),
                    "Uh".yellow(),
                    "oh".b_red(),
                    ">".b_black(),
                    format!("No stable version found for crate \"{crate_name}\"").yellow()
                );
                exit(105);
            })
    });

    let entry = entries
        .into_iter()
        .find(|e| e.vers == crate_version)
        .unwrap_or_else(|| {
            eprintln!(
                "{}{} {}{} {}\n- Version \"{crate_version}\" not found for crate \"{crate_name}\"",
                "<".b_black(),
                "Uh".yellow(),
                "oh".b_red(),
                ">".b_black(),
                "The specified version does not exist on crates.io".yellow(),
            );
            exit(104);
        });

    let mut all_features = entry.features;
    all_features.extend(entry.features2);
    let mut features: Vec<(String, Vec<String>)> = all_features.into_iter().collect();

    if features.is_empty() {
        println!(
            "{} {} {} {} {}",
            "—".bold().yellow(),
            crate_name.b_magenta().bold(),
            "crate does exist,".b_yellow(),
            "but has no features".b_yellow(),
            "—".bold().yellow()
        );
        return;
    }

    println!(
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

    for (key, val) in features.iter().filter(|(k, _)| !k.starts_with("__")) {
        if key == "default" {
            if feat_filter == FILTER_ND {
                continue;
            }
            if val.is_empty() {
                println!(
                    "\t{} {} \n\t     {}",
                    "★".b_yellow(),
                    key.clone().b_magenta().bold().underlined(),
                    "none".blue()
                );
            } else {
                println!(
                    "\t{} {} \n\t     {}",
                    "★".b_yellow(),
                    key.clone().b_magenta().bold().underlined(),
                    val.join("\n\t     ").blue()
                );
            }
            continue;
        }

        println!(
            "\t{} {} {}",
            "—".b_magenta(),
            key.clone().b_cyan().bold(),
            if default_features_set.contains(key.as_str()) {
                format!(
                    "{}{}{}",
                    "(".b_yellow(),
                    "default".bold().b_magenta(),
                    ")".b_yellow()
                )
            } else {
                "".into()
            }
        );
    }
}
