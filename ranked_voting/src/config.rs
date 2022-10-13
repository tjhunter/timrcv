// ********* Input data structures ***********

use std::error::Error;
use std::fmt::Display;

/// All the possible states corresponding to a choice in a ballot.
///
/// In most cases, it is enough to use the higher-level builder API.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum BallotChoice {
    /// A candidate, which may or may not be valid.
    Candidate(String),
    /// A name that has been already written out as not being a
    /// declared candidate.
    UndeclaredWriteIn,
    /// An excess of choices for this particular rank.
    /// The current system does not acccept more than one vote per rank.
    /// Any greater number will be treated as overvote.
    Overvote,
    /// A missing vote.
    Undervote,
    /// A blank content in the vote or some content that is not valid.
    /// This is the policy with blank votes that are not clearly labeled as under- or overvotes.
    Blank,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Vote {
    pub candidates: Vec<BallotChoice>,
    pub count: u64,
}

// ******** Output data structures *********

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

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct VotingResult {
    // TODO: replace by an enumeration: SingleWinner, MultiWinner, NoWinner
    pub winners: Option<Vec<String>>,
    pub threshold: u64,
    pub round_stats: Vec<RoundStats>,
}

/// Errors that prevent the algorithm from completig successfully.
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum VotingErrors {
    EmptyElection,
    NoConvergence,
}

impl Error for VotingErrors {}

impl Display for VotingErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VotingError in ranked_choice")
    }
}

// ********* Configuration **********

// The configuration options
// They follow the configuration options defined here:
// https://github.com/BrightSpots/rcv/blob/develop/config_file_documentation.txt

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum TieBreakMode {
    // stopCountingAndAsk is not going to be implemented.
    UseCandidateOrder,
    // Note: the random mode is implemented differently. It uses a cryptographic hash on the candidate
    // names instead of relying on the java primitives.
    Random(u32),
    // TODO add the other modes
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum OverVoteRule {
    ExhaustImmediately,
    AlwaysSkipToNextRank,
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

/// The elimination algorithm to apply.
///
/// - Single eliminates one candidate at a time. This is the easiest to
/// understand, but it may add many more rounds when there a lot of
/// candidates with a comparatively very low number of votes.
///
/// - Batch eliminates candidates more rapidly.
/// TODO document algorithm.
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum EliminationAlgorithm {
    Batch,
    Single,
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum MaxSkippedRank {
    Unlimited,
    ExhaustOnFirstOccurence,
    MaxAllowed(u32),
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct VoteRules {
    pub tiebreak_mode: TieBreakMode,
    pub overvote_rule: OverVoteRule,
    pub winner_election_mode: WinnerElectionMode,
    pub number_of_winners: u32, // TODO should it be an option?
    // TODO decimalPlacesForVoteArithmetic
    pub minimum_vote_threshold: Option<u32>,
    pub max_skipped_rank_allowed: MaxSkippedRank,
    pub max_rankings_allowed: Option<u32>,
    // TODO: randomSeed
    // TODO multiSeatBottomsUpPercentageThreshold
    // TODO rulesDescription
    // TODO nonIntegerWinningThreshold
    // TODO hareQuota
    // TODO continueUntilTwoCandidatesRemain
    pub elimination_algorithm: EliminationAlgorithm,
    pub duplicate_candidate_mode: DuplicateCandidateMode,
}

impl VoteRules {
    pub const DEFAULT_RULES: VoteRules = VoteRules {
        tiebreak_mode: TieBreakMode::UseCandidateOrder,
        overvote_rule: OverVoteRule::AlwaysSkipToNextRank,
        winner_election_mode: WinnerElectionMode::SingelWinnerMajority,
        max_skipped_rank_allowed: MaxSkippedRank::Unlimited,
        number_of_winners: 1,
        minimum_vote_threshold: None,
        max_rankings_allowed: None,
        elimination_algorithm: EliminationAlgorithm::Single,
        duplicate_candidate_mode: DuplicateCandidateMode::SkipDuplicate,
    };
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub code: Option<String>,
    pub excluded: bool,
}
