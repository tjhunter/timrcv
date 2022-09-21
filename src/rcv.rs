use log::{debug, info, warn};

use ranked_voting::*;
use snafu::{prelude::*, ErrorCompat, ResultExt, Snafu};

use std::fs;
use std::path::{Path, PathBuf};

use calamine::{open_workbook, Reader, Xlsx};

use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Map as JSMap;
use serde_json::Value as JSValue;
use std::collections::HashSet;
use text_diff::print_diff;

use crate::rcv::config_reader::*;
pub mod io_cdf;
pub mod io_common;
pub mod io_dominion;
pub mod io_ess;
mod io_msforms;

#[derive(Debug, Snafu)]
pub enum RcvError {
    // General
    #[snafu(display(""))]
    OpeningFile {
        source: Box<RcvError>,
        root_path: String,
    },
    #[snafu(display(""))]
    UnknownFormat { format: String },

    // Excel
    #[snafu(display("Error opening file {path}"))]
    OpeningExcel {
        source: calamine::XlsxError,
        path: String,
    },
    #[snafu(display(""))]
    EmptyExcel {},
    #[snafu(display(""))]
    ExcelWrongCellType { lineno: u64, content: String },
    #[snafu(display(""))]
    ExcelCannotFindCandidateInHeader { candidate_name: String },

    // Format issues
    #[snafu(display(""))]
    CdfParsingJson {},
    #[snafu(display(""))]
    DominionParsingJson {},
    #[snafu(display(""))]
    DominionMissingCandidateId { candidate_name: String },
    #[snafu(display(""))]
    DominionParsingCandidateId { source: std::num::ParseIntError },
    #[snafu(display(""))]
    OpeningJson {
        source: std::io::Error,
        path: String,
    },
    #[snafu(display(""))]
    ParsingJson { source: serde_json::Error },

    #[snafu(display(""))]
    MissingChoices {},

    #[snafu(display(""))]
    ParsingJsonNumber {},
    #[snafu(display(""))]
    MissingParentDir {},

    #[snafu(display("ID may not be less than 10, but it was {id}"))]
    InvalidId { id: u16 },

    #[snafu(display(""))]
    ConfigOpeningJson { source: std::io::Error },

    // Reference errors
    #[snafu(display(""))]
    ReferenceOpeningFile { source: Box<RcvError> },

    // Summary errors
    #[snafu(display(""))]
    SummaryWrite {
        source: std::io::Error,
        path: String,
    },

    #[snafu(whatever, display("{message}"))]
    Whatever {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
        source: Option<Box<dyn std::error::Error>>,
    },
}

pub type RcvResult<T> = Result<T, RcvError>;
type BRcvResult<T> = Result<T, Box<RcvError>>;

fn result_stats_to_json(rs: &VotingResult) -> Vec<JSValue> {
    let mut l: Vec<JSValue> = Vec::new();
    let num_rounds = rs.round_stats.len();
    for (idx, _round_stat) in rs.round_stats.iter().enumerate() {
        let round_stat = _round_stat.clone();
        let mut tally: JSMap<String, JSValue> = JSMap::new();
        for (name, count) in round_stat.tally {
            tally.insert(name.clone(), json!(count.to_string()));
        }

        let mut tally_results: Vec<JSValue> = Vec::new();
        for elim_stats in round_stat.tally_result_eliminated {
            let mut transfers: JSMap<String, JSValue> = JSMap::new();
            for (name, count) in elim_stats.transfers {
                // No UWI to account for in transfers for now
                // TODO: check that this is the case
                transfers.insert(name, json!(count.to_string()));
            }
            if elim_stats.exhausted > 0 {
                transfers.insert(
                    "exhausted".to_string(),
                    json!(elim_stats.exhausted.to_string()),
                );
            }
            // The eliminated candidates are not output for the last round.
            if idx < num_rounds - 1 {
                tally_results.push(json!({
                    "eliminated": elim_stats.name.clone(),
                    "transfers": transfers
                }));
            }
        }
        for winner_name in round_stat.tally_results_elected {
            tally_results.push(json!({
                "elected": winner_name,
                "transfers": {}
            }));
        }

        let js = json!({"round": round_stat.round, "tally": tally, "tallyResults": tally_results});
        l.push(js);
    }
    l
}

