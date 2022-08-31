use env_logger;
use log::{debug, info, warn};
use ranked_voting;
use ranked_voting::*;

use calamine::{open_workbook, Error, Reader, Xlsx};

fn read_file() -> Result<Vec<ranked_voting::Vote>, Error> {
    let path = "/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data/precinct_example/precinct_example_cvr.xlsx";
    let mut workbook: Xlsx<_> = open_workbook(path)?;
    let wrange = workbook
        .worksheet_range_at(0)
        .ok_or(Error::Msg("Missing first sheet"))??;
    let header = wrange
        .rows()
        .next()
        .ok_or(Error::Msg("Missing first row"))?;
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
                            return Err(Error::Msg("wrong type"));
                        }
                    })
                    .collect();
                let count = match last {
                    calamine::DataType::Float(f) => *f as u64,
                    calamine::DataType::Int(i) => *i as u64,
                    _ => {
                        return Err(Error::Msg("wrong type"));
                    }
                };
                res.push(Vote {
                    candidates: cs?,
                    count: count,
                });
            }
            _ => {
                return Err(Error::Msg("wrong row"));
            }
        }
    }
    Ok(res)
}

fn main() {
    env_logger::init();
    // Test
    let data = read_file();
    info!("data: {:?}", data);

    let rules = VoteRules {
        tiebreak_mode: TieBreakMode::UseCandidateOrder,
        winner_election_mode: WinnerElectionMode::SingelWinnerMajority,
        number_of_winners: 1,
        minimum_vote_threshold: None,
        max_rankings_allowed: None,
    };

    let res = run_voting_stats(&data.unwrap(), &rules, &None);

    info!("res {:?}", res);

    // let num = 10;
    // println!(
    //     "Hello, world! {num} plus one is {}!",
    //     ranked_voting::add_one(num)
    // );
}
