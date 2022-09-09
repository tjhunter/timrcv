mod config;
use log::{debug, info};

use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    ops::{Add, AddAssign},
};

pub use crate::config::*;

// **** Private structures ****

type RoundId = u32;

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Ord, PartialOrd)]
struct CandidateId(u32);

// A position in a ballot may not be filled with a candidate name, and this may still be acceptable.
// It simply means that this ballot will not be account for this turn.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Ord, PartialOrd)]
enum Choice {
    Blank,
    Undervote,
    Overvote,
    Undeclared,
    Filled(CandidateId),
}

// Invariant: there is at least one CandidateId in all the choices.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
struct RankedChoice {
    first_valid: CandidateId,
    rest: Vec<Choice>,
}

impl RankedChoice {
    /// Removes all the eliminated candidates from the list of choices.
    /// Takes into account the policy for duplicated candidates. If the head candidates appears multiple
    /// time under the exhaust policy, this ballot will be exhausted.
    fn filtered_candidate(
        &self,
        still_valid: &HashSet<CandidateId>,
        duplicate_policy: DuplicateCandidateMode,
        overvote: OverVoteRule,
        skipped_ranks: MaxSkippedRank,
    ) -> Option<RankedChoice> {
        // If the top candidate did not get eliminated, keep the current ranked choice.
        if still_valid.contains(&self.first_valid) {
            return Some(self.clone());
        }

        // Run the choice pruning procedure.
        // Add again the first choice since it may have an impact on the elimination rules.
        let mut all_choices = vec![Choice::Filled(self.first_valid)];
        all_choices.extend(self.rest.clone());

        if let Some((first_valid, rest)) = advance_voting(
            &all_choices,
            still_valid,
            duplicate_policy,
            overvote,
            skipped_ranks,
        ) {
            Some(RankedChoice { first_valid, rest })
        } else {
            None
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

impl Add for VoteCount {
    type Output = VoteCount;
    fn add(self: VoteCount, rhs: VoteCount) -> VoteCount {
        VoteCount(self.0 + rhs.0)
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

// TODO: rename InternalRoundStatistics
#[derive(Eq, PartialEq, Debug, Clone)]
struct RoundStatistics {
    candidate_stats: Vec<(CandidateId, VoteCount, RoundCandidateStatusInternal)>,
    uwi_elimination_stats: Option<(Vec<(CandidateId, VoteCount)>, VoteCount)>,
}

#[derive(Eq, PartialEq, Debug, Clone)]
struct RoundResult {
    votes: Vec<VoteInternal>,
    stats: RoundStatistics,
    // Winning vote threshold
    vote_threshold: VoteCount,
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
        coll.len(),
        candidates,
        rules
    );

    // TODO: ensure candidates
    let cr: CheckResult = checks(coll, &candidates.clone().unwrap(), rules)?;
    let checked_votes = cr.votes;
    debug!(
        "run_voting_stats: Checked votes: {:?}, detected UWIs {:?}",
        checked_votes.len(),
        cr.count_exhausted_uwi_first_round
    );
    let all_candidates = cr.candidates;
    {
        info!("Processing {:?} aggregated votes", checked_votes.len());
        let mut sorted_candidates: Vec<&(String, CandidateId)> = all_candidates.iter().collect();
        sorted_candidates.sort_by_key(|p| p.1);
        for p in sorted_candidates.iter() {
            info!("Candidate: {}: {}", p.1 .0, p.0);
        }
    }

    let mut initial_count: VoteCount = VoteCount::EMPTY;
    for v in checked_votes.iter() {
        initial_count += v.count;
    }

    // We are done, stop here.
    let candidates_by_id: HashMap<CandidateId, String> = all_candidates
        .iter()
        .map(|(cname, cid)| (*cid, cname.clone()))
        .collect();

    // The candidates that are still running, in sorted order as defined by input.
    let mut cur_sorted_candidates: Vec<(String, CandidateId)> = all_candidates;
    let mut cur_votes: Vec<VoteInternal> = checked_votes;
    let mut cur_stats: Vec<RoundStatistics> = Vec::new();

    // TODO: better management of the number of iterations
    while cur_stats.iter().len() < 10000 {
        let round_id = (cur_stats.iter().len() + 1) as u32;
        info!(
            "Round id: {:?} cur_candidates: {:?}",
            round_id, cur_sorted_candidates
        );
        let has_initial_uwis = cur_stats.is_empty()
            && (!cr.uwi_first_votes.is_empty()
                || cr.count_exhausted_uwi_first_round > VoteCount::EMPTY);
        let round_res: RoundResult = if has_initial_uwis {
            // First round and we have some undeclared write ins.
            // Apply a special path to get rid of them.
            run_first_round_uwi(
                &cur_votes,
                &cr.uwi_first_votes,
                cr.count_exhausted_uwi_first_round,
                &cur_sorted_candidates,
            )?
        } else {
            run_one_round(&cur_votes, rules, &cur_sorted_candidates, round_id)?
        };
        let round_stats = round_res.stats.clone();
        info!("Round id: {:?} stats: {:?}", round_id, round_stats);

        cur_votes = round_res.votes;
        cur_stats.push(round_res.stats);
        let stats = round_stats.candidate_stats;

        // Survivors are described in candidate order.
        let mut survivors: Vec<(String, CandidateId)> = Vec::new();
        for (s, cid) in cur_sorted_candidates.iter() {
            // Has this candidate been marked as eliminated? Skip it
            let is_eliminated = stats.iter().any(|(cid2, _, s)| {
                matches!(s, RoundCandidateStatusInternal::Eliminated(_, _) if *cid == *cid2)
            });
            if !is_eliminated {
                survivors.push((s.clone(), *cid));
            }
        }
        // Invariant: the number of candidates decreased or all the candidates are winners
        let all_survivors_winners = stats
            .iter()
            .all(|(_, _, s)| matches!(s, RoundCandidateStatusInternal::Elected));
        if !has_initial_uwis {
            assert!(
                all_survivors_winners || (survivors.len() < cur_sorted_candidates.len()),
                "The number of candidates did not decrease: {:?} -> {:?}",
                cur_sorted_candidates,
                survivors
            );
        }
        cur_sorted_candidates = survivors;

        // Check end. For now, simply check that we have a winner.
        // TODO check that everyone is a winner or eliminated.

        assert!(!stats.is_empty());
        let winners: Vec<CandidateId> = stats
            .iter()
            .filter_map(|(cid, _, s)| match s {
                RoundCandidateStatusInternal::Elected => Some(*cid),
                _ => None,
            })
            .collect();
        if !winners.is_empty() {
            let stats = round_results_to_stats(&cur_stats, &candidates_by_id)?;
            let mut winner_names: Vec<String> = Vec::new();
            for cid in winners {
                winner_names.push(candidates_by_id.get(&cid).unwrap().clone());
            }
            return Ok(VotingResult {
                threshold: round_res.vote_threshold.0,
                winners: Some(winner_names),
                round_stats: stats,
            });
        }
    }
    Err(VotingErrors::NoConvergence)
}

fn get_threshold(tally: &HashMap<CandidateId, VoteCount>) -> VoteCount {
    let total_count: VoteCount = tally.values().cloned().sum();
    if total_count == VoteCount::EMPTY {
        VoteCount::EMPTY
    } else {
        // TODO: this is hardcoding the formula for num_winners = 1, implement the other ones.
        VoteCount((total_count.0 / 2) + 1)
    }
}

fn round_results_to_stats(
    results: &[RoundStatistics],
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
    stats: &RoundStatistics,
    round_id: RoundId,
    candidates_by_id: &HashMap<CandidateId, String>,
) -> Result<RoundStats, VotingErrors> {
    let mut rs = config::RoundStats {
        round: round_id,
        tally: Vec::new(),
        tally_results_elected: Vec::new(),
        tally_result_eliminated: Vec::new(),
    };

    for (cid, c, status) in stats.candidate_stats.iter() {
        let name: &String = candidates_by_id
            .get(cid)
            .ok_or(VotingErrors::EmptyElection)?; // TODO: wrong error
        rs.tally.push((name.clone(), c.0));
        match status {
            RoundCandidateStatusInternal::StillRunning => {
                // Nothing to say about this candidate
            }
            RoundCandidateStatusInternal::Elected => {
                rs.tally_results_elected.push(name.clone());
            }
            RoundCandidateStatusInternal::Eliminated(transfers, exhausts)
                if (!transfers.is_empty()) || *exhausts > VoteCount::EMPTY =>
            {
                let mut pub_transfers: Vec<(String, u64)> = Vec::new();
                for (t_cid, t_count) in transfers {
                    let t_name: &String = candidates_by_id
                        .get(t_cid)
                        .ok_or(VotingErrors::EmptyElection)?; // TODO: wrong error
                    pub_transfers.push((t_name.clone(), t_count.0));
                }
                rs.tally_result_eliminated.push(config::EliminationStats {
                    name: name.clone(),
                    transfers: pub_transfers,
                    exhausted: exhausts.0,
                });
            }
            RoundCandidateStatusInternal::Eliminated(_, _) => {
                // Do not print a candidate if its corresponding stats are going to be empty.
            }
        }
    }

    let uwi = "Undeclared Write-ins".to_string();

    if let Some((uwi_transfers, uwi_exhauster)) = stats.uwi_elimination_stats.clone() {
        let uwi_tally: VoteCount =
            uwi_transfers.iter().map(|(_, vc)| *vc).sum::<VoteCount>() + uwi_exhauster;
        if uwi_tally > VoteCount::EMPTY {
            rs.tally.push((uwi.clone(), uwi_tally.0));
        }
        let mut pub_transfers: Vec<(String, u64)> = Vec::new();
        for (t_cid, t_count) in uwi_transfers.iter() {
            let t_name: &String = candidates_by_id
                .get(t_cid)
                .ok_or(VotingErrors::EmptyElection)?; // TODO: wrong error
            pub_transfers.push((t_name.clone(), t_count.0));
        }

        rs.tally_result_eliminated.push(EliminationStats {
            name: uwi,
            transfers: pub_transfers,
            exhausted: uwi_exhauster.0,
        });
    }

    rs.tally_result_eliminated.sort_by_key(|es| es.name.clone());
    rs.tally_results_elected.sort();
    Ok(rs)
}

fn run_first_round_uwi(
    votes: &[VoteInternal],
    uwi_first_votes: &[VoteInternal],
    uwi_first_exhausted: VoteCount,
    candidate_names: &[(String, CandidateId)],
) -> Result<RoundResult, VotingErrors> {
    let tally = compute_tally(votes, candidate_names);
    let mut elimination_stats: HashMap<CandidateId, VoteCount> = HashMap::new();
    for v in uwi_first_votes.iter() {
        let e = elimination_stats
            .entry(v.candidates.first_valid)
            .or_insert(VoteCount::EMPTY);
        *e += v.count;
    }

    let full_stats = RoundStatistics {
        candidate_stats: tally
            .iter()
            .map(|(cid, vc)| (*cid, *vc, RoundCandidateStatusInternal::StillRunning))
            .collect(),
        uwi_elimination_stats: Some((
            elimination_stats
                .iter()
                .map(|(cid, vc)| (*cid, *vc))
                .collect(),
            uwi_first_exhausted,
        )),
    };

    let mut all_votes = votes.to_vec();
    all_votes.extend(uwi_first_votes.to_vec());

    Ok(RoundResult {
        votes: all_votes,
        stats: full_stats,
        vote_threshold: VoteCount::EMPTY,
    })
}

fn compute_tally(
    votes: &[VoteInternal],
    candidate_names: &[(String, CandidateId)],
) -> HashMap<CandidateId, VoteCount> {
    let mut tally: HashMap<CandidateId, VoteCount> = HashMap::new();
    for (_, cid) in candidate_names.iter() {
        tally.insert(*cid, VoteCount::EMPTY);
    }
    for v in votes.iter() {
        if let Some(vc) = tally.get_mut(&v.candidates.first_valid) {
            *vc += v.count;
        }
    }
    tally
}

/// Returns the removed candidates, and the remaining votes
fn run_one_round(
    votes: &[VoteInternal],
    rules: &config::VoteRules,
    candidate_names: &[(String, CandidateId)],
    num_round: u32,
) -> Result<RoundResult, VotingErrors> {
    // Initialize the tally with the current candidate names to capture all the candidates who do
    // not even have a vote.
    let tally = compute_tally(votes, candidate_names);
    debug!("tally: {:?}", tally);

    let vote_threshold = get_threshold(&tally);
    debug!("run_one_round: vote_threshold: {:?}", vote_threshold);

    // Only one candidate. It is the winner by any standard.
    // TODO: improve with multi candidate modes.
    if tally.len() == 1 {
        debug!(
            "run_one_round: only one candidate, directly winning: {:?}",
            tally
        );
        let stats = RoundStatistics {
            candidate_stats: tally
                .iter()
                .map(|(cid, count)| (*cid, *count, RoundCandidateStatusInternal::Elected))
                .collect(),
            uwi_elimination_stats: Some((vec![], VoteCount::EMPTY)),
        };
        return Ok(RoundResult {
            votes: votes.to_vec(),
            stats,
            vote_threshold,
        });
    }

    // Find the candidates to eliminate
    let p = find_eliminated_candidates(&tally, rules, candidate_names, num_round);
    let resolved_tiebreak: TiebreakSituation = p.1;
    let eliminated_candidates: HashSet<CandidateId> = p.0.iter().cloned().collect();

    // TODO strategy to pick the winning candidates

    assert!(!eliminated_candidates.is_empty(), "No candidate eliminated");
    debug!("run_one_round: tiebreak situation: {:?}", resolved_tiebreak);
    debug!("run_one_round: eliminated_candidates: {:?}", p.0);

    // Statistics about transfers:
    // For every eliminated candidates, keep the vote transfer, or the exhausted vote.
    let mut elimination_stats: HashMap<CandidateId, (HashMap<CandidateId, VoteCount>, VoteCount)> =
        eliminated_candidates
            .iter()
            .map(|cid| (*cid, (HashMap::new(), VoteCount::EMPTY)))
            .collect();

    let remaining_candidates: HashSet<CandidateId> = candidate_names
        .iter()
        .filter_map(|p| match p {
            (_, cid) if !eliminated_candidates.contains(cid) => Some(*cid),
            _ => None,
        })
        .collect();

    // Filter the rest of the votes to simply keep the votes that still matter
    let rem_votes: Vec<VoteInternal> = votes
        .iter()
        .filter_map(|va| {
            // Remove the choices that are not valid anymore and collect statistics.
            let new_rank = va.candidates.filtered_candidate(
                &remaining_candidates,
                rules.duplicate_candidate_mode,
                rules.overvote_rule,
                rules.max_skipped_rank_allowed,
            );
            let old_first = va.candidates.first_valid;
            let new_first = new_rank.clone().map(|nr| nr.first_valid);

            match new_first {
                None => {
                    // Ballot is now exhausted. Record the exhausted vote.
                    let e = elimination_stats
                        .entry(old_first)
                        .or_insert((HashMap::new(), VoteCount::EMPTY));
                    e.1 += va.count;
                }
                Some(new_first_cid) if new_first_cid != old_first => {
                    // The ballot has been transfered. Record the transfer.
                    let e = elimination_stats
                        .entry(old_first)
                        .or_insert((HashMap::new(), VoteCount::EMPTY));
                    let e2 = e.0.entry(new_first_cid).or_insert(VoteCount::EMPTY);
                    *e2 += va.count;
                }
                _ => {
                    // Nothing to do, the first choice is the same.
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
                Some((*cid, *vc))
            }
        })
        .collect();

    debug!("run_one_round: remainers: {:?}", remainers);
    let mut winners: HashSet<CandidateId> = HashSet::new();
    // If a tiebreak was resolved in this round, do not select a winner.
    // This is just an artifact of the reference implementation.
    if resolved_tiebreak == TiebreakSituation::Clean {
        for (&cid, &count) in remainers.iter() {
            if count >= vote_threshold {
                debug!(
                    "run_one_round: {:?} has count {:?}, marking as winner",
                    cid, count
                );
                winners.insert(cid);
            }
        }
    }

    let mut candidate_stats: Vec<(CandidateId, VoteCount, RoundCandidateStatusInternal)> =
        Vec::new();
    for (&cid, &count) in tally.iter() {
        if let Some((transfers, exhaust)) = elimination_stats.get(&cid) {
            candidate_stats.push((
                cid,
                count,
                RoundCandidateStatusInternal::Eliminated(
                    transfers.iter().map(|(cid2, c2)| (*cid2, *c2)).collect(),
                    *exhaust,
                ),
            ))
        } else if winners.contains(&cid) {
            candidate_stats.push((cid, count, RoundCandidateStatusInternal::Elected));
        } else {
            // Not eliminated, still running
            candidate_stats.push((cid, count, RoundCandidateStatusInternal::StillRunning));
        }
    }

    Ok(RoundResult {
        votes: rem_votes,
        stats: RoundStatistics {
            candidate_stats,
            uwi_elimination_stats: None,
        },
        vote_threshold,
    })
}

fn find_eliminated_candidates(
    tally: &HashMap<CandidateId, VoteCount>,
    rules: &config::VoteRules,
    candidate_names: &[(String, CandidateId)],
    num_round: u32,
) -> (Vec<CandidateId>, TiebreakSituation) {
    // Try to eliminate candidates in batch
    if rules.elimination_algorithm == EliminationAlgorithm::Batch {
        if let Some(v) = find_eliminated_candidates_batch(tally) {
            return (v, TiebreakSituation::Clean);
        }
    }

    if let Some((v, tb)) =
        find_eliminated_candidates_single(tally, rules.tiebreak_mode, candidate_names, num_round)
    {
        return (v, tb);
    }
    // No candidate to eliminate.
    // TODO check the conditions for this to happen.
    unimplemented!("find_eliminated_candidates: No candidate to eliminate");
}

fn find_eliminated_candidates_batch(
    tally: &HashMap<CandidateId, VoteCount>,
) -> Option<Vec<CandidateId>> {
    // Sort the candidates in increasing tally.
    let mut sorted_tally: Vec<(CandidateId, VoteCount)> =
        tally.iter().map(|(&cid, &vc)| (cid, vc)).collect();
    sorted_tally.sort_by_key(|(_, vc)| *vc);

    // the vote count for this candidate and the cumulative count (excluding the current one)
    let mut sorted_tally_cum: Vec<(CandidateId, VoteCount, VoteCount)> = Vec::new();
    let mut curr_count = VoteCount::EMPTY;
    for (cid, cur_vc) in sorted_tally.iter() {
        sorted_tally_cum.push((*cid, *cur_vc, curr_count));
        curr_count += *cur_vc;
    }
    debug!(
        "find_eliminated_candidates_batch: sorted_tally_cum: {:?}",
        sorted_tally_cum
    );

    // Find the largest index for which the previous cumulative count is strictly lower than the current vote count.
    // Anything below will not be able to transfer higher.

    let large_gap_idx = sorted_tally_cum
        .iter()
        .enumerate()
        .filter(|(_, (_, cur_vc, previous_cum_count))| previous_cum_count < cur_vc)
        .last();

    // The idx == 0 element is not relevant because the previous cumulative count was zero.
    if let Some((idx, _)) = large_gap_idx {
        if idx > 0 {
            let res = sorted_tally.iter().map(|(cid, _)| *cid).take(idx).collect();
            debug!(
                "find_eliminated_candidates_batch: found a batch to eliminate: {:?}",
                res
            );
            return Some(res);
        }
    }
    debug!("find_eliminated_candidates_batch: no candidates to eliminate");
    None
}

// Flag to indicate if a tiebreak happened.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
enum TiebreakSituation {
    Clean,           // Did not happen
    TiebreakOccured, // Happened and had to be resolved.
}

// Elimination method for single candidates.
fn find_eliminated_candidates_single(
    tally: &HashMap<CandidateId, VoteCount>,
    tiebreak: TieBreakMode,
    candidate_names: &[(String, CandidateId)],
    num_round: u32,
) -> Option<(Vec<CandidateId>, TiebreakSituation)> {
    // TODO should be a programming error
    if tally.is_empty() {
        return None;
    }

    // Only one candidate left, it is the winner by default.
    // No need to eliminate candidates.
    if tally.len() == 1 {
        debug!(
            "find_eliminated_candidates_single: Only one candidate left in tally, no one to eliminate: {:?}",
            tally
        );
        return None;
    }
    assert!(tally.len() >= 2);

    let min_count: VoteCount = *tally.values().min().unwrap();

    let all_smallest: Vec<CandidateId> = tally
        .iter()
        .filter_map(|(cid, vc)| if *vc <= min_count { Some(cid) } else { None })
        .cloned()
        .collect();
    debug!(
        "find_eliminated_candidates_single: all_smallest: {:?}",
        all_smallest
    );
    assert!(!all_smallest.is_empty());

    // No tiebreak, the logic below is not relevant.
    if all_smallest.len() == 1 {
        return Some((all_smallest, TiebreakSituation::Clean));
    }

    // Look at the tiebreak mode:
    let mut sorted_candidates: Vec<CandidateId> = match tiebreak {
        TieBreakMode::UseCandidateOrder => {
            let candidate_order: HashMap<CandidateId, usize> = candidate_names
                .iter()
                .enumerate()
                .map(|(idx, (_, cid))| (*cid, idx))
                .collect();
            let mut res = all_smallest;
            res.sort_by_key(|cid| candidate_order.get(cid).unwrap());
            // For loser selection, the selection is done in reverse order according to the reference implementation.
            res.reverse();
            debug!("find_eliminated_candidates_single: sorted candidates in elimination queue using tiebreak mode usecandidateorder: {:?}", res);
            res
        }
        TieBreakMode::Random(seed) => {
            let cand_with_names: Vec<(CandidateId, String)> = all_smallest
                .iter()
                .map(|cid| {
                    let m: Option<(CandidateId, String)> = candidate_names
                        .iter()
                        .filter_map(|(n, cid2)| {
                            if cid == cid2 {
                                Some((*cid2, n.clone()))
                            } else {
                                None
                            }
                        })
                        .next();
                    m.unwrap()
                })
                .collect();
            let res = candidate_permutation_crypto(&cand_with_names, seed, num_round);
            debug!(
                "find_eliminated_candidates_single: sorted candidates in elimination queue using tiebreak mode random: {:?}",
                res
            );
            res
        }
    };

    // Temp copy
    let sc = sorted_candidates.clone();

    // TODO check that it is accurate to do.
    // For now, just select a single candidate for removal.
    sorted_candidates.truncate(1);

    // We are currently proceeding to remove all the candidates. Do not remove the last one.
    if sc.len() == tally.len() {
        let last = sc.last().unwrap();
        sorted_candidates.retain(|cid| cid != last);
    }
    Some((sorted_candidates, TiebreakSituation::TiebreakOccured))
}

// The algorithm is lazy. It will only apply the rules up to finding the next candidate.
fn advance_voting(
    choices: &[Choice],
    still_valid: &HashSet<CandidateId>,
    duplicate_policy: DuplicateCandidateMode,
    overvote: OverVoteRule,
    skipped_ranks: MaxSkippedRank,
) -> Option<(CandidateId, Vec<Choice>)> {
    // Find a potential candidate.
    let first_candidate = choices
        .iter()
        .enumerate()
        .find_map(|(idx, choice)| match choice {
            Choice::Filled(cid) if still_valid.contains(cid) => Some((idx, cid)),
            _ => None,
        });
    if let Some((idx, cid)) = first_candidate {
        // A valid candidate was found, but still look in the initial slice to find if some
        // overvote or multiple blanks occured.
        let initial_slice = &choices[..idx];

        if duplicate_policy == DuplicateCandidateMode::Exhaust {
            let mut seen_cids: HashSet<CandidateId> = HashSet::new();
            for choice in initial_slice.iter() {
                match *choice {
                    Choice::Filled(cid) if seen_cids.contains(&cid) => {
                        return None;
                    }
                    Choice::Filled(cid) => {
                        seen_cids.insert(cid);
                    }
                    _ => {}
                }
            }
        }

        // Overvote rule
        let has_initial_overvote = initial_slice.iter().any(|c| *c == Choice::Overvote);
        if has_initial_overvote && overvote == OverVoteRule::ExhaustImmediately {
            return None;
        }

        // Skipped rank rule
        if skipped_ranks == MaxSkippedRank::ExhaustOnFirstOccurence {
            let has_skippable_elements = initial_slice
                .iter()
                .any(|choice| matches!(choice, Choice::Blank | Choice::Undervote));
            if has_skippable_elements {
                return None;
            }
        }

        if let MaxSkippedRank::MaxAllowed(range_len) = skipped_ranks {
            let mut start_skipped_block: Option<usize> = None;
            let rl = range_len as usize;
            for (idx, choice) in initial_slice.iter().enumerate() {
                match (choice, start_skipped_block) {
                    // We went beyond the threshold
                    (Choice::Blank, Some(start_idx)) if idx >= start_idx + rl => {
                        return None;
                    }
                    (Choice::Undervote, Some(start_idx)) if idx >= start_idx + rl => {
                        return None;
                    }
                    // We are starting a new block
                    (Choice::Blank, None) => {
                        start_skipped_block = Some(idx);
                    }
                    (Choice::Undervote, None) => {
                        start_skipped_block = Some(idx);
                    }
                    // We are exiting a block or encountering a new element. Reset.
                    _ => {
                        start_skipped_block = None;
                    }
                }
            }
        }

        let final_slice = &choices[idx + 1..];
        Some((*cid, final_slice.to_vec()))
    } else {
        None
    }
}

struct CheckResult {
    votes: Vec<VoteInternal>,
    // further_votes: Vec<VoteInternal>,
    candidates: Vec<(String, CandidateId)>,
    uwi_first_votes: Vec<VoteInternal>,
    count_exhausted_uwi_first_round: VoteCount,
}

// Candidates are returned in the same order.
fn checks(
    coll: &[Vote],
    reg_candidates: &[config::Candidate],
    rules: &config::VoteRules,
) -> Result<CheckResult, VotingErrors> {
    debug!("checks: coll size: {:?}", coll.len());
    let blacklisted_candidates: HashSet<String> = reg_candidates
        .iter()
        .filter_map(|c| {
            if c.excluded {
                Some(c.name.clone())
            } else {
                None
            }
        })
        .collect();
    let candidates: HashMap<String, CandidateId> = reg_candidates
        .iter()
        .enumerate()
        .map(|(idx, c)| (c.name.clone(), CandidateId((idx + 1) as u32)))
        .collect();

    let valid_cids: HashSet<CandidateId> = candidates.values().cloned().collect();

    // The votes that are validated and that have a candidate from the first round
    let mut validated_votes: Vec<VoteInternal> = vec![];
    // The votes that are valid but do not have a candidate in the first round.
    let mut uwi_validated_votes: Vec<VoteInternal> = vec![];
    // The count of votes that are immediately exhausted with a UWI in the first round.
    let mut uwi_exhausted_first_round: VoteCount = VoteCount::EMPTY;

    for v in coll.iter() {
        let mut choices: Vec<Choice> = vec![];
        for c in v.candidates.iter() {
            let choice: Choice = match c {
                BallotChoice::Candidate(name) if blacklisted_candidates.contains(name) => {
                    unimplemented!("blacklisted not implemented");
                }
                BallotChoice::Candidate(name) => {
                    if let Some(cid) = candidates.get(name) {
                        Choice::Filled(*cid)
                    } else {
                        // Undeclared candidate
                        Choice::Undeclared
                    }
                }
                BallotChoice::Blank => Choice::Blank,
                BallotChoice::Overvote => Choice::Overvote,
                BallotChoice::Undervote => Choice::Undervote,
                BallotChoice::UndeclaredWriteIn => Choice::Undeclared,
            };
            choices.push(choice);
        }

        let count = VoteCount(v.count);
        // The first choice is a valid one. A ballot can be constructed out of it.
        if let Some(Choice::Filled(cid)) = choices.first() {
            let candidates = RankedChoice {
                first_valid: *cid,
                rest: choices[1..].to_vec(),
            };
            validated_votes.push(VoteInternal { candidates, count });
        } else if let Some((first_cid, rest)) = advance_voting(
            &choices,
            &valid_cids,
            rules.duplicate_candidate_mode,
            rules.overvote_rule,
            rules.max_skipped_rank_allowed,
        ) {
            let candidates = RankedChoice {
                first_valid: first_cid,
                rest,
            };
            // The ballot started with undeclared but could eventually be recovered
            // with a valid candidate. Keep it.
            if let Some(Choice::Undeclared) = choices.first() {
                uwi_validated_votes.push(VoteInternal { candidates, count })
            } else {
                validated_votes.push(VoteInternal { candidates, count });
            }
        } else {
            // The vote cannot be recovered. Just record it if it was an undeclared ballot.
            if let Some(Choice::Undeclared) = choices.first() {
                uwi_exhausted_first_round += VoteCount(v.count);
            }
        }
    }

    debug!(
        "checks: vote aggs size: {:?}  candidates: {:?}",
        validated_votes.len(),
        candidates.len()
    );

    let ordered_candidates: Vec<(String, CandidateId)> = reg_candidates
        .iter()
        .filter_map(|c| candidates.get(&c.name).map(|cid| (c.name.clone(), *cid)))
        .collect();

    debug!("checks: ordered_candidates {:?}", ordered_candidates);
    Ok(CheckResult {
        votes: validated_votes,
        uwi_first_votes: uwi_validated_votes,
        candidates: ordered_candidates,
        count_exhausted_uwi_first_round: uwi_exhausted_first_round,
    })
}

/// Generates a "random" permutation of the candidates. Random in this context means hard to guess in advance.
/// This uses a cryptographic algorithm that is resilient to collisions.
fn candidate_permutation_crypto(
    candidates: &[(CandidateId, String)],
    seed: u32,
    num_round: u32,
) -> Vec<CandidateId> {
    let mut data: Vec<(CandidateId, String)> = candidates
        .iter()
        .map(|(cid, name)| (*cid, format!("{:08}{:08}{}", seed, num_round, name)))
        .collect();
    data.sort_by_key(|p| p.1.clone());
    data.iter().map(|p| p.0).collect()
}
