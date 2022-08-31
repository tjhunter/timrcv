mod config;
use log::{debug, info, warn};
// use nanoserde::DeJson;

use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    ops::AddAssign,
};

pub use crate::config::*;

// Public structures

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Vote {
    pub candidates: Vec<String>,
    pub count: u64,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum VotingResult {
    SingleWinner(String, ResultStats),
    NoMajorityCandidate,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum VotingErrors {
    EmptyElection,
}

// **** Private structures ****

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
struct CandidateId(u32);

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
struct RankedChoice {
    first: CandidateId,
    rest: Vec<CandidateId>,
}

impl RankedChoice {
    /// Removes all the eliminated candidates from the list of choices.
    fn filtered_candidate(self: &Self, eliminated: &HashSet<CandidateId>) -> Option<RankedChoice> {
        let mut choices = vec![self.first];
        choices.extend(self.rest.clone());
        let rem_choices: Vec<CandidateId> = choices
            .iter()
            .filter(|cid| !eliminated.contains(cid))
            .cloned()
            .collect();
        match &rem_choices[..] {
            [] => None,
            [first, rest @ ..] => Some(RankedChoice {
                first: first.clone(),
                rest: rest.iter().cloned().collect(),
            }),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, PartialOrd, Ord, Hash)]
struct VoteCount(u64);

impl VoteCount {
    const EMPTY: VoteCount = VoteCount(0);
}

impl std::iter::Sum for VoteCount {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        VoteCount(iter.map(|vc| vc.0).sum())
    }
}

impl AddAssign for VoteCount {
    fn add_assign(&mut self, rhs: VoteCount) {
        self.0 += rhs.0;
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
struct VoteInternal {
    candidates: RankedChoice,
    count: VoteCount,
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
struct VoteSignature {
    // Guaranteed to never be empty at construction.
    ranks: Vec<CandidateId>,
    // Guaranteed to never be zero at construction
    count: VoteCount,
}

impl VoteSignature {}

/// Runs the voting algorithm with the given rules for the given votes.
///
/// Arguments:
/// * `coll` the collection of votes to process
/// * `rules` the rules that govern this election
/// * `candidates` the registered candidates for this election. If not provided, the
/// candidates will be inferred from the votes.
pub fn run_voting_stats(
    coll: &Vec<Vote>,
    rules: &config::VoteRules,
    candidates: &Option<Vec<config::Candidate>>,
) -> Result<VotingResult, VotingErrors> {
    info!(
        "Processing {:?} votes, candidates: {:?}, rules: {:?}",
        coll.iter().count(),
        candidates,
        rules
    );

    let (stats, all_candidates) = checks(coll, candidates)?;
    info!(
        "Processing {:?} aggregated votes, candidates: {:?}",
        stats.iter().count(),
        all_candidates
    );
    let candidates_by_id: HashMap<CandidateId, String> = all_candidates
        .iter()
        .map(|(cname, cid)| (cid.clone(), cname.clone()))
        .collect();
    let mut cur_votes: Vec<VoteInternal> = stats.clone();
    let mut cur_stats: Vec<config::RoundStats> = Vec::new();

    while cur_votes.clone().iter().count() > 0 {
        let round_id = cur_stats.iter().count() + 1;
        match &cur_votes[..] {
            [] => return Err(VotingErrors::EmptyElection),
            [va] => {
                let name = match candidates_by_id.get(&va.candidates.first) {
                    None => unimplemented!(""),
                    Some(cname) => cname.clone(),
                };
                return Ok(VotingResult::SingleWinner(
                    name,
                    config::ResultStats { rounds: cur_stats },
                ));
            }
            _ => {}
        }
        let (next_votes, _, round_stats) = run_one_round(&cur_votes, &candidates_by_id)?;
        info!("Round id: {:?} stats: {:?}", round_id, round_stats);
        cur_votes = next_votes;
        cur_stats.push(round_stats);
    }
    unimplemented!("");
}

/// Returns the removed candidates, and the remaining votes
fn run_one_round(
    votes: &Vec<VoteInternal>,
    candidate_ids: &HashMap<CandidateId, String>, // Needed for the stats. just return everything with candidate ids from this function
) -> Result<(Vec<VoteInternal>, Vec<CandidateId>, config::RoundStats), VotingErrors> {
    let by_groups = votes.iter().fold(HashMap::new(), |mut acc, va| {
        *acc.entry(va.candidates.first).or_insert(VoteCount(0)) += va.count;
        acc
    });
    match by_groups.values().min() {
        None => {
            // TODO: no min values => empty? it should not happen, really
            return Ok((
                Vec::new(),
                Vec::new(),
                config::RoundStats { tally: Vec::new() },
            ));
        }
        Some(min_count) => {
            let all_smallest: Vec<CandidateId> = by_groups
                .iter()
                .filter_map(|(cid, vc)| if *vc <= *min_count { Some(cid) } else { None })
                .cloned()
                .collect();
            debug!("all_smallest: {:?}", all_smallest);
            assert!(all_smallest.iter().count() > 0);

            // TODO the strategy to pick the candidates to eliminate.
            // For now, it is simply all the candidates with the smallest number of votes

            let eliminated_candidates: HashSet<CandidateId> =
                all_smallest.iter().cloned().collect();

            // Statistics about transfers:
            // For every eliminated candidates, keep the vote transfer, or the exhausted vote.
            let mut elimination_stats: HashMap<
                CandidateId,
                (HashMap<CandidateId, VoteCount>, VoteCount),
            > = HashMap::new();

            // Filter the rest of the votes to simply keep the votes that still matter
            let rem_votes: Vec<VoteInternal> = votes
                .iter()
                .filter_map(|va| {
                    // Remove the choices that are not valid anymore and collect statistics.
                    let new_rank = va.candidates.filtered_candidate(&eliminated_candidates);
                    let new_first = new_rank.clone().map(|nr| nr.first);
                    let first = va.candidates.first;

                    match new_first {
                        None => {
                            // The vote has been exhausted
                            let e = elimination_stats
                                .entry(first)
                                .or_insert((HashMap::new(), VoteCount::EMPTY));
                            e.1 += va.count;
                        }
                        Some(cid) if eliminated_candidates.contains(&first) => {
                            // The vote has been transfered. Record the transfer.
                            let e = elimination_stats
                                .entry(first)
                                .or_insert((HashMap::new(), VoteCount::EMPTY));
                            let e2 = e.0.entry(cid).or_insert(VoteCount::EMPTY);
                            *e2 += va.count;
                        }
                        _ => {
                            // Nothing to do, the first choice has not changed.
                        }
                    }

                    new_rank.map(|rc| VoteInternal {
                        candidates: rc,
                        count: va.count,
                    })
                })
                .collect();

            let stats = RoundStats {
                tally: elimination_stats
                    .iter()
                    .map(|(cid, (reports, exhausted))| {
                        let name = candidate_ids.get(cid).unwrap();
                        // The elimination stats
                        let eli_stats: Vec<(String, u64)> = reports
                            .iter()
                            .map(|(report_cid, report_count)| {
                                (
                                    candidate_ids.get(report_cid).unwrap().clone(),
                                    report_count.0,
                                )
                            })
                            .collect();
                        let part_tally: u64 = eli_stats.iter().map(|(_, c)| c.clone()).sum();
                        let tally: u64 = part_tally + exhausted.0;
                        RoundCandidateStats {
                            name: name.clone(),
                            tally: tally,
                            status: RoundCandidateStatus::Eliminated(eli_stats, exhausted.0),
                        }
                    })
                    .collect(),
            };
            debug!("count votes: {:?}", rem_votes.iter().count());

            Ok((rem_votes, all_smallest, stats))
        }
    }
}

fn checks(
    coll: &Vec<Vote>,
    reg_candidates: &Option<Vec<config::Candidate>>,
) -> Result<(Vec<VoteInternal>, HashMap<String, CandidateId>), VotingErrors> {
    debug!("checks: coll size: {:?}", coll.iter().count());
    let blacklisted_candidates: HashSet<String> = reg_candidates
        .clone()
        .unwrap_or(Vec::new())
        .iter()
        .filter_map(|c| {
            if c.excluded {
                Some(c.name.clone())
            } else {
                None
            }
        })
        .collect();
    let mut candidates: HashMap<String, CandidateId> = HashMap::new();
    let mut counter: u32 = 0;
    let vas: Vec<VoteInternal> = coll
        .iter()
        .map(|v| {
            let cs: Vec<CandidateId> = v
                .candidates
                .iter()
                .filter_map(|c| {
                    if blacklisted_candidates.contains(c) {
                        None
                    } else {
                        let cid: CandidateId = candidates
                            .entry(c.clone())
                            .or_insert({
                                counter += 1;
                                CandidateId(counter)
                            })
                            .clone();
                        Some(cid)
                    }
                })
                .collect();
            let randked_choice = match &cs[..] {
                [first, rest @ ..] => RankedChoice {
                    first: first.clone(),
                    rest: rest.iter().cloned().collect(),
                },
                _ => {
                    unimplemented!("bad vote. not implemented {:?}", v);
                }
            };
            VoteInternal {
                count: VoteCount(v.count),
                candidates: randked_choice,
            }
            // unimplemented!("checks");
        })
        .collect();
    debug!(
        "checks: vote aggs size: {:?}  candidates: {:?}",
        vas.iter().count(),
        candidates.iter().count()
    );
    Ok((vas, candidates))
}

pub fn add_one(x: i32) -> i32 {
    let data = vec![
        Vote {
            candidates: vec!["a".to_string(), "b".to_string()],
            count: 1,
        },
        Vote {
            candidates: vec!["x".to_string(), "b".to_string()],
            count: 2,
        },
    ];

    info!("data {:?}", data);

    let rules = config::VoteRules {
        tiebreak_mode: config::TieBreakMode::UseCandidateOrder,
        winner_election_mode: config::WinnerElectionMode::SingelWinnerMajority,
        number_of_winners: 1,
        minimum_vote_threshold: None,
        max_rankings_allowed: None,
    };

    let res = run_voting_stats(&data, &rules, &None);

    info!("res {:?}", res);

    info!("info arg {:?}", x);
    warn!("info arg {:?}", x);
    x + 1
}
