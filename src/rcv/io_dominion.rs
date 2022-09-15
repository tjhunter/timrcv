use snafu::OptionExt;

use crate::rcv::{
    io_common::{assemble_choices, get_count},
    *,
};
use std::collections::HashMap;

pub fn read_dominion(path: &str) -> BRcvResult<Vec<ParsedBallot>> {
    let manifest: CandidateManifest = {
        let p: PathBuf = [path, "CandidateManifest.json"].iter().collect();
        let cvr_export_path = p.as_path().display().to_string();
        info!(
            "Attempting to read candidate manifest file {:?}",
            cvr_export_path
        );

        let contents = fs::read_to_string(cvr_export_path.clone()).context(OpeningJsonSnafu {})?;
        debug!("Read rank file {:?}", cvr_export_path);
        serde_json::from_str(contents.as_str()).context(ParsingJsonSnafu {})?
    };

    let cvrr: CvrExport = {
        let p: PathBuf = [path, "CvrExport.json"].iter().collect();
        let cvr_export_path = p.as_path().display().to_string();
        info!("Attempting to read rank file {:?}", cvr_export_path);

        let contents = fs::read_to_string(cvr_export_path.clone()).context(OpeningJsonSnafu {})?;
        debug!("Read rank file {:?}", cvr_export_path);
        serde_json::from_str(contents.as_str()).context(ParsingJsonSnafu {})?
    };

    let mut candidate_id_mapping: HashMap<u32, String> = HashMap::new();
    for c in manifest.candidates.iter() {
        candidate_id_mapping.insert(c.id, c.name.clone());
    }

    debug!("candidate_id_mapping {:?}", candidate_id_mapping);

    let mut ballots: Vec<ParsedBallot> = vec![];

    // Very simple parsing for now, assuming that there is a single contest.
    for s in cvrr.sessions.iter() {
        for card in s.original.cards.iter() {
            let mut num_votes: Vec<u64> = vec![];
            let mut ranks: Vec<(String, u32)> = vec![];
            for contest in card.contests.iter() {
                for mark in contest.marks.iter() {
                    debug!("mark {:?}", mark);
                    let candidate_name = candidate_id_mapping
                        .get(&mark.candidate_id)
                        .context(DominionParsingJsonSnafu {})?;
                    // TODO: could use here isvote / isambiguous
                    num_votes.push(1);
                    ranks.push((candidate_name.clone(), mark.rank));
                }
            }
            let b = ParsedBallot {
                id: None, // TODO
                count: get_count(&num_votes),
                choices: assemble_choices(&ranks),
            };
            debug!("ballot: {:?}", b.clone());
            ballots.push(b);
        }
    }

    Ok(ballots)
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct Mark {
    #[serde(rename = "CandidateId")]
    candidate_id: u32,
    #[serde(rename = "Rank")]
    rank: u32,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct Contest {
    #[serde(rename = "Marks")]
    pub marks: Vec<Mark>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct Card {
    #[serde(rename = "Contests")]
    pub contests: Vec<Contest>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct Original {
    #[serde(rename = "Cards")]
    pub cards: Vec<Card>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct Session {
    #[serde(rename = "Original")]
    pub original: Original,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct CvrExport {
    #[serde(rename = "Sessions")]
    pub sessions: Vec<Session>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct Candidate {
    #[serde(rename = "Description")]
    pub name: String,
    #[serde(rename = "Id")]
    pub id: u32,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
struct CandidateManifest {
    #[serde(rename = "List")]
    pub candidates: Vec<Candidate>,
}
