use log::{debug, info, warn};

use std::{collections::HashMap, hash::Hash, ops::AddAssign};

// Public structures

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Vote {
    candidates: Vec<String>,
    count: u64,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum VotingResult {
    LeaderFound(String),
    NoMajorityCandidate,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum VotingErrors {
    EmptyElection,
}

// Private structures

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
struct CandidateId(u32);

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
struct RankedChoice {
    first: CandidateId,
    rest: Vec<CandidateId>,
}

impl RankedChoice {
    /// Removes certain candidates from the ranked choice. Returns none if there is no candidate left.
    fn filtered_candidate(self: &Self, blacklist: &Vec<CandidateId>) -> Option<RankedChoice> {
        let mut choices = vec![self.first];
        choices.extend(self.rest.clone());
        let rem_choices: Vec<CandidateId> = choices
            .iter()
            .filter(|cid| !blacklist.contains(cid))
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
struct VoteAgg {
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

pub fn run_voting_stats(coll: &Vec<Vote>) -> Result<VotingResult, VotingErrors> {
    let (stats, candidates) = checks(coll)?;
    info!(
        "Processing {:?} aggregated votes, candidates: {:?}",
        stats.iter().count(),
        candidates
    );
    let candidates_by_id: HashMap<CandidateId, String> = candidates
        .iter()
        .map(|(cname, cid)| (cid.clone(), cname.clone()))
        .collect();
    let mut cur_stats: Vec<VoteAgg> = stats.clone();

    while cur_stats.clone().iter().count() > 0 {
        match &cur_stats[..] {
            [] => return Err(VotingErrors::EmptyElection),
            [va] => {
                let name = match candidates_by_id.get(&va.candidates.first) {
                    None => unimplemented!(""),
                    Some(cname) => cname.clone(),
                };
                return Ok(VotingResult::LeaderFound(name));
            }
            _ => {}
        }
        (cur_stats, _) = run_one_round(&cur_stats)?;
    }
    unimplemented!("");
}

/// Returns the removed candidates, and the remaining votes
fn run_one_round(votes: &Vec<VoteAgg>) -> Result<(Vec<VoteAgg>, Vec<CandidateId>), VotingErrors> {
    let by_groups = votes.iter().fold(HashMap::new(), |mut acc, va| {
        *acc.entry(va.candidates.first).or_insert(VoteCount(0)) += va.count;
        acc
    });
    match by_groups.values().min() {
        None => return Ok((Vec::new(), Vec::new())),
        Some(min_count) => {
            let all_smallest: Vec<CandidateId> = by_groups
                .iter()
                .filter_map(|(cid, vc)| if *vc <= *min_count { Some(cid) } else { None })
                .cloned()
                .collect();
            debug!("all_smallest: {:?}", all_smallest);
            // Filter the rest of the votes to simply keep the votes that still matter
            let rem_votes: Vec<VoteAgg> = votes
                .iter()
                .filter_map(|va| {
                    // Remove the choices that are not valid anymore
                    va.candidates
                        .filtered_candidate(&all_smallest)
                        .map(|rc| VoteAgg {
                            candidates: rc,
                            count: va.count,
                        })
                })
                .collect();
            debug!("count votes: {:?}", rem_votes.iter().count());
            Ok((rem_votes, all_smallest))
        }
    }
}

fn checks(coll: &Vec<Vote>) -> Result<(Vec<VoteAgg>, HashMap<String, CandidateId>), VotingErrors> {
    debug!("checks: coll size: {:?}", coll.iter().count());
    let mut candidates: HashMap<String, CandidateId> = HashMap::new();
    let mut counter: u32 = 0;
    let vas: Vec<VoteAgg> = coll
        .iter()
        .map(|v| {
            let cs: Vec<CandidateId> = v
                .candidates
                .iter()
                .map(|c| {
                    counter += 1;
                    candidates
                        .entry(c.clone())
                        .or_insert(CandidateId(counter))
                        .clone()
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
            VoteAgg {
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

    let res = run_voting_stats(&data);

    info!("res {:?}", res);

    info!("info arg {:?}", x);
    warn!("info arg {:?}", x);
    x + 1
}