pub mod config_reader {
    use crate::rcv::*;

    #[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
    pub struct OutputSettings {
        #[serde(rename = "contestName")]
        pub contest_name: String,
        #[serde(rename = "outputDirectory")]
        pub output_directory: Option<String>,
        #[serde(rename = "contestDate")]
        pub contest_date: Option<String>,
        #[serde(rename = "contestJurisdiction")]
        pub contest_juridiction: Option<String>,
        #[serde(rename = "contestOffice")]
        pub contest_office: Option<String>,
        #[serde(rename = "tabulateByPrecinct")]
        pub tabulate_by_precinct: Option<bool>,
        #[serde(rename = "generateCdfJson")]
        pub generate_cdf_json: Option<bool>,
    }

    #[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
    pub struct OutputConfig {
        pub contest: String,
        pub date: Option<String>,
        pub jurisdiction: Option<String>,
        pub office: Option<String>,
        pub threshold: Option<String>,
    }

    #[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
    pub struct FileSource {
        pub provider: String,
        #[serde(rename = "filePath")]
        pub file_path: String,
        #[serde(rename = "contestId")]
        pub contest_id: Option<String>,
        #[serde(rename = "firstVoteColumnIndex")]
        _first_vote_column_index: Option<JSValue>,
        #[serde(rename = "firstVoteRowIndex")]
        pub first_vote_row_index: Option<JSValue>,
        #[serde(rename = "idColumnIndex")]
        pub id_column_index: Option<String>,
        #[serde(rename = "precinctColumnIndex")]
        pub precinct_column_index: Option<String>,
        #[serde(rename = "overvoteDelimiter")]
        pub overvote_delimiter: Option<String>,
        #[serde(rename = "overvoteLabel")]
        pub overvote_label: Option<String>,
        #[serde(rename = "undervoteLabel")]
        pub undervote_label: Option<String>,
        #[serde(rename = "undeclaredWriteInLabel")]
        pub undeclared_write_in_label: Option<String>,
        #[serde(rename = "treatBlankAsUndeclaredWriteIn")]
        pub treat_blank_as_undeclared_write_in: Option<bool>,
        // New options specific to timrcv
        #[serde(rename = "excelWorksheetName")]
        pub excel_worksheet_name: Option<String>,
        #[serde(rename = "choices")]
        pub choices: Option<Vec<String>>,
    }

    impl FileSource {
        pub fn first_vote_column_index(&self) -> RcvResult<usize> {
            let x = read_js_int(&self._first_vote_column_index)?;
            Ok(x - 1)
        }
    }

    #[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
    pub struct RcvCandidate {
        pub name: String,
        pub code: Option<String>,
        pub excluded: Option<bool>,
    }

    #[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
    pub struct RcvRules {
        #[serde(rename = "tiebreakMode")]
        pub tiebreak_mode: String,
        #[serde(rename = "overvoteRule")]
        pub _overvote_rule: String,
        #[serde(rename = "winnerElectionMode")]
        pub winner_election_mode: String,
        #[serde(rename = "randomSeed")]
        pub random_seed: Option<String>,
        #[serde(rename = "maxSkippedRanksAllowed")]
        pub max_skipped_ranks_allowed: String,
        #[serde(rename = "maxRankingsAllowed")]
        pub max_rankings_allowed: String,
        #[serde(rename = "rulesDescription")]
        pub rules_description: Option<String>,
        #[serde(rename = "batchElimination")]
        pub batch_elimination: Option<bool>,
        #[serde(rename = "exhaustOnDuplicateCandidate")]
        pub exhaust_on_duplicate_candidate: Option<bool>,
    }

