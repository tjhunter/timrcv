// The configuration options
// They follow the configuration options defined here:
// https://github.com/BrightSpots/rcv/blob/develop/config_file_documentation.txt

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum TieBreakMode {
    // stopCountingAndAsk is not going to be implemented.
    UseCandidateOrder,
    // Note: the random mode is implemented differently. It uses a cryptographic hash on the candidate
    // names instead of relying on the java primitives.
    Random(u32),
    // TODO add the other modes
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum DuplicateCandidateMode {
    Exhaust,
    SkipDuplicate,
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
    pub duplicate_candidate_mode: DuplicateCandidateMode,
}

impl VoteRules {
    pub const DEFAULT_RULES: VoteRules = VoteRules {
        tiebreak_mode: TieBreakMode::UseCandidateOrder,
        winner_election_mode: WinnerElectionMode::SingelWinnerMajority,
        number_of_winners: 1,
        minimum_vote_threshold: None,
        max_rankings_allowed: None,
        duplicate_candidate_mode: DuplicateCandidateMode::SkipDuplicate,
    };
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub code: Option<String>,
    pub excluded: bool,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct EliminationStats {
    pub name: String,
    pub transfers: Vec<(String, u64)>,
    pub exhausted: u64,
}

/// Statistics for one round
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct RoundStats {
    pub round: u32,
    pub tally: Vec<(String, u64)>,
    pub tally_results_elected: Vec<String>,
    pub tally_result_eliminated: Vec<EliminationStats>,
}

/// Result statistics that can the be processed by analysis tools.
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct ResultStats {
    pub rounds: Vec<RoundStats>,
}
