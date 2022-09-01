mod config;
use log::{debug, info};

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
pub struct VotingResult {
    // TODO: replace by an enumeration: SingleWinner, MultiWinner, NoWinner
    pub winners: Option<Vec<String>>,
    pub round_stats: Vec<RoundStats>,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum VotingErrors {
    EmptyElection,
    NoConvergence,
}

// **** Private structures ****

type RoundId = u32;

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
struct CandidateId(u32);

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
struct RankedChoice {
    first: CandidateId,
    rest: Vec<CandidateId>,
}

impl RankedChoice {
    /// Removes all the eliminated candidates from the list of choices.
    /// Takes into account the policy for duplicated candidates. If the head candidates appears multiple
    /// time under the exhaust policy, this ballot will be exhausted.
    fn filtered_candidate(
        self: &Self,
        eliminated: &HashSet<CandidateId>,
        duplicate_policy: DuplicateCandidateMode,
    ) -> Option<RankedChoice> {
        let mut choices = vec![self.first];
        choices.extend(self.rest.clone());
        // See if the current top candidate is present multiple time.
        if duplicate_policy == DuplicateCandidateMode::Exhaust
            && self.rest.iter().any(|&cid| cid == self.first)
        {
            return None;
        }
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

#[derive(Eq, PartialEq, Debug, Clone)]
enum RoundCandidateStatusInternal {
    StillRunning,
    Elected,
    /// if eliminated, the transfers of the votes to each candidate
    /// the last element is the number of exhausted votes
    Eliminated(Vec<(CandidateId, VoteCount)>, VoteCount),
}

#[derive(Eq, PartialEq, Debug, Clone)]
struct RoundResult {
    votes: Vec<VoteInternal>,
    stats: Vec<(CandidateId, VoteCount, RoundCandidateStatusInternal)>,
}

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
    let mut cur_votes: Vec<VoteInternal> = stats.clone();
    let mut cur_stats: Vec<Vec<(CandidateId, VoteCount, RoundCandidateStatusInternal)>> =
        Vec::new();

    // TODO: better management of the number of iterations
    while cur_stats.iter().len() < 10000 {
        let round_id = cur_stats.iter().len() + 1;
        let round_res = run_one_round(&cur_votes, &rules)?;
        let stats = round_res.stats.clone();
        info!("Round id: {:?} stats: {:?}", round_id, round_res.stats);
        cur_votes = round_res.votes;
        cur_stats.push(round_res.stats);

        // Check end. For now, simply check that we have a winner.
        // TODO check that everyone is a winner or eliminated.

        assert!(stats.clone().len() > 0);
        let winners: Vec<CandidateId> = stats
            .iter()
            .filter_map(|(cid, _, s)| match s {
                RoundCandidateStatusInternal::Elected => Some(*cid),
                _ => None,
            })
            .collect();
        if !winners.is_empty() {
            // We are done, stop here.
            let candidates_by_id: HashMap<CandidateId, String> = all_candidates
                .iter()
                .map(|(cname, cid)| (cid.clone(), cname.clone()))
                .collect();
            let stats = round_results_to_stats(&cur_stats, &candidates_by_id)?;
            let mut winner_names: Vec<String> = Vec::new();
            for cid in winners {
                winner_names.push(candidates_by_id.get(&cid).unwrap().clone());
            }
            return Ok(VotingResult {
                winners: Some(winner_names),
                round_stats: stats,
            });
        }
    }
    Err(VotingErrors::NoConvergence)
}

fn round_results_to_stats(
    results: &Vec<Vec<(CandidateId, VoteCount, RoundCandidateStatusInternal)>>,
    candidates_by_id: &HashMap<CandidateId, String>,
) -> Result<Vec<RoundStats>, VotingErrors> {
    let mut res: Vec<RoundStats> = Vec::new();
    for (idx, r) in results.iter().enumerate() {
        let round_id: RoundId = idx as u32 + 1;
        res.push(round_result_to_stat(r, round_id, candidates_by_id)?);
    }
    Ok(res)
}

fn round_result_to_stat(
    stats: &Vec<(CandidateId, VoteCount, RoundCandidateStatusInternal)>,
    round_id: RoundId,
    candidates_by_id: &HashMap<CandidateId, String>,
) -> Result<RoundStats, VotingErrors> {
    let mut rs = config::RoundStats {
        round: round_id,
        tally: Vec::new(),
        tally_results_elected: Vec::new(),
        tally_result_eliminated: Vec::new(),
    };

    for (cid, c, status) in stats.clone() {
        let name: &String = candidates_by_id
            .get(&cid)
            .ok_or_else(|| (VotingErrors::EmptyElection))?; // TODO: wrong error
        rs.tally.push((name.clone(), c.0));
        match status {
            RoundCandidateStatusInternal::StillRunning => {
                // Nothing to say about this candidate
            }
            RoundCandidateStatusInternal::Elected => {
                rs.tally_results_elected.push(name.clone());
            }
            RoundCandidateStatusInternal::Eliminated(transfers, exhausts) => {
                let mut pub_transfers: Vec<(String, u64)> = Vec::new();
                for (t_cid, t_count) in transfers {
                    let t_name: &String = candidates_by_id
                        .get(&t_cid)
                        .ok_or_else(|| (VotingErrors::EmptyElection))?; // TODO: wrong error
                    pub_transfers.push((t_name.clone(), t_count.0));
                }
                rs.tally_result_eliminated.push(config::EliminationStats {
                    name: name.clone(),
                    transfers: pub_transfers,
                    exhausted: exhausts.0,
                });
            }
        }
    }
    Ok(rs)
}

/// Returns the removed candidates, and the remaining votes
fn run_one_round(
    votes: &Vec<VoteInternal>,
    rules: &config::VoteRules,
) -> Result<RoundResult, VotingErrors> {
    let tally: HashMap<CandidateId, VoteCount> =
        votes.iter().fold(HashMap::new(), |mut acc, va| {
            *acc.entry(va.candidates.first).or_insert(VoteCount(0)) += va.count;
            acc
        });

    debug!("tally: {:?}", tally);

    let min_count: VoteCount = {
        match tally.values().min() {
            None => {
                // TODO: no min values => empty? it should not happen, really
                return Ok(RoundResult {
                    votes: Vec::new(),
                    stats: Vec::new(),
                });
            }
            Some(c) => *c,
        }
    };

    let all_smallest: Vec<CandidateId> = tally
        .iter()
        .filter_map(|(cid, vc)| if *vc <= min_count { Some(cid) } else { None })
        .cloned()
        .collect();
    debug!("all_smallest: {:?}", all_smallest);
    assert!(all_smallest.iter().count() > 0);

    // TODO strategy to pick the winning candidates

    // TODO the strategy to pick the candidates to eliminate.
    // For now, it is simply all the candidates with the smallest number of votes

    let eliminated_candidates: HashSet<CandidateId> = all_smallest.iter().cloned().collect();

    // Statistics about transfers:
    // For every eliminated candidates, keep the vote transfer, or the exhausted vote.
    let mut elimination_stats: HashMap<CandidateId, (HashMap<CandidateId, VoteCount>, VoteCount)> =
        eliminated_candidates
            .iter()
            .map(|cid| (cid.clone(), (HashMap::new(), VoteCount::EMPTY)))
            .collect();

    // Filter the rest of the votes to simply keep the votes that still matter
    let rem_votes: Vec<VoteInternal> = votes
        .iter()
        .filter_map(|va| {
            // Remove the choices that are not valid anymore and collect statistics.
            let new_rank = va
                .candidates
                .filtered_candidate(&eliminated_candidates, rules.duplicate_candidate_mode);
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
                Some(_) => {
                    // Nothing to do
                }
            }

            new_rank.map(|rc| VoteInternal {
                candidates: rc,
                count: va.count,
            })
        })
        .collect();