    impl RcvRules {
        pub fn overvote_rule(&self) -> RcvResult<OverVoteRule> {
            match self._overvote_rule.as_str() {
                "exhaustImmediately" => Ok(OverVoteRule::ExhaustImmediately),
                "alwaysSkipToNextRank" => Ok(OverVoteRule::AlwaysSkipToNextRank),
                "invalidOption" => whatever!("overvote rule is an invalid option for this contest"),
                _ => whatever!("unknown overvote rule: {}", self._overvote_rule),
            }
        }
    }

    #[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
    pub struct RcvConfig {
        #[serde(rename = "outputSettings")]
        pub output_settings: OutputSettings,
        #[serde(rename = "cvrFileSources")]
        pub cvr_file_sources: Vec<FileSource>,
        pub candidates: Vec<RcvCandidate>,
        pub rules: RcvRules,
    }

    pub fn read_summary(path: String) -> BRcvResult<JSValue> {
        let contents = fs::read_to_string(path.clone()).context(OpeningJsonSnafu { path })?;
        // debug!("read content: {:?}", contents);
        let mut js: JSValue =
            serde_json::from_str(contents.as_str()).context(ParsingJsonSnafu {})?;
        // Order the tally results to ensure stability
        // Remove the mention of the undeclared write-in's when they have zero votes associated to them.
        let results_ordered: Vec<JSValue> = js["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|jsv| {
                let mut res = jsv.clone();
                let mut tally_results: Vec<JSValue> = res["tallyResults"]
                    .as_array()
                    .unwrap()
                    .clone()
                    .iter()
                    .filter(|jsv| {
                        let obj = jsv.as_object().unwrap().clone();
                        if obj.get("eliminated").is_some() {
                            !obj.get("transfers")
                                .unwrap()
                                .as_object()
                                .unwrap()
                                .is_empty()
                        } else {
                            true
                        }
                    })
                    .cloned()
                    .collect();

                tally_results.sort_by_key(|trjs| {
                    let obj = trjs.as_object().unwrap().clone();
                    let elected = obj.get("elected");
                    let eliminated = obj.get("eliminated");
                    let s: String = elected
                        .or(eliminated)
                        .map(|x| x.as_str().unwrap().to_string())
                        .unwrap();
                    s
                });

                let mut tally = res["tally"].as_object().unwrap().clone();
                let k = "Undeclared Write-ins".to_string();
                if let Some(v) = tally.get(&k) {
                    if v.as_str() == Some("0") {
                        tally.remove(&k);
                    }
                }

                res["tallyResults"] = serde_json::Value::Array(tally_results);
                res["tally"] = serde_json::Value::Object(tally);
                res
            })
            .collect();
        js["results"] = serde_json::Value::Array(results_ordered);
        // debug!("read content: {:?}", js["results"].as_array().unwrap());
        Ok(js)
    }

    fn read_js_int(x: &Option<JSValue>) -> RcvResult<usize> {
        match x {
            Some(JSValue::Number(n)) => n
                .as_u64()
                .map(|x| x as usize)
                .context(ParsingJsonNumberSnafu {}),
            // Parsing the Excel-style columns
            Some(JSValue::String(s)) if s.chars().all(|c| c.is_alphabetic()) => {
                // Just treating the simple case for now. It should be expanded to more than 26 columns.
                assert_eq!(s.chars().count(), 1);
                let c1: char = s.to_lowercase().chars().next().unwrap();
                Ok((c1 as usize) - ('a' as usize))
            }
            Some(JSValue::String(s)) => s.parse::<usize>().ok().context(ParsingJsonNumberSnafu {}),
            _ => None.context(ParsingJsonNumberSnafu {}),
        }
    }
}

/// A ballot, as parsed by the readers
/// This is before applying rules for undervote, blanks, etc.
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct ParsedBallot {
    // TODO: add precinct
    // TODO: add filename?
    pub id: Option<String>,
    pub count: Option<u64>,
    pub choices: Vec<Vec<String>>,
}

