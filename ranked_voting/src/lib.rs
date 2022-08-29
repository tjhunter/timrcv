use itertools::Itertools;
use log::{debug, info, warn};

use std::{collections::HashMap, ops::AddAssign};

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
struct RankedChoice(Vec<CandidateId>);

impl RankedChoice {
    fn only_candidate(self: &Self) -> Option(CandidateId) {
        self.0.first()
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
    let candidates_by_id: HashMap<CandidateId, String> = candidates
        .iter()
        .map(|(cname, cid)| (cid.clone(), cname.clone()))
        .collect();
    let mut cur_stats: &Vec<VoteAgg> = &stats;

    while cur_stats.iter().count() > 0 {
        match &cur_stats[..] {
            [] => return Err(VotingErrors::EmptyElection),
            [va] => {
                let name = match va
                    .candidates
                    .only_candidate()
                    .and_then(|cid| candidates_by_id.get(&cid))
                {
                    None => unimplemented!(""),
                    Some(cname) => cname.clone(),
                };
                return Ok(VotingResult::LeaderFound(name));
            }
            _ => {}
        }
        cur_stats = &run_one_round(cur_stats)?;
    }
    unimplemented!("");
}

fn run_one_round(coll: &Vec<VoteAgg>) -> Result<Vec<VoteAgg>, VotingErrors> {
    unimplemented!("");
}

fn checks(_coll: &Vec<Vote>) -> Result<(Vec<VoteAgg>, HashMap<String, CandidateId>), VotingErrors> {
    unimplemented!("checks")
}

// // Very simple version full of cloning for the time being.
// fn run_voting_stats_inner(coll: &Vec<VoteAgg>) -> VotingResult {
//     // Making a copy for the time being
//     let coll2: Vec<VoteAgg> = coll.iter().filter(|va| va.count.0 > 0).cloned().collect();
//     debug!("filtered: {:?}", coll2);

//     match &coll2[..] {
//         [] => return VotingResult::EmptyElection,
//         [va] => return VotingResult::LeaderFound(va.candidate.clone()),
//         _ => {
//             let by_groups = coll2.iter().fold(HashMap::new(), |mut acc, va| {
//                 *acc.entry(va.candidate).or_insert(VoteCount(0)) += va.count;
//                 acc
//             });
//             match by_groups.values().min() {
//                 None => return VotingResult::EmptyElection,
//                 Some(min_count) => {
//                     let _all_smaller: Vec<(&CandidateId, &VoteCount)> = by_groups
//                         .iter()
//                         .filter(|(_, vc)| **vc <= *min_count)
//                         .collect();
//                     debug!("_all_smaller: {:?}", _all_smaller);
//                 }
//             }
//             // Find the minimum candidate(s)
//             let by_cid = &coll2.into_iter().group_by(|key| key.candidate);
//             let _mini = by_cid
//                 .into_iter()
//                 .map(|(cid, gr)| (cid, gr.map(|va| va.count).sum::<VoteCount>()));

//             if coll.into_iter().count() > 0 {
//                 VotingResult::LeaderFound(CandidateId(0))
//             } else {
//                 VotingResult::NoMajorityCandidate
//             }
//         }
//     }
// }

// pub fn run_voting_stats0<'a>(coll: &(impl IntoIterator<Item = &'a VoteAgg> + Copy)) -> VotingResult {
//     debug!("count: {:?}", coll.into_iter().count());
//     debug!("count: {:?}", coll.into_iter().count());

//     coll.into_iter().filter(|va| va.count.0 > 0);

//     if coll.into_iter().count() > 0 {
//         VotingResult::LeaderFound(CandidateId(0))
//     } else {
//         VotingResult::NoMajorityCandidate
//     }
// }

pub fn add_one(x: i32) -> i32 {
    let data = vec![Vote {
        candidates: vec!["a".to_string(), "b".to_string()],
        count: 1,
    }];

    info!("data {:?}", data);

    let res = run_voting_stats(&data);

    info!("res {:?}", res);

    info!("info arg {:?}", x);
    warn!("info arg {:?}", x);
    x + 1
}
