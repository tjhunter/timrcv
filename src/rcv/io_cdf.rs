use snafu::OptionExt;

use crate::rcv::*;
use std::collections::HashMap;

pub fn read_json(path: String) -> BRcvResult<Vec<ParsedBallot>> {
    let contents = fs::read_to_string(path).context(OpeningJsonSnafu {})?;

    let cvrr: CastVoteRecordReport =
        serde_json::from_str(contents.as_str()).context(ParsingJsonSnafu {})?;

    // Mapping from id to candidate name
    let mut candidateids_mapping: HashMap<String, String> = HashMap::new();
    let mut candidate_contest_mapping: HashMap<String, String> = HashMap::new();
    let e = cvrr.election.get(0).context(CdfParsingJsonSnafu {})?;
    for c in e.contests.iter() {
        for cs in c.contest_selection.iter() {
            for cid in cs.candidate_ids.iter() {
                candidate_contest_mapping.insert(cid.clone(), cs.candidate_selection_id.clone());
            }
        }
    }
    for c in e.candidates.iter() {
        let contest_id = candidate_contest_mapping
            .get(&c.candidate_id)
            .context(CdfParsingJsonSnafu {})?;
        candidateids_mapping.insert(contest_id.clone(), c.candidate_name.clone());
    }

    let mut ballots: Vec<ParsedBallot> = Vec::new();
    for cvr in cvrr.cvr.iter() {
        for snap in cvr.snapshots.iter() {
            for contest in snap.contests.iter() {
                let mut num_votes: Vec<u64> = vec![];
                let mut ranks: Vec<(String, u64)> = vec![];
                for selection in contest.selection.iter() {
                    let candidate_name = candidateids_mapping
                        .get(&selection.selection_id)
                        .context(CdfParsingJsonSnafu {})?;
                    for pos in selection.positions.iter() {
                        num_votes.push(pos.num_votes);
                        ranks.push((candidate_name.clone(), pos.rank))
                    }
                }
                let max_sels = ranks
                    .iter()
                    .map(|(_, rank)| *rank)
                    .max()
                    .context(CdfParsingJsonSnafu {})?;
                let mut choices: Vec<Vec<String>> = vec![];
                for _ in 1..max_sels {
                    choices.push(vec![]);
                }
                for (cname, rank) in ranks.iter() {
                    if let Some(elt) = choices.get_mut((rank - 1) as usize) {
                        elt.push(cname.clone());
                    }
                }
                // TODO: check that all the votes have the same weight
                let count: u64 = *num_votes.first().context(CdfParsingJsonSnafu {})?;
                ballots.push(ParsedBallot {
                    id: None, // TODO
                    count: Some(count),
                    choices,
                });
            }
        }
    }

    debug!(
        "read_json: candidateids_mapping: {:?}",
        candidateids_mapping
    );

    Ok(ballots)
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct CVRSelectionPosition {
    #[serde(rename = "NumberVotes")]
    pub num_votes: u64,
    #[serde(rename = "Rank")]
    pub rank: u64,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct CVRContestSelection {
    #[serde(rename = "ContestSelectionId")]
    pub selection_id: String,
    #[serde(rename = "SelectionPosition")]
    pub positions: Vec<CVRSelectionPosition>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct CVRContest {
    #[serde(rename = "CVRContestSelection")]
    pub selection: Vec<CVRContestSelection>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct CVRSnapshot {
    #[serde(rename = "CVRContest")]
    pub contests: Vec<CVRContest>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct Cvr {
    #[serde(rename = "BallotPrePrintedId")]
    pub ballot_id: String,
    #[serde(rename = "CVRSnapshot")]
    pub snapshots: Vec<CVRSnapshot>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct Candidate {
    #[serde(rename = "@id")]
    pub candidate_id: String,
    #[serde(rename = "Name")]
    pub candidate_name: String,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct CandidateSelection {
    #[serde(rename = "@id")]
    pub candidate_selection_id: String,
    #[serde(rename = "CandidateIds")]
    pub candidate_ids: Vec<String>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct Contest {
    #[serde(rename = "ContestSelection")]
    pub contest_selection: Vec<CandidateSelection>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct Election {
    #[serde(rename = "Candidate")]
    pub candidates: Vec<Candidate>,
    #[serde(rename = "Contest")]
    pub contests: Vec<Contest>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct CastVoteRecordReport {
    #[serde(rename = "Election")]
    election: Vec<Election>,
    #[serde(rename = "CVR")]
    cvr: Vec<Cvr>,
}
