// ********* Input data structures ***********

use std::default::Default;
use std::error::Error;
use std::fmt::Display;

/// All the possible states corresponding to a choice in a ballot.
///
/// In most cases, it is enough to use the higher-level builder API.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum BallotChoice {
    /// A candidate, which may or may not be in the list of valid candidates.
    Candidate(String),
    /// A name that has been already written out as not being a
    /// declared candidate.
    UndeclaredWriteIn,
    /// An excess of choices for this particular rank.
    /// The current system does not acccept more than one vote per rank.
    /// Any greater number will be treated as overvote and not tabulated.
    Overvote,
    /// A missing vote.
    Undervote,
    /// A blank content in the vote or some content that is not valid.
    /// This is the policy with blank votes that are not clearly labeled as under- or overvotes.
    Blank,
}

/// A ballot submitted by a voter.
///
/// This is a low-level interface that is meant to express all the situations
/// found in practice.
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Ballot {
    /// Ranked candidates in the ballot. The order of the candidates in
    /// the list indicates the rank of the choices made by the voter.
    pub candidates: Vec<BallotChoice>,
    /// A count associated to a ballot (typically 1). Ballots with
    /// a count of zero are immediately exhausted.
    pub count: u64,
}

// ******** Output data structures *********

/// Statistics for the elimination of the candidates.
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct EliminationStats {
    /// The name of the candidate being eliminated. It could also be
    /// 'Undeclared write-ins' to account for all the choices that do not
    /// correspond to declared candidates.
    pub name: String,
    /// Transfers of the votes to other candidates.
    /// Includes the names of the candidates and the count of votes
    /// associated to this transfer.
    pub transfers: Vec<(String, u64)>,
    /// The number of votes that were associated to this candidate and that
    /// do not have a transfer.
    pub exhausted: u64,
}

/// Statistics for one round
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct RoundStats {
    /// The id of the round (starting with 0)
    pub round: u32,
    /// The tally for each candidate.
    pub tally: Vec<(String, u64)>,
    /// The list of candidates that are elected in this round.
    pub tally_results_elected: Vec<String>,
    /// The list of candidates that are eliminated, along with
    /// transfer information.
    pub tally_result_eliminated: Vec<EliminationStats>,
}

/// The result, in case of a successful election.
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct VotingResult {
    /// The winner(s) of this election, if any.
    pub winners: Option<Vec<String>>,
    /// The threshold that was applied to determine the winners.
    pub threshold: u64,
    /// The statistics for each round.
    pub round_stats: Vec<RoundStats>,
}

/// Errors that prevent the algorithm from completing successfully.
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum VotingErrors {
    /// There is no vote to process.
    EmptyElection,
    /// The algorithm failed to determine one or more winners.
    ///
    // TODO: explain when it may happen
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

/// The different modes to break a tie in case of multiple counts.
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum TieBreakMode {
    /// Uses the order in which the candidates have been declared.
    /// The first candidate in the list will have priority over all other candidates.
    UseCandidateOrder,
    /// Use a random order. The input argument is the seed to initialize the
    /// order.
    ///
    /// Note: the random mode is implemented differently than the 'rcv' program. It uses a cryptographic hash on the candidate
    /// names instead of relying on the java primitives.
    Random(u32),
}

/// How to deal with overvotes.
///
/// An overvote happens when a ballot contains two names for the same rank.
/// This is disallowed in general by instant-runoff algorithms.
///
/// As an example, if a ballot is the following: `[["A", "B"], ["C"]]` with
/// two initial choices, the following will happen:
/// - the ballot will be exhausted (discarded) under `ExhaustImmediately`
/// - under AlwaysSkipToNextRank, the initial `["A", "B"]` choice will be discarded
///   and `"C"` will be considered instead.
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum OverVoteRule {
    /// The ballot is exhausted (discarded).
    ExhaustImmediately,
    /// This particular choice is skipped and the next choice in ranking order
    /// will be considered.
    AlwaysSkipToNextRank,
}

/// Strategy on how to deal with duplicated names.
///
/// Consider the ballot `[A, B, B, C]`. After candidate `A` is eliminated:
/// - with Exhaust, this ballot would be entirely discarded
/// - with SkipDuplicate, this ballot would be equivalent to reducing `B` to only
/// a single instance: `[B, C]`.
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum DuplicateCandidateMode {
    Exhaust,
    SkipDuplicate,
}

/// The sort of election to run.
/// For now, only elections with a single winner are implemented.
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

/// Controls how to deal with skipping blank or undervote pieces.
///
/// Consider the following ballot: `[BLANK, BLANK, BLANK, A]`
/// - Unlimited would read this ballot as `[A]`
/// - ExhaustOnFirstOccurence would discard this ballot
/// - `MaxAllowed(3)` would read this ballot as `[A]`, but `MaxAllowed(2)` or below would
/// exhaust the ballot.
///
/// The rule is only applied to sequences of blank or undervotes. For instance,
/// `MaxAllowed(2)` would:
/// - allow the ballot: `[BLANK, OVERVOTE, BLANK, OVERVOTE, A]`
/// - disallow the ballot: `[OVERVOTE, BLANK, BLANK, BLANK, A]`
///
/// Default: `Unlimited`.
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum MaxSkippedRank {
    Unlimited,
    ExhaustOnFirstOccurence,
    MaxAllowed(u32),
}

/// The rules that control the voting process.
///
/// The easiest way to use them is to use a default instance of the rules and modify them.
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct VoteRules {
    /// Tie break mode (see documentation)
    pub tiebreak_mode: TieBreakMode,
    /// Overvoting control (see documentation)
    pub overvote_rule: OverVoteRule,
    /// Winner selection (see documentation)
    pub winner_election_mode: WinnerElectionMode,
    // TODO: remove
    pub number_of_winners: u32,
    /// If set, indicates the minimum number of votes that a candidate
    /// must have in order to be considered. Any number below will lead to
    /// the candidate to be immediately eliminated.
    ///
    /// Default: None (no threshold)
    pub minimum_vote_threshold: Option<u32>,
    /// Control of skipped rankings (blank or undervote)
    pub max_skipped_rank_allowed: MaxSkippedRank,
    /// The maximum number of rankings (choices) allowed for each ballot.
    ///
    /// If a ballot has more choices than this number, it is immediately discarded.
    pub max_rankings_allowed: Option<u32>,
    pub elimination_algorithm: EliminationAlgorithm,
    /// Duplicate candidate control (see documentation)
    pub duplicate_candidate_mode: DuplicateCandidateMode,
}

impl Default for VoteRules {
    fn default() -> Self {
        VoteRules::DEFAULT_RULES.clone()
    }
}

impl VoteRules {
    const DEFAULT_RULES: VoteRules = VoteRules {
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
pub(crate) struct Candidate {
    pub name: String,
    pub code: Option<String>,
    pub excluded: bool,
}
