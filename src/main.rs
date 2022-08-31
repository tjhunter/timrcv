use env_logger;
use log::{debug, info};
use ranked_voting;
use ranked_voting::*;
use std::fs;

use anyhow::anyhow;
use anyhow::Result as AHResult;

use calamine::Error as CError;
use calamine::{open_workbook, Reader, Xlsx};

use serde_json::json;
use serde_json::Value;
use serde_json::Value::{Array, Bool, Number, Object, String as JSString};

fn result_stats_to_json(rs: &ResultStats) -> Value {
    let x: Vec<Value> = rs
        .rounds
        .iter()
        .enumerate()
        .map(|(idx, rs)| {
            let mut tally2 = rs.tally.clone();
            tally2.sort_by_key(|rcs| rcs.name.clone());
            let tally: Vec<Value> = tally2
                .iter()
                .map(|rcs| json!({rcs.name.clone() : rcs.tally.to_string()}))
                .collect();
            // TODO: tallyResults
            json!({"round": idx + 1, "tally": tally})
        })
        .collect();
    json!({ "results": x })
}

fn read_summary(path: String) -> AHResult<ResultStats> {
    let contents = fs::read_to_string(path)?;
    debug!("read content: {:?}", contents);
    let js: Value = serde_json::from_str(contents.as_str())?;
    debug!("read content: {:?}", js["results"].as_array().unwrap());
    Ok(ResultStats { rounds: Vec::new() })
}

fn read_file(path: String) -> AHResult<Vec<ranked_voting::Vote>> {
    let mut workbook: Xlsx<_> = open_workbook(path)?;
    let wrange = workbook
        .worksheet_range_at(0)
        .ok_or(CError::Msg("Missing first sheet"))??;
    let header = wrange
        .rows()
        .next()
        .ok_or(CError::Msg("Missing first row"))?;
    debug!("header: {:?}", header);
    let mut iter = wrange.rows();
    iter.next();
    let mut res: Vec<Vote> = Vec::new();
    for row in iter {
        debug!("workbook: {:?}", row);
        // Not looking at configuration for now: dropping the first column (id) and assuming that the last column is the weight.
        match row {
            [_, choices @ .., last] => {
                let cs: Result<Vec<String>, _> = choices
                    .iter()
                    .map(|elt| match elt {
                        calamine::DataType::String(s) => Ok(s.clone()),
                        _ => {
                            return Err(CError::Msg("wrong type"));
                        }
                    })
                    .collect();
                let count = match last {
                    calamine::DataType::Float(f) => *f as u64,
                    calamine::DataType::Int(i) => *i as u64,
                    _ => {
                        return Err(anyhow!(CError::Msg("wrong type")));
                    }
                };
                res.push(Vote {
                    candidates: cs?,
                    count: count,
                });
            }
            _ => {
                return Err(anyhow!(CError::Msg("wrong row")));
            }
        }
    }
    Ok(res)
}

fn main() {
    env_logger::init();
    // Test
    let data = read_file("/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data/precinct_example/precinct_example_cvr.xlsx".to_string());
    info!("data: {:?}", data);
    let summary = read_summary("/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data/precinct_example/precinct_example_expected_summary.json".to_string());
    info!("summary: {:?}", summary);

    let rules = VoteRules {
        tiebreak_mode: TieBreakMode::UseCandidateOrder,
        winner_election_mode: WinnerElectionMode::SingelWinnerMajority,
        number_of_winners: 1,
        minimum_vote_threshold: None,
        max_rankings_allowed: None,
    };

    let res = run_voting_stats(&data.unwrap(), &rules, &None);

    info!("res {:?}", res);

    let x = match res.unwrap() {
        VotingResult::NoMajorityCandidate => unimplemented!(""),
        VotingResult::SingleWinner(_, s) => s,
    };
    let pretty_js_stats = serde_json::to_string_pretty(&result_stats_to_json(&x)).unwrap();
    println!("stats:{}", pretty_js_stats);
}
