use std::process::exit;

use colorize::AnsiColor;
use reqwest::header::USER_AGENT;

use mimalloc::MiMalloc;

use ahash::RandomState;
use hashbrown::{HashMap as HHashMap, HashSet};

type BrownMap<K, V> = HHashMap<K, V, RandomState>;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(serde::Deserialize, Debug, Clone)]
struct CratesResponseVersion {
    num: String,
    features: BrownMap<String, Vec<String>>,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct CratesResponseCrate {
    max_stable_version: String,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct CratesResponse {
    #[serde(rename = "crate")]
    krate: CratesResponseCrate,
    versions: Vec<CratesResponseVersion>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

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

    let client = reqwest::blocking::Client::builder()
        .deflate(true)
        .gzip(true)
        .brotli(true)
        .zstd(true)
        .hickory_dns(true)
        .deflate(true)
        .build().map_err(|err| {
	       	eprintln!("{}{} {}{} {}\n- {:#?}", "<".b_black(), "Uh".yellow(), "oh".b_red(), ">".b_black(), "Couldn't create a reqwest client at all\nPlease submit this issue in the git repository with the following error:".yellow(), err);
			exit(100);
        }).unwrap();

    let user_agent = fake_user_agent::get_firefox_rua();

    let crate_name = args.first().unwrap().trim().replace("_", "-");
    let feat_filter = args
        .get(1)
        .unwrap_or(&String::from("all"))
        .trim()
        .to_string();
    let crate_api = "https://crates.io/api/v1/crates/";

    match client
        .get(format!("{}{}", crate_api, crate_name))
        .header(USER_AGENT, user_agent)
        .send()
    {
        Ok(response) => match response.bytes() {
            Ok(body) => {
                let mut body_bytes = body.to_vec();
                let data: CratesResponse = match simd_json::from_slice(&mut body_bytes) {
                    Ok(data) => data,
                    Err(err) => {
                        eprintln!(
                            "{}{} {}{} {}\n- {:#?}",
                            "<".b_black(),
                            "Uh".yellow(),
                            "oh".b_red(),
                            ">".b_black(),
                            "Received a bad response from the used api\nError details:".yellow(),
                            err
                        );
                        exit(103);
                    }
                };

                let crate_version = args
                    .get(2)
                    .map(|s| s.as_str())
                    .unwrap_or(&data.krate.max_stable_version);

                let mut features: Vec<_> = data
                    .versions
                    .iter()
                    .find(|i| i.num == crate_version)
                    .unwrap_or_else(|| {
                        eprintln!(
                            "{}{} {}{} {}\n- Couldn't find the specified version",
                            "<".b_black(),
                            "Uh".yellow(),
                            "oh".b_red(),
                            ">".b_black(),
                            "Received a bad response from the used api\nError details:".yellow(),
                        );
                        exit(104);
                    })
                    .features
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

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

                features.sort_by_key(|(key, _)| (key != "default", key.clone()));

                let default_features_set: HashSet<&String> = features
                    .iter()
                    .find(|(k, _)| k == "default")
                    .map(|(_, v)| v.iter().collect::<HashSet<_>>())
                    .unwrap_or_default();

                for (key, val) in features.iter().filter(|a| !a.0.starts_with("__")) {
                    if key != "default" {
                        println!(
                            "\t{} {} {}",
                            "—".b_magenta(),
                            key.to_owned().b_cyan().bold(),
                            if default_features_set.contains(key) {
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
                        continue;
                    }

                    if feat_filter == "nd" {
                        continue;
                    }

                    if val.is_empty() {
                        println!(
                            "\t{} {} \n\t     {}",
                            "★".b_yellow(),
                            key.to_owned().b_magenta().bold().underlined(),
                            "none".blue()
                        );
                        continue;
                    }

                    println!(
                        "\t{} {} \n\t     {}",
                        "★".b_yellow(),
                        key.to_owned().b_magenta().bold().underlined(),
                        val.join("\n\t     ").to_string().blue()
                    );
                }
            }
            Err(err) => {
                eprintln!(
                    "{}{} {}{} {}\n- {:#?}",
                    "<".b_black(),
                    "Uh".yellow(),
                    "oh".b_red(),
                    ">".b_black(),
                    "Received a bad response from the used api\nError details:".yellow(),
                    err
                );
                exit(102);
            }
        },
        Err(err) => {
            eprintln!(
                "{}{} {}{} {}\n- {:#?}",
                "<".b_black(),
                "Uh".yellow(),
                "oh".b_red(),
                ">".b_black(),
                "No response was received to your request\nExiting with the following error:"
                    .yellow(),
                err
            );
            exit(101);
        }
    }
}