fn read_ranking_data(
    root_path: String,
    cfs: &FileSource,
    candidates: &[RcvCandidate],
    rules: &RcvRules,
) -> RcvResult<Vec<ranked_voting::Vote>> {
    let p: PathBuf = [root_path.clone(), cfs.file_path.clone()].iter().collect();
    let p2 = p.as_path().display().to_string();
    info!("Attempting to read rank file {:?}", p2);
    let cand_names: Vec<String> = candidates.iter().map(|c| c.name.clone()).collect();
    let parsed_ballots = match cfs.provider.as_str() {
        "ess" => io_ess::read_excel_file(p2, cfs).context(OpeningFileSnafu { root_path })?,
        "cdf" => io_cdf::read_json(p2).context(OpeningFileSnafu { root_path })?,
        "dominion" => io_dominion::read_dominion(&p2).context(OpeningFileSnafu { root_path })?,
        "msforms_ranking" => {
            io_msforms::read_msforms_ranking(p2, cfs).context(OpeningFileSnafu { root_path })?
        }
        "msforms_likert" => io_msforms::read_msforms_likert(p2, cfs, &cand_names)
            .context(OpeningFileSnafu { root_path })?,
        "msforms_likert_transpose" => {
            io_msforms::read_msforms_likert_transpose(p2, cfs, &cand_names)
                .context(OpeningFileSnafu { root_path })?
        }
        x => {
            return Err(RcvError::UnknownFormat {
                format: x.to_string(),
            })
        }
    };
    validate_ballots(&parsed_ballots, candidates, cfs, rules)
}

fn validate_ballots(
    parsed_ballots: &[ParsedBallot],
    candidates: &[RcvCandidate],
    source: &FileSource,
    _rules: &RcvRules,
) -> RcvResult<Vec<Vote>> {
    let candidate_names: HashSet<String> = candidates.iter().map(|c| c.name.clone()).collect();
    let mut res: Vec<Vote> = Vec::new();

    let treat_blank_as_undeclared_write_in =
        source.treat_blank_as_undeclared_write_in.unwrap_or(false);

    for pb in parsed_ballots.iter() {
        let mut choices: Vec<BallotChoice> = Vec::new();

        for s in pb.choices.iter() {
            let res: BallotChoice = match &s[..] {
                [] => BallotChoice::Undervote,
                [_, _, ..] => BallotChoice::Overvote,
                [c] if candidate_names.contains(c) => BallotChoice::Candidate(c.to_string()),
                [c] if c == "UWI" => BallotChoice::UndeclaredWriteIn,
                [c] if source.undervote_label == Some(c.to_string()) => BallotChoice::Undervote,
                [c] if source.overvote_label == Some(c.to_string()) => BallotChoice::Overvote,
                [c] if c.is_empty() => {
                    if treat_blank_as_undeclared_write_in {
                        BallotChoice::UndeclaredWriteIn
                    } else {
                        BallotChoice::Blank
                    }
                }
                [c] => {
                    if let Some(delim) = source.overvote_delimiter.clone() {
                        if c.contains(&delim) {
                            BallotChoice::Overvote
                        } else {
                            BallotChoice::UndeclaredWriteIn
                        }
                    } else {
                        BallotChoice::UndeclaredWriteIn
                    }
                }
            };
            choices.push(res);
        }

        debug!(
            "validate_ballots: Choices for ballot {:?}: {:?}",
            pb.id, choices
        );

        // Default of 1 if not specified
        let count = pb.count.unwrap_or(1);

        if count > 0 && !candidates.is_empty() {
            let v = Vote {
                candidates: choices,
                count,
            };
            debug!(
                "validate_ballots: ballot {:?}: adding vote {:?}",
                pb.id,
                v.clone()
            );
            res.push(v);
        }
    }
    Ok(res)
}