    // Check if some candidates are winners.
    // Right now, it is simply if one candidate is left.
    let remainers: HashMap<CandidateId, VoteCount> = tally
        .iter()
        .filter_map(|(cid, vc)| {
            if eliminated_candidates.contains(cid) {
                None
            } else {
                Some((cid.clone(), vc.clone()))
            }
        })
        .collect();
    let mut winners: HashSet<CandidateId> = HashSet::new();
    if remainers.len() == 1 {
        for cid in remainers.keys() {
            winners.insert(*cid);
        }
    }

    let mut round_stats: Vec<(CandidateId, VoteCount, RoundCandidateStatusInternal)> = Vec::new();
    for (&cid, &count) in tally.iter() {
        if let Some((transfers, exhaust)) = elimination_stats.get(&cid) {
            round_stats.push((
                cid,
                count,
                RoundCandidateStatusInternal::Eliminated(
                    transfers.iter().map(|(cid2, c2)| (*cid2, *c2)).collect(),
                    *exhaust,
                ),
            ))
        } else if winners.contains(&cid) {
            round_stats.push((cid, count, RoundCandidateStatusInternal::Elected));
        } else {
            // Not eliminated, still running
            round_stats.push((cid, count, RoundCandidateStatusInternal::StillRunning));
        }
    }

    return Ok(RoundResult {
        votes: rem_votes,
        stats: round_stats,
    });
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
                            .or_insert_with(|| {
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
