use log::{debug, info, warn};

use ranked_voting::*;
use snafu::{prelude::*, ErrorCompat, Snafu};

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

// All the possible choices that can be made on a ballot
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
enum BallotChoice {
    Candidate(String),
    UndeclaredWriteIn(String),
    Overvote,
    Undervote, // Blank vote
}

#[derive(Debug, Snafu)]
pub enum RcvError {
    #[snafu(display("Error opening file {path}"))]
    OpeningExcel {
        source: calamine::XlsxError,
        path: String,
    },
    #[snafu(display(""))]
    EmptyExcel {},
    #[snafu(display(""))]
    OpeningJson { source: std::io::Error },
    #[snafu(display(""))]
    ParsingJson { source: serde_json::Error },
    #[snafu(display(""))]
    ParsingJsonNumber {},
    #[snafu(display(""))]
    MissingParentDir {},

    #[snafu(display("ID may not be less than 10, but it was {id}"))]
    InvalidId { id: u16 },

    #[snafu(whatever, display("{message}"))]
    Whatever {
        message: String,
        #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
        source: Option<Box<dyn std::error::Error>>,
    },
}

type RcvResult<T> = Result<T, RcvError>;

fn result_stats_to_json(rs: &VotingResult) -> Vec<JSValue> {
    let mut l: Vec<JSValue> = Vec::new();
    let num_rounds = rs.round_stats.len();
    for (idx, _round_stat) in rs.round_stats.iter().enumerate() {
        let round_stat = _round_stat.clone();
        let mut tally: JSMap<String, JSValue> = JSMap::new();
        for (name, count) in round_stat.tally {
            let name2 = if name == UWI {
                "Undeclared Write-ins".to_string()
            } else {
                name.clone()
            };

            tally.insert(name2, json!(count.to_string()));
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
                let name2 = if elim_stats.name == UWI {
                    "Undeclared Write-ins".to_string()
                } else {
                    elim_stats.name.clone()
                };
                tally_results.push(json!({
                    "eliminated": name2,
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
        pub overvote_rule: String,
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
        #[serde(rename = "exhaustOnDuplicateCandidate")]
        pub exhaust_on_duplicate_candidate: Option<bool>,
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

    pub fn read_summary(path: String) -> RcvResult<JSValue> {
        let contents = fs::read_to_string(path).context(OpeningJsonSnafu {})?;
        debug!("read content: {:?}", contents);
        let js: JSValue = serde_json::from_str(contents.as_str()).context(ParsingJsonSnafu {})?;
        debug!("read content: {:?}", js["results"].as_array().unwrap());
        Ok(js)
    }

    fn read_js_int(x: &Option<JSValue>) -> RcvResult<usize> {
        match x {
            Some(JSValue::Number(n)) => n
                .as_u64()
                .map(|x| x as usize)
                .context(ParsingJsonNumberSnafu {}),
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
    pub choices: Vec<String>,
}

pub mod ess_reader {
    use crate::rcv::*;
    use std::collections::HashSet;

    pub fn read_excel_file(path: String, cfs: &FileSource) -> RcvResult<Vec<ParsedBallot>> {
        let p = path.clone();
        let mut workbook: Xlsx<_> =
            open_workbook(p).context(OpeningExcelSnafu { path: path.clone() })?;
        let wrange = workbook
            .worksheet_range_at(0)
            .context(EmptyExcelSnafu {})?
            .context(OpeningExcelSnafu { path })?;

        // .ok_or(CError::Msg("Missing first sheet"))??;
        let header = wrange.rows().next().context(EmptyExcelSnafu {})?;
        debug!("header: {:?}", header);
        let start_range = cfs.first_vote_column_index()?;

        let mut iter = wrange.rows();
        // TODO check for correctness
        iter.next();
        let mut res: Vec<ParsedBallot> = Vec::new();
        for row in iter {
            debug!("workbook: {:?}", row);
            // Not looking at configuration for now: dropping the first column (id) and assuming that the last column is the weight.
            let choices = &row[start_range..];
            let mut cs: Vec<String> = Vec::new();
            for elt in choices {
                let bc = read_choice_calamine2(elt)?;
                cs.push(bc)
            }
            // TODO implement count
            let count: u64 = match None {
                Some(calamine::DataType::Float(f)) => f as u64,
                Some(calamine::DataType::Int(i)) => i as u64,
                Some(_) => {
                    whatever!("wrong type")
                }
                None => 1,
            };
            res.push(ParsedBallot {
                id: None,
                count: Some(count),
                choices: cs,
            });
        }
        Ok(res)
    }

    fn read_choice_calamine2(cell: &calamine::DataType) -> RcvResult<String> {
        match cell {
            calamine::DataType::String(s) => Ok(s.clone()),
            calamine::DataType::Empty => Ok("".to_string()),
            _ => whatever!(
                "TODO MSG:read_choice_calamine: could not understand cell {:?}",
                cell
            ),
        }
    }

    pub fn read_excel_file0(
        path: String,
        cfs: &FileSource,
        candidates: &[RcvCandidate],
        rules: &RcvRules,
    ) -> RcvResult<Vec<ranked_voting::Vote>> {
        let p = path.clone();
        let mut workbook: Xlsx<_> =
            open_workbook(p).context(OpeningExcelSnafu { path: path.clone() })?;
        let wrange = workbook
            .worksheet_range_at(0)
            .context(EmptyExcelSnafu {})?
            .context(OpeningExcelSnafu { path })?;

        // .ok_or(CError::Msg("Missing first sheet"))??;
        let header = wrange.rows().next().context(EmptyExcelSnafu {})?;
        debug!("header: {:?}", header);
        let start_range = cfs.first_vote_column_index()?;

        let candidate_names: HashSet<String> = candidates.iter().map(|c| c.name.clone()).collect();

        let mut iter = wrange.rows();
        // TODO check for correctness
        iter.next();
        let mut res: Vec<Vote> = Vec::new();
        for row in iter {
            debug!("workbook: {:?}", row);
            // Not looking at configuration for now: dropping the first column (id) and assuming that the last column is the weight.
            let choices = &row[start_range..];
            let mut cs: Vec<BallotChoice> = Vec::new();
            for elt in choices {
                let bc = read_choice_calamine(elt, &candidate_names, cfs)?;
                cs.push(bc)
            }
            // TODO implement count
            let count: u64 = match None {
                Some(calamine::DataType::Float(f)) => f as u64,
                Some(calamine::DataType::Int(i)) => i as u64,
                Some(_) => {
                    whatever!("wrong type")
                }
                None => 1,
            };
            if let Some(v) = create_vote(&"NO ID".to_string(), count, &cs, rules)? {
                res.push(v);
            }
        }
        Ok(res)
    }

    fn read_choice_calamine(
        cell: &calamine::DataType,
        candidates: &HashSet<String>,
        source_setting: &FileSource,
    ) -> RcvResult<BallotChoice> {
        match cell {
            calamine::DataType::String(s) if candidates.contains(s) => {
                Ok(BallotChoice::Candidate(s.clone()))
            }
            calamine::DataType::String(s) if s == "UWI" => {
                Ok(BallotChoice::UndeclaredWriteIn("".to_string()))
            }
            calamine::DataType::String(s)
                if s.is_empty()
                    && source_setting
                        .treat_blank_as_undeclared_write_in
                        .unwrap_or(false) =>
            {
                Ok(BallotChoice::UndeclaredWriteIn("".to_string()))
            }
            calamine::DataType::String(s) if source_setting.undervote_label == Some(s.clone()) => {
                Ok(BallotChoice::Undervote)
            }
            calamine::DataType::String(s) => {
                if let Some(delim) = source_setting.overvote_delimiter.clone() {
                    if s.contains(&delim) {
                        return Ok(BallotChoice::Overvote);
                    }
                }
                whatever!("Wrong data type: {:?}", s)
            }
            calamine::DataType::Empty => Ok(BallotChoice::Undervote),
            _ => whatever!(
                "TODO MSG:read_choice_calamine: could not understand cell {:?}",
                cell
            ),
        }
    }
}

// TODO: add policy on how to treat the bad ballots.
fn create_vote(
    ballot_id: &String,
    count: u64,
    choices: &[BallotChoice],
    _rules: &RcvRules,
) -> RcvResult<Option<Vote>> {
    let mut candidates: Vec<String> = Vec::new();
    // For now, be very permissive.
    for c in choices {
        match c {
            BallotChoice::Candidate(s) => {
                candidates.push(s.clone());
            }
            BallotChoice::UndeclaredWriteIn(_) => {
                // TODO: this is hardcoded.
                candidates.push("UWI".to_string());
            }
            _ => {
                warn!(
                    "create_vote: ballot_id {}: skipping choice {:?}",
                    ballot_id, c
                );
            }
        }
    }
    Ok(Some(Vote { candidates, count }))
}

fn read_ranking_data(
    root_path: String,
    cfs: &FileSource,
    candidates: &[RcvCandidate],
    rules: &RcvRules,
) -> RcvResult<Vec<ranked_voting::Vote>> {
    let p: PathBuf = [root_path, cfs.file_path.clone()].iter().collect();
    let p2 = p.as_path().display().to_string();
    info!("Attempting to read rank file {:?}", p2);
    let parsed_ballots = match cfs.provider.as_str() {
        "ess" => ess_reader::read_excel_file(p2, cfs),
        x => unimplemented!("Provider not implemented {:?}", x),
    }?;
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
            let res: BallotChoice = match s.clone().as_str() {
                c if candidate_names.contains(c) => BallotChoice::Candidate(c.to_string()),
                "UWI" => BallotChoice::UndeclaredWriteIn("".to_string()),
                "" if treat_blank_as_undeclared_write_in => {
                    BallotChoice::UndeclaredWriteIn("".to_string())
                }
                "" => BallotChoice::Undervote,
                c if source.undervote_label == Some(c.to_string()) => BallotChoice::Undervote,
                c if source.overvote_label == Some(c.to_string()) => BallotChoice::Overvote,
                _ => {
                    if let Some(delim) = source.overvote_delimiter.clone() {
                        if s.contains(&delim) {
                            BallotChoice::Overvote
                        } else {
                            BallotChoice::UndeclaredWriteIn(s.clone())
                        }
                    } else {
                        BallotChoice::UndeclaredWriteIn(s.clone())
                    }
                }
            };
            choices.push(res);
        }

        debug!("Choices for ballot {:?}: {:?}", pb.id, choices);

        // Filter some of the choices.

        let candidates: Vec<String> = choices
            .iter()
            .filter_map(|x| match x {
                BallotChoice::Candidate(x) => Some(x.clone()),
                BallotChoice::UndeclaredWriteIn(_) => Some(UWI.to_string()),
                BallotChoice::Overvote => None,
                BallotChoice::Undervote => None,
            })
            .collect();

        // Default of 1 if not specified
        let count = pb.count.unwrap_or(1);
        if count > 0 && !candidates.is_empty() {
            res.push(Vote { candidates, count });
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

pub fn run_election(config_path: String, check_summary_path: Option<String>) -> RcvResult<()> {
    let config_p = Path::new(config_path.as_str());
    let config_str = fs::read_to_string(config_path.clone()).context(OpeningJsonSnafu {})?;
    let config: RcvConfig = serde_json::from_str(&config_str).context(ParsingJsonSnafu {})?;
    let config2 = config.clone();
    info!("config: {:?}", config);

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

    let result = match res {
        Result::Ok(x) => x,
        Result::Err(x) => {
            whatever!("Voting error: {:?}", x)
        }
    };

    // Assemble the final json
    let result_js = build_summary_js(&config2, &result);

    // TODO
    let pretty_js_stats = serde_json::to_string_pretty(&result_js).context(ParsingJsonSnafu {})?;
    println!("stats:{}", pretty_js_stats);

    // The reference summary, if provided for comparison
    if let Some(summary_p) = check_summary_path {
        let summary_ref = read_summary(summary_p)?;
        info!("summary: {:?}", summary_ref);
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

    Ok(())
}

fn run_election_test(test_name: &str, config_lpath: &str, summary_lpath: &str) {
    let test_dir = option_env!("RCV_TEST_DIR").unwrap_or(
        "/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data",
    );
    info!("Running test {}", test_name);
    let res = run_election(
        format!("{}/{}/{}", test_dir, test_name, config_lpath),
        Some(format!("{}/{}/{}", test_dir, test_name, summary_lpath)),
    );
    if let Err(e) = res {
        warn!("Error occured {:?}", e);
        eprintln!("An error occured {}", e);
        if let Some(bt) = ErrorCompat::backtrace(&e) {
            eprintln!("trace: {}", bt);
        } else {
            eprintln!("No trace found");
        }
    }
}

pub fn test_wrapper(test_name: &str) {
    run_election_test(
        test_name,
        format!("{}_config.json", test_name).as_str(),
        format!("{}_expected_summary.json", test_name).as_str(),
    )
}

// TODO p0 https://github.com/commure/datatest/tree/main/tests
#[cfg(test)]
mod tests {

    use super::test_wrapper;

    #[test]
    #[ignore = "TODO"]
    fn _2013_minneapolis_mayor() {
        // TODO P1
        test_wrapper("2013_minneapolis_mayor");
    }

    #[test]
    #[ignore = "TODO"]
    fn _2013_minneapolis_mayor_scale() {
        test_wrapper("2013_minneapolis_mayor_scale");
    }

    // #[test]
    // fn _2013_minneapolis_mayor() {
    //     // TODO P1
    //     test_wrapper("2013_minneapolis_mayor");
    // }

    #[test]
    #[ignore = "TODO implement clearBallot provider"]
    fn clear_ballot_kansas_primary() {
        // TODO P1
        test_wrapper("clear_ballot_kansas_primary");
    }

    #[test]
    #[ignore = "TODO P1 more permissive input"]
    fn continue_tabulation_test() {
        test_wrapper("continue_tabulation_test");
    }

    #[test]
    #[ignore]
    fn continue_until_two_with_batch_elimination_test() {
        test_wrapper("continue_until_two_with_batch_elimination_test");
    }

    #[test]
    #[ignore = "TODO implement dominion input"]
    fn dominion_alaska() {
        // TODO P1
        test_wrapper("dominion_alaska");
    }

    #[test]
    #[ignore = "TODO implement dominion input"]
    fn dominion_kansas() {
        // TODO P1
        test_wrapper("dominion_kansas");
    }

    #[test]
    #[ignore = "TODO implement dominion input"]
    fn dominion_multi_file() {
        // TODO P1
        test_wrapper("dominion_multi_file");
    }

    #[test]
    #[ignore = "TODO implement dominion input"]
    fn dominion_no_precinct_data() {
        // TODO P1
        test_wrapper("dominion_no_precinct_data");
    }

    #[test]
    #[ignore = "TODO implement dominion input"]
    fn dominion_wyoming() {
        // TODO P1
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
    #[ignore]
    fn exhaust_if_multiple_continuing() {
        // TODO P1 better input management
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
    #[ignore = "TODO P1"]
    fn missing_precinct_example() {
        // TODO P1
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
    #[ignore = "TODO P1 incomplete test, investigate failure mode"]
    fn no_one_meets_minimum() {
        test_wrapper("no_one_meets_minimum");
    }

    #[test]
    #[ignore = "TODO P0"]
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
    #[ignore = "TODO P2 provider cdf"]
    fn test_set_0_skipped_first_choice() {
        test_wrapper("test_set_0_skipped_first_choice");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn test_set_1_exhaust_at_overvote() {
        test_wrapper("test_set_1_exhaust_at_overvote");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn test_set_2_overvote_skip_to_next() {
        test_wrapper("test_set_2_overvote_skip_to_next");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn test_set_3_skipped_choice_exhaust() {
        test_wrapper("test_set_3_skipped_choice_exhaust");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn test_set_4_skipped_choice_next() {
        test_wrapper("test_set_4_skipped_choice_next");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn test_set_5_two_skipped_choice_exhaust() {
        test_wrapper("test_set_5_two_skipped_choice_exhaust");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn test_set_6_duplicate_exhaust() {
        test_wrapper("test_set_6_duplicate_exhaust");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
    fn test_set_7_duplicate_skip_to_next() {
        test_wrapper("test_set_7_duplicate_skip_to_next");
    }

    #[test]
    #[ignore = "TODO P2 provider cdf"]
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
    fn test_set_overvote_delimiter() {
        test_wrapper("test_set_overvote_delimiter");
    }

    #[test]
    #[ignore = "TODO P0"]
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
    #[ignore = "TODO P0"]
    fn uwi_cannot_win_test() {
        test_wrapper("uwi_cannot_win_test");
    }
}