fn validate_rules(rcv_rules: &RcvRules) -> RcvResult<VoteRules> {
    let res = VoteRules {
        tiebreak_mode: match rcv_rules.tiebreak_mode.as_str() {
            "useCandidateOrder" => TieBreakMode::UseCandidateOrder,
            "random" => {
                let seed = match rcv_rules.random_seed.clone().map(|s| s.parse::<u32>()) {
                    Some(Result::Ok(x)) => x,
                    x => {
                        whatever!(
                            "Cannot use tiebreak mode {:?} (currently not implemented)",
                            x
                        )
                    }
                };
                TieBreakMode::Random(seed)
            }
            x => {
                whatever!(
                    "Cannot use tiebreak mode {:?} (currently not implemented)",
                    x
                )
            }
        },
        max_skipped_rank_allowed: match rcv_rules.max_skipped_ranks_allowed.as_str() {
            "unlimited" => MaxSkippedRank::Unlimited,
            "0" => MaxSkippedRank::ExhaustOnFirstOccurence,
            x => match x.parse() {
                Ok(num) => MaxSkippedRank::MaxAllowed(num),
                _ => {
                    whatever!(
                        "Value '{:?}' cannot be understood for maxSkippedRanksAllowed",
                        rcv_rules.max_rankings_allowed
                    )
                }
            },
        },
        overvote_rule: rcv_rules.overvote_rule()?,
        winner_election_mode: match rcv_rules.winner_election_mode.as_str() {
            "singleWinnerMajority" => WinnerElectionMode::SingelWinnerMajority,
            x => {
                whatever!(
                    "Cannot use election mode {:?}: currently not implemented",
                    x
                )
            }
        },
        number_of_winners: 1,         // TODO: implement
        minimum_vote_threshold: None, // TODO: implement
        max_rankings_allowed: match rcv_rules.max_rankings_allowed.parse::<u32>() {
            Err(_) if rcv_rules.max_rankings_allowed == "max" => None,
            Result::Ok(x) if x > 0 => Some(x),
            x => {
                whatever!(
                    "Failed to understand maxRankingsAllowed option: {:?}: currently not implemented",
                    x
                )
            }
        },
        elimination_algorithm: {
            if rcv_rules.batch_elimination.unwrap_or(false) {
                EliminationAlgorithm::Batch
            } else {
                EliminationAlgorithm::Single
            }
        },
        duplicate_candidate_mode: match rcv_rules.exhaust_on_duplicate_candidate {
            Some(true) => DuplicateCandidateMode::Exhaust,
            _ => DuplicateCandidateMode::SkipDuplicate,
        },
    };
    Ok(res)
}

fn build_summary_js(config: &RcvConfig, rv: &VotingResult) -> JSValue {
    let c = OutputConfig {
        contest: config.output_settings.contest_name.clone(),
        date: config.output_settings.contest_date.clone(),
        jurisdiction: config.output_settings.contest_juridiction.clone(),
        office: config.output_settings.contest_office.clone(),
        threshold: Some(rv.threshold.to_string()),
    };
    json!({
        "config": c,
         "results": result_stats_to_json(rv) })
}

