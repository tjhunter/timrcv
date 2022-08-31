// The configuration options
// They follow the configuration options defined here:
// https://github.com/BrightSpots/rcv/blob/develop/config_file_documentation.txt

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum TieBreakMode {
    // stopCountingAndAsk is not going to be implemented.
    UseCandidateOrder,
    // TODO add the other modes
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum WinnerElectionMode {
    SingelWinnerMajority, // TODO add the other modes
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct VoteRules {
    pub tiebreak_mode: TieBreakMode,
    // TODO overvote rule
    pub winner_election_mode: WinnerElectionMode,
    pub number_of_winners: u32, // TODO should it be an option?
    // TODO decimalPlacesForVoteArithmetic
    pub minimum_vote_threshold: Option<u32>,
    // TODO max_skipped_rank_allowed. currently set to unlimited
    pub max_rankings_allowed: Option<u32>,
    // TODO: randomSeed
    // TODO multiSeatBottomsUpPercentageThreshold
    // TODO rulesDescription
    // TODO nonIntegerWinningThreshold
    // TODO hareQuota
    // TODO batchElimination
    // TODO continueUntilTwoCandidatesRemain
    // TODO exhaustOnDuplicateCandidate <- do this one TODO
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub code: Option<String>,
    pub excluded: bool,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum RoundCandidateStatus {
    StillRunning,
    Elected,
    /// if eliminated, the transfers of the votes to each candidate
    /// the last element is the number of exhausted votes
    Eliminated(Vec<(String, u64)>, u64),
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct RoundCandidateStats {
    pub name: String,
    pub tally: u64,
    pub status: RoundCandidateStatus,
}

/// Statistics for one round
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct RoundStats {
    pub tally: Vec<RoundCandidateStats>,
}

/// Result statistics that can the be processed by analysis tools.
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct ResultStats {
    pub rounds: Vec<RoundStats>,
}
