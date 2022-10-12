pub use crate::config::*;

/// A builder for adding votes.
///
/// Using the builder should be considered for performance code.
///
/// ```
/// pub use ranked_voting::builder::Builder;
/// pub use ranked_voting::VoteRules;
/// # use ranked_voting::VotingErrors;
///
/// let mut builder = Builder::new(&VoteRules::DEFAULT_RULES)?
///     .candidates(&["Anna".to_string(), "Bob".to_string()])?;
///
/// builder.add_vote_simple(&["Anna".to_string(), "Clara".to_string(), "".to_string()])?;
///
///
/// # Ok::<(), VotingErrors>(())
/// ```
pub struct Builder {
    pub(crate) _rules: VoteRules,
    pub(crate) _candidates: Option<Vec<Candidate>>,
    pub(crate) _votes: Vec<Vote>,
}

impl Builder {
    pub fn new(rules: &VoteRules) -> Result<Builder, VotingErrors> {
        Ok(Builder {
            _rules: rules.clone(),
            _candidates: None,
            _votes: Vec::new(),
        })
    }

    pub fn candidates(self, cands: &[String]) -> Result<Builder, VotingErrors> {
        Ok(Builder {
            _rules: self._rules,
            _candidates: Some(
                cands
                    .to_vec()
                    .iter()
                    .map(|name| Candidate {
                        name: name.clone(),
                        code: None,
                        excluded: false,
                    })
                    .collect(),
            ),
            _votes: Vec::new(),
        })
    }

    /// Adds a vote to the builder.
    ///
    /// It is the simplest use case for most cases.
    ///
    pub fn add_vote_simple(&mut self, candidates: &[String]) -> Result<(), VotingErrors> {
        self.add_vote(&[candidates.to_vec()], 1)
    }

    /// Adds a vote, with a potential weight attached to it.
    ///
    /// candidates: the list of choices made by the voter, in order. Choices do not need to be unique,
    /// or distinct or non-empty.
    pub fn add_vote(&mut self, candidates: &[Vec<String>], count: u32) -> Result<(), VotingErrors> {
        let mut choices: Vec<BallotChoice> = Vec::new();
        for c in candidates {
            let cand = match c.as_slice() {
                [] => BallotChoice::Undervote,
                [s] if s.is_empty() => BallotChoice::Blank,
                [s] => {
                    if let Some(valid_candidates) = self._candidates.as_deref() {
                        if valid_candidates.iter().any(|cd| cd.name == *s) {
                            BallotChoice::Candidate(s.clone())
                        } else {
                            BallotChoice::UndeclaredWriteIn
                        }
                    } else {
                        BallotChoice::Candidate(s.clone())
                    }
                }
                _ => BallotChoice::Overvote,
            };
            choices.push(cand);
        }
        self.add_vote_2(&Vote {
            count: count as u64,
            candidates: choices,
        })
    }

    pub fn add_vote_2(&mut self, vote: &Vote) -> Result<(), VotingErrors> {
        self._votes.push(vote.clone());
        Ok(())
    }
}