// override_out_path: used in test mode to disregard any output to disk.
pub fn run_election(
    config_path: String,
    check_summary_path: Option<String>,
    out_path: Option<String>,
    override_out_path: bool,
) -> RcvResult<()> {
    let config_p = Path::new(config_path.as_str());
    debug!("Opening file {:?}", config_p);
    let config_str = fs::read_to_string(config_path.clone()).context(ConfigOpeningJsonSnafu {})?;
    let config: RcvConfig = serde_json::from_str(&config_str).context(ParsingJsonSnafu {})?;
    let config2 = config.clone();
    debug!("run_election: config: {:?}", config);

    // Validate the rules:
    let rules = validate_rules(&config.rules)?;

    if config.cvr_file_sources.is_empty() {
        unimplemented!("no file sources detected");
    }

    let root_p = config_p.parent().context(MissingParentDirSnafu {})?;
    let mut data: Vec<Vote> = Vec::new();
    for cfs in config.cvr_file_sources {
        let mut file_data = read_ranking_data(
            root_p.as_os_str().to_str().unwrap().to_string(),
            &cfs,
            &config.candidates,
            &config.rules,
        )?;
        data.append(&mut file_data);
    }

    debug!("run_election:data: {:?} vote records", data.len());

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

    let result = match res {
        Result::Ok(x) => x,
        Result::Err(x) => {
            whatever!("Voting error: {:?}", x)
        }
    };

    // Assemble the final json
    let result_js = build_summary_js(&config2, &result);

    let pretty_js_stats = serde_json::to_string_pretty(&result_js).context(ParsingJsonSnafu {})?;
    debug!("stats:{}", pretty_js_stats);

    // The reference summary, if provided for comparison
    if let Some(ref_summary_path) = check_summary_path {
        let summary_ref = read_summary(ref_summary_path).context(ReferenceOpeningFileSnafu {})?;
        let pretty_js_summary_ref =
            serde_json::to_string_pretty(&summary_ref).context(ParsingJsonSnafu {})?;
        if pretty_js_summary_ref != pretty_js_stats {
            warn!("Found differences with the reference string");
            print_diff(
                pretty_js_summary_ref.as_str(),
                pretty_js_stats.as_ref(),
                "\n",
            );
            whatever!("Difference detected between calculated summary and reference summary")
        }
    }

    let default_out_path = config.output_settings.output_directory.map(|p| {
        let pb: PathBuf = vec![p, "summary.json".to_string()].iter().collect();
        pb.as_os_str().to_str().unwrap().to_string()
    });

    if let Some(out_p) = if override_out_path {
        out_path
    } else {
        out_path.or(default_out_path)
    } {
        if out_p == "stdout" {
            print!("{}", pretty_js_stats);
        } else if out_p.is_empty() {
        } else {
            debug!("Writing output to {}", out_p);
            fs::write(out_p.clone(), pretty_js_stats).context(SummaryWriteSnafu {
                path: out_p.clone(),
            })?;
            info!("Output written to {}", out_p);
        }
    }

    Ok(())
}

fn run_election_test(test_name: &str, config_lpath: &str, summary_lpath: &str, is_local: bool) {
    let test_dir = if is_local {
        "./tests"
    } else {
        option_env!("RCV_TEST_DIR").unwrap_or(
        "/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data",
    )
    };
    info!("Running test {}", test_name);
    let res = run_election(
        format!("{}/{}/{}", test_dir, test_name, config_lpath),
        Some(format!("{}/{}/{}", test_dir, test_name, summary_lpath)),
        None,
        true,
    );
    if let Err(e) = res {
        warn!("Error occured {:?}", e);
        eprintln!("An error occured {:?}", e);
        if let Some(bt) = ErrorCompat::backtrace(&e) {
            eprintln!("trace: {}", bt);
        } else {
            eprintln!("No trace found");
        }
        panic!("Exiting hard");
    }
}

pub fn test_wrapper(test_name: &str) {
    run_election_test(
        test_name,
        format!("{}_config.json", test_name).as_str(),
        format!("{}_expected_summary.json", test_name).as_str(),
        false,
    )
}

pub fn test_wrapper_local(test_name: &str) {
    run_election_test(
        test_name,
        format!("{}_config.json", test_name).as_str(),
        format!("{}_expected_summary.json", test_name).as_str(),
        true,
    )
}

#[cfg(test)]
mod tests {

    use super::test_wrapper;
    use super::test_wrapper_local;

    // #[test]
    // fn _2013_minneapolis_mayor() {
    //     test_wrapper("2013_minneapolis_mayor");
    // }

    // Takes about 100s to complete on github actions, disabled for the time being.
    #[test]
    #[ignore = "SLOW"]
    fn _2013_minneapolis_mayor_scale() {
        test_wrapper("2013_minneapolis_mayor_scale");
    }

    #[test]
    fn _2015_portland_mayor() {
        test_wrapper("2015_portland_mayor");
    }

    #[test]
    #[ignore = "TODO implement clearBallot provider"]
    fn clear_ballot_kansas_primary() {
        // TODO P1
        test_wrapper("clear_ballot_kansas_primary");
    }

    #[test]
    #[ignore = "TODO GH-17"]
    fn continue_tabulation_test() {
        test_wrapper("continue_tabulation_test");
    }

    #[test]
    #[ignore = "GH-17"]
    fn continue_until_two_with_batch_elimination_test() {
        test_wrapper("continue_until_two_with_batch_elimination_test");
    }

