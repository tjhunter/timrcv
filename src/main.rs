
use log::{debug, info, warn};

use ranked_voting::*;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use anyhow::Result as AHResult;

use calamine::Error as CError;
use calamine::{open_workbook, Reader, Xlsx};

use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Map as JSMap;
use serde_json::Value as JSValue;
use text_diff::print_diff;

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct OutputSettings {
    #[serde(rename = "contestName")]
    contest_name: String,
    #[serde(rename = "outputDirectory")]
    output_directory: Option<String>,
    #[serde(rename = "contestDate")]
    contest_date: Option<String>,
    #[serde(rename = "contestJuridiction")]
    contest_juridiction: Option<String>,
    #[serde(rename = "contestOffice")]
    contest_office: Option<String>,
    #[serde(rename = "tabulateByPrecinct")]
    tabulate_by_precinct: Option<bool>,
    #[serde(rename = "generateCdfJson")]
    generate_cdf_json: Option<bool>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct FileSource {
    provider: String,
    #[serde(rename = "filePath")]
    file_path: String,
    #[serde(rename = "contestId")]
    contest_id: Option<String>,
    #[serde(rename = "firstVoteColumnIndex")]
    first_vote_column_index: Option<String>,
    #[serde(rename = "firstVoteRowIndex")]
    first_vote_row_index: Option<String>,
    #[serde(rename = "idColumnIndex")]
    id_column_index: Option<String>,
    #[serde(rename = "precinctColumnIndex")]
    precinct_column_index: Option<String>,
    #[serde(rename = "overvoteDelimiter")]
    overvote_delimiter: Option<String>,
    #[serde(rename = "overvoteLabel")]
    overvote_label: Option<String>,
    #[serde(rename = "undervoteLabel")]
    undervote_label: Option<String>,
    #[serde(rename = "undeclaredWriteInLabel")]
    undeclared_write_in_label: Option<String>,
    #[serde(rename = "treatBlankAsUndeclaredWriteIn")]
    treat_blank_as_undeclared_write_in: Option<bool>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct RcvCandidate {
    name: String,
    code: Option<String>,
    excluded: Option<bool>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct RcvRules {
    #[serde(rename = "tiebreakMode")]
    tiebreak_mode: String,
    #[serde(rename = "overvoteRule")]
    overvote_rule: String,
    #[serde(rename = "winnerElectionMode")]
    winner_election_mode: String,
    #[serde(rename = "randomSeed")]
    random_seed: Option<String>,
    #[serde(rename = "maxSkippedRanksAllowed")]
    max_skipped_ranks_allowed: String,
    #[serde(rename = "maxRankingsAllowed")]
    max_rankings_allowed: String,
    #[serde(rename = "rulesDescription")]
    rules_description: Option<String>,
    #[serde(rename = "exhaustOnDuplicateCandidate")]
    exhaust_on_duplicate_candidate: Option<bool>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct RcvConfig {
    #[serde(rename = "outputSettings")]
    output_settings: OutputSettings,
    #[serde(rename = "cvrFileSources")]
    cvr_file_sources: Vec<FileSource>,
    candidates: Vec<RcvCandidate>,
    rules: RcvRules,
}

fn result_stats_to_json(rs: &VotingResult) -> Vec<JSValue> {
    let mut l: Vec<JSValue> = Vec::new();
    for round_stat in rs.round_stats.clone() {
        let mut tally: JSMap<String, JSValue> = JSMap::new();
        for (name, count) in round_stat.tally {
            tally.insert(name.clone(), json!(count.to_string()));
        }

        let mut tally_results: Vec<JSValue> = Vec::new();
        for elim_stats in round_stat.tally_result_eliminated {
            let mut transfers: JSMap<String, JSValue> = JSMap::new();
            for (name, count) in elim_stats.transfers {
                transfers.insert(name, json!(count.to_string()));
            }
            if elim_stats.exhausted > 0 {
                transfers.insert(
                    "exhausted".to_string(),
                    json!(elim_stats.exhausted.to_string()),
                );
            }
            tally_results.push(json!({
                "eliminated": elim_stats.name,
                "transfers": transfers
            }));
        }

        let js = json!({"round": round_stat.round, "tally": tally, "tallyResults": tally_results});
        l.push(js);
    }
    l
}

fn read_summary(path: String) -> AHResult<JSValue> {
    let contents = fs::read_to_string(path)?;
    debug!("read content: {:?}", contents);
    let js: JSValue = serde_json::from_str(contents.as_str())?;
    debug!("read content: {:?}", js["results"].as_array().unwrap());
    Ok(js)
}

fn read_excel_file(path: String, _cfs: &FileSource) -> AHResult<Vec<ranked_voting::Vote>> {
    let mut workbook: Xlsx<_> = open_workbook(path)?;
    let wrange = workbook
        .worksheet_range_at(0)
        .ok_or(CError::Msg("Missing first sheet"))??;
    let header = wrange
        .rows()
        .next()
        .ok_or(CError::Msg("Missing first row"))?;
    debug!("header: {:?}", header);
    let start_range: usize = match _cfs
        .first_vote_column_index
        .clone()
        .map(|s| s.parse::<i32>())
    {
        Some(Ok(x)) if x >= 1 => (x - 1) as usize,
        _ => unimplemented!(
            "failed to find start range {:?}",
            _cfs.first_vote_column_index
        ),
    };

    let mut iter = wrange.rows();
    // TODO check for correctness
    iter.next();
    let mut res: Vec<Vote> = Vec::new();
    for row in iter {
        debug!("workbook: {:?}", row);
        // Not looking at configuration for now: dropping the first column (id) and assuming that the last column is the weight.
        let choices = &row[start_range..];
        let mut cs: Vec<String> = Vec::new();
        for elt in choices {
            match elt {
                // TODO: check for all the undervotes, overvotes, etc.
                calamine::DataType::String(s) => {
                    cs.push(s.clone());
                }
                // Undervote
                calamine::DataType::Empty => {}
                _ => {
                    return Err(anyhow!(CError::Msg("wrong type")));
                }
            }
        }
        // TODO implement count
        let count: u64 = match None {
            Some(calamine::DataType::Float(f)) => f as u64,
            Some(calamine::DataType::Int(i)) => i as u64,
            Some(_) => {
                return Err(anyhow!(CError::Msg("wrong type")));
            }
            None => 1,
        };
        res.push(Vote {
            candidates: cs.clone(),
            count,
        });
    }
    Ok(res)
}

fn read_ranking_data(root_path: String, cfs: &FileSource) -> AHResult<Vec<ranked_voting::Vote>> {
    let p: PathBuf = [root_path, cfs.file_path.clone()].iter().collect();
    let p2 = p.as_path().display().to_string();
    info!("Attempting to read rank file {:?}", p2);
    match cfs.provider.as_str() {
        "ess" => read_excel_file(p2, cfs),
        x => unimplemented!("Provider not implemented {:?}", x),
    }
}

fn validate_rules(rcv_rules: &RcvRules) -> AHResult<VoteRules> {
    let res = VoteRules {
        tiebreak_mode: match rcv_rules.tiebreak_mode.as_str() {
            "useCandidateOrder" => TieBreakMode::UseCandidateOrder,
            "random" => {
                let seed = match rcv_rules.random_seed.clone().map(|s| s.parse::<u32>()) {
                    Some(Ok(x)) => x,
                    x => {
                        return Err(anyhow!(
                            "Cannot use tiebreak mode {:?} (currently not implemented)",
                            x
                        ));
                    }
                };
                TieBreakMode::Random(seed)
            }
            x => {
                return Err(anyhow!(
                    "Cannot use tiebreak mode {:?} (currently not implemented)",
                    x
                ));
            }
        },
        winner_election_mode: match rcv_rules.winner_election_mode.as_str() {
            "singleWinnerMajority" => WinnerElectionMode::SingelWinnerMajority,
            x => {
                return Err(anyhow!(
                    "Cannot use election mode {:?}: currently not implemented",
                    x
                ));
            }
        },
        number_of_winners: 1,         // TODO: implement
        minimum_vote_threshold: None, // TODO: implement
        max_rankings_allowed: match rcv_rules.max_rankings_allowed.parse::<u32>() {
            Err(_) if rcv_rules.max_rankings_allowed == "max" => None,
            Ok(x) if x > 0 => Some(x),
            x => {
                return Err(anyhow!(
                    "Failed to understand maxRankingsAllowed option: {:?}: currently not implemented",
                    x
                ));
            }
        },
        duplicate_candidate_mode: match rcv_rules.exhaust_on_duplicate_candidate {
            Some(true) => DuplicateCandidateMode::Exhaust,
            _ => DuplicateCandidateMode::SkipDuplicate,
        },
    };
    Ok(res)
}

fn main() {
    env_logger::init();

    let config_path = "/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data/duplicate_test/duplicate_test_config.json";
    let config_p = Path::new(config_path);
    let config_str = fs::read_to_string(config_path).unwrap();
    let config: RcvConfig = serde_json::from_str(&config_str).unwrap();
    info!("config: {:?}", config);

    // Validate the rules:
    let rules = validate_rules(&config.rules).unwrap();

    if config.cvr_file_sources.is_empty() {
        unimplemented!("no file sources detected");
    }

    let root_p = config_p.parent().unwrap();
    let mut data: Vec<Vote> = Vec::new();
    for cfs in config.cvr_file_sources {
        let mut file_data =
            read_ranking_data(root_p.as_os_str().to_str().unwrap().to_string(), &cfs).unwrap();
        data.append(&mut file_data);
    }

    info!("data: {:?}", data);

    let candidates: Vec<Candidate> = config
        .candidates
        .iter()
        .map(|c| Candidate {
            name: c.name.clone(),
            code: match c.code.clone() {
                Some(x) if x.is_empty() => None,
                x => x,
            },
            excluded: c.excluded.unwrap_or(false),
        })
        .collect();

    let res = run_voting_stats(&data, &rules, &Some(candidates));

    info!("res {:?}", res);

    let result = res.unwrap();

    // Assemble the final json
    let result_js = json!({ "results": result_stats_to_json(&result) });

    // TODO
    let pretty_js_stats = serde_json::to_string_pretty(&result_js).unwrap();
    println!("stats:{}", pretty_js_stats);

    // The reference summary, if provided for comparison
    let summary_ref = read_summary("/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data/duplicate_test/duplicate_test_expected_summary.json".to_string()).unwrap();
    info!("summary: {:?}", summary_ref);
    let pretty_js_summary_ref = serde_json::to_string_pretty(&summary_ref).unwrap();
    if pretty_js_summary_ref != pretty_js_stats {
        warn!("Found differences with the reference string");
        print_diff(
            pretty_js_summary_ref.as_str(),
            pretty_js_stats.as_ref(),
            "\n",
        );
    }
}