    #[test]
    fn dominion_alaska() {
        test_wrapper("dominion_alaska");
    }

    #[test]
    fn dominion_kansas() {
        test_wrapper("dominion_kansas");
    }

    #[test]
    #[ignore = "TODO GH-10"]
    fn dominion_multi_file() {
        test_wrapper("dominion_multi_file");
    }

    #[test]
    fn dominion_no_precinct_data() {
        test_wrapper("dominion_no_precinct_data");
    }

    #[test]
    fn dominion_wyoming() {
        test_wrapper("dominion_wyoming");
    }

    #[test]
    fn duplicate_test() {
        test_wrapper("duplicate_test");
    }

    #[test]
    #[ignore = "TODO P3 stopCountingAndAsk"]
    fn excluded_test() {
        test_wrapper("excluded_test");
    }

    #[test]
    #[ignore = "TODO P2 exhaustIfMultipleContinuing"]
    fn exhaust_if_multiple_continuing() {
        test_wrapper("exhaust_if_multiple_continuing");
    }

    #[test]
    #[ignore = "TODO implement hart input"]
    fn hart_cedar_park_school_board() {
        test_wrapper("hart_cedar_park_school_board");
    }

    #[test]
    #[ignore = "TODO implement hart input"]
    fn hart_travis_county_officers() {
        test_wrapper("hart_travis_county_officers");
    }

    #[test]
    #[ignore = "Alreday caught by the parser"]
    fn invalid_params_test() {
        test_wrapper("invalid_params_test");
    }

    #[test]
    #[ignore = "Alreday caught by the parser"]
    fn invalid_sources_test() {
        test_wrapper("invalid_sources_test");
    }

    #[test]
    #[ignore]
    fn minimum_threshold_test() {
        // TODO P1
        test_wrapper("minimum_threshold_test");
    }

    #[test]
    #[ignore = "TODO P3 stopCountingAndAsk"]
    fn minneapolis_multi_seat_threshold() {
        test_wrapper("minneapolis_multi_seat_threshold");
    }

    #[test]
    fn missing_precinct_example() {
        test_wrapper("missing_precinct_example");
    }

    #[test]
    #[ignore = "TODO P1 bottomsUpUsingPercentageThreshold"]
    fn multi_seat_bottoms_up_with_threshold() {
        test_wrapper("multi_seat_bottoms_up_with_threshold");
    }

    #[test]
    #[ignore = "TODO P1 implement multiWinnerAllowMultipleWinnersPerRound"]
    fn multi_seat_uwi_test() {
        test_wrapper("multi_seat_uwi_test");
    }

    #[test]
    #[ignore = "TODO implement cdf provider"]
    fn nist_xml_cdf_2() {
        test_wrapper("nist_xml_cdf_2");
    }

    #[test]
    #[ignore = "TODO GH-16"]
    fn no_one_meets_minimum() {
        test_wrapper("no_one_meets_minimum");
    }

    #[test]
    fn precinct_example() {
        test_wrapper("precinct_example");
    }

    #[test]
    #[ignore = "TODO P3 stopCountingAndAsk"]
    fn sample_interactive_tiebreak() {
        test_wrapper("sample_interactive_tiebreak");
    }

    #[test]
    #[ignore = "TODO P1 multiPassIrv"]
    fn sequential_with_batch() {
        test_wrapper("sequential_with_batch");
    }

    #[test]
    #[ignore = "TODO P1 multiPassIrv"]
    fn sequential_with_continue_until_two() {
        test_wrapper("sequential_with_continue_until_two");
    }

    #[test]
    fn skip_to_next_test() {
        test_wrapper("skip_to_next_test");
    }

    #[test]
    fn test_set_0_skipped_first_choice() {
        test_wrapper("test_set_0_skipped_first_choice");
    }

    #[test]
    fn test_set_1_exhaust_at_overvote() {
        test_wrapper("test_set_1_exhaust_at_overvote");
    }

    #[test]
    fn test_set_2_overvote_skip_to_next() {
        test_wrapper("test_set_2_overvote_skip_to_next");
    }

    #[test]
    fn test_set_3_skipped_choice_exhaust() {
        test_wrapper("test_set_3_skipped_choice_exhaust");
    }

    #[test]
    fn test_set_4_skipped_choice_next() {
        test_wrapper("test_set_4_skipped_choice_next");
    }

    #[test]
    fn test_set_5_two_skipped_choice_exhaust() {
        test_wrapper("test_set_5_two_skipped_choice_exhaust");
    }

    #[test]
    fn test_set_6_duplicate_exhaust() {
        test_wrapper("test_set_6_duplicate_exhaust");
    }

    #[test]
    fn test_set_7_duplicate_skip_to_next() {
        test_wrapper("test_set_7_duplicate_skip_to_next");
    }

    #[test]
    fn test_set_8_multi_cdf() {
        test_wrapper("test_set_8_multi_cdf");
    }

    #[test]
    #[ignore = "TODO P1 election mode multiWinnerAllowOnlyOneWinnerPerRound"]
    fn test_set_allow_only_one_winner_per_round() {
        test_wrapper("test_set_allow_only_one_winner_per_round");
    }

    #[test]
    #[ignore = "P3 tiebreak stopCountingAndAsk"]
    fn test_set_multi_winner_fractional_threshold() {
        test_wrapper("test_set_multi_winner_fractional_threshold");
    }

    #[test]
    #[ignore = "P3 tiebreak stopCountingAndAsk"]
    fn test_set_multi_winner_whole_threshold() {
        test_wrapper("test_set_multi_winner_whole_threshold");
    }

    #[test]
    #[ignore = "P1 unknown overvote rule: exhaustIfMultipleContinuing"]
    fn test_set_overvote_delimiter() {
        test_wrapper("test_set_overvote_delimiter");
    }

    #[test]
    #[ignore = "TODO P1 random"]
    fn test_set_treat_blank_as_undeclared_write_in() {
        test_wrapper("test_set_treat_blank_as_undeclared_write_in");
    }

    #[test]
    #[ignore = "P1 tiebreak generatePermutation"]
    fn tiebreak_generate_permutation_test() {
        test_wrapper("tiebreak_generate_permutation_test");
    }

    #[test]
    #[ignore = "P1 tiebreak previousRoundCountsThenRandom"]
    fn tiebreak_previous_round_counts_then_random_test() {
        test_wrapper("tiebreak_previous_round_counts_then_random_test");
    }

    #[test]
    #[ignore = "TODO P2 output format is different"]
    fn tiebreak_seed_test() {
        test_wrapper("tiebreak_seed_test");
    }

    #[test]
    fn tiebreak_use_permutation_in_config_test() {
        test_wrapper("tiebreak_use_permutation_in_config_test");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn unisyn_xml_cdf_city_chief_of_police() {
        test_wrapper("unisyn_xml_cdf_city_chief_of_police");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn unisyn_xml_cdf_city_coroner() {
        test_wrapper("unisyn_xml_cdf_city_coroner");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn unisyn_xml_cdf_city_council_member() {
        test_wrapper("unisyn_xml_cdf_city_council_member");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn unisyn_xml_cdf_city_mayor() {
        test_wrapper("unisyn_xml_cdf_city_mayor");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn unisyn_xml_cdf_city_tax_collector() {
        test_wrapper("unisyn_xml_cdf_city_tax_collector");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn unisyn_xml_cdf_county_coroner() {
        test_wrapper("unisyn_xml_cdf_county_coroner");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn unisyn_xml_cdf_county_sheriff() {
        test_wrapper("unisyn_xml_cdf_county_sheriff");
    }

    #[test]
    fn uwi_cannot_win_test() {
        test_wrapper("uwi_cannot_win_test");
    }

    // ********** Tests specific to timrcv *************

    #[test]
    fn msforms_1() {
        test_wrapper_local("msforms_1");
    }

    #[test]
    fn msforms_likert() {
        test_wrapper_local("msforms_likert");
    }

    #[test]
    fn msforms_likert_transpose() {
        test_wrapper_local("msforms_likert_transpose");
    }
}
