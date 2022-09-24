use crate::rcv::*;

use serde::{Deserialize, Serialize};
use serde_json::Value as JSValue;

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct OutputSettings {
    #[serde(rename = "contestName")]
    pub contest_name: String,
    #[serde(rename = "outputDirectory")]
    pub output_directory: Option<String>,
    #[serde(rename = "contestDate")]
    pub contest_date: Option<String>,
    #[serde(rename = "contestJurisdiction")]
    pub contest_juridiction: Option<String>,
    #[serde(rename = "contestOffice")]
    pub contest_office: Option<String>,
    #[serde(rename = "tabulateByPrecinct")]
    pub tabulate_by_precinct: Option<bool>,
    #[serde(rename = "generateCdfJson")]
    pub generate_cdf_json: Option<bool>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub contest: String,
    pub date: Option<String>,
    pub jurisdiction: Option<String>,
    pub office: Option<String>,
    pub threshold: Option<String>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct FileSource {
    pub provider: String,
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "contestId")]
    pub contest_id: Option<String>,
    #[serde(rename = "firstVoteColumnIndex")]
    _first_vote_column_index: Option<JSValue>,
    #[serde(rename = "firstVoteRowIndex")]
    _first_vote_row_index: Option<JSValue>,
    #[serde(rename = "idColumnIndex")]
    pub id_column_index: Option<JSValue>,
    #[serde(rename = "precinctColumnIndex")]
    pub precinct_column_index: Option<String>,
    #[serde(rename = "overvoteDelimiter")]
    pub overvote_delimiter: Option<String>,
    #[serde(rename = "overvoteLabel")]
    pub overvote_label: Option<String>,
    #[serde(rename = "undervoteLabel")]
    pub undervote_label: Option<String>,
    #[serde(rename = "undeclaredWriteInLabel")]
    pub undeclared_write_in_label: Option<String>,
    #[serde(rename = "treatBlankAsUndeclaredWriteIn")]
    pub treat_blank_as_undeclared_write_in: Option<bool>,
    // New options specific to timrcv
    #[serde(rename = "countColumnIndex")]
    pub count_column_index: Option<JSValue>,
    #[serde(rename = "excelWorksheetName")]
    pub excel_worksheet_name: Option<String>,
    #[serde(rename = "choices")]
    pub choices: Option<Vec<String>>,
}

impl FileSource {
    pub fn first_vote_column_index(&self) -> RcvResult<usize> {
        let x = read_js_int(&self._first_vote_column_index)?;
        Ok(x - 1)
    }

    pub fn first_vote_row_index(&self) -> RcvResult<usize> {
        let x = read_js_int(&self._first_vote_row_index)?;
        Ok(x - 1)
    }

    pub fn id_column_index_int(&self) -> RcvResult<Option<usize>> {
        if self.id_column_index.is_some() {
            read_js_int(&self.id_column_index).map(Some)
        } else {
            Err(RcvError::ParsingJsonNumber {})
        }
    }
    pub fn count_column_index_int(&self) -> RcvResult<Option<usize>> {
        if self.count_column_index.is_some() {
            read_js_int(&self.count_column_index).map(Some)
        } else {
            Err(RcvError::ParsingJsonNumber {})
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct RcvCandidate {
    pub name: String,
    pub code: Option<String>,
    pub excluded: Option<bool>,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct RcvRules {
    #[serde(rename = "tiebreakMode")]
    pub tiebreak_mode: String,
    #[serde(rename = "overvoteRule")]
    pub _overvote_rule: String,
    #[serde(rename = "winnerElectionMode")]
    pub winner_election_mode: String,
    #[serde(rename = "randomSeed")]
    pub random_seed: Option<String>,
    #[serde(rename = "maxSkippedRanksAllowed")]
    pub max_skipped_ranks_allowed: String,
    #[serde(rename = "maxRankingsAllowed")]
    pub max_rankings_allowed: String,
    #[serde(rename = "rulesDescription")]
    pub rules_description: Option<String>,
    #[serde(rename = "batchElimination")]
    pub batch_elimination: Option<bool>,
    #[serde(rename = "exhaustOnDuplicateCandidate")]
    pub exhaust_on_duplicate_candidate: Option<bool>,
}

impl RcvRules {
    pub fn overvote_rule(&self) -> RcvResult<OverVoteRule> {
        match self._overvote_rule.as_str() {
            "exhaustImmediately" => Ok(OverVoteRule::ExhaustImmediately),
            "alwaysSkipToNextRank" => Ok(OverVoteRule::AlwaysSkipToNextRank),
            "invalidOption" => whatever!("overvote rule is an invalid option for this contest"),
            _ => whatever!("unknown overvote rule: {}", self._overvote_rule),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct RcvConfig {
    #[serde(rename = "outputSettings")]
    pub output_settings: OutputSettings,
    #[serde(rename = "cvrFileSources")]
    pub cvr_file_sources: Vec<FileSource>,
    pub candidates: Vec<RcvCandidate>,
    pub rules: RcvRules,
}

pub fn read_summary(path: String) -> BRcvResult<JSValue> {
    let contents = fs::read_to_string(path.clone()).context(OpeningJsonSnafu { path })?;
    // debug!("read content: {:?}", contents);
    let mut js: JSValue = serde_json::from_str(contents.as_str()).context(ParsingJsonSnafu {})?;
    // Order the tally results to ensure stability
    // Remove the mention of the undeclared write-in's when they have zero votes associated to them.
    let results_ordered: Vec<JSValue> = js["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|jsv| {
            let mut res = jsv.clone();
            let mut tally_results: Vec<JSValue> = res["tallyResults"]
                .as_array()
                .unwrap()
                .clone()
                .iter()
                .filter(|jsv| {
                    let obj = jsv.as_object().unwrap().clone();
                    if obj.get("eliminated").is_some() {
                        !obj.get("transfers")
                            .unwrap()
                            .as_object()
                            .unwrap()
                            .is_empty()
                    } else {
                        true
                    }
                })
                .cloned()
                .collect();

            tally_results.sort_by_key(|trjs| {
                let obj = trjs.as_object().unwrap().clone();
                let elected = obj.get("elected");
                let eliminated = obj.get("eliminated");
                let s: String = elected
                    .or(eliminated)
                    .map(|x| x.as_str().unwrap().to_string())
                    .unwrap();
                s
            });

            let mut tally = res["tally"].as_object().unwrap().clone();
            let k = "Undeclared Write-ins".to_string();
            if let Some(v) = tally.get(&k) {
                if v.as_str() == Some("0") {
                    tally.remove(&k);
                }
            }

            res["tallyResults"] = serde_json::Value::Array(tally_results);
            res["tally"] = serde_json::Value::Object(tally);
            res
        })
        .collect();
    js["results"] = serde_json::Value::Array(results_ordered);
    // debug!("read content: {:?}", js["results"].as_array().unwrap());
    Ok(js)
}

fn read_js_int(x: &Option<JSValue>) -> RcvResult<usize> {
    match x {
        Some(JSValue::Number(n)) => n
            .as_u64()
            .map(|x| x as usize)
            .context(ParsingJsonNumberSnafu {}),
        // Parsing the Excel-style columns
        Some(JSValue::String(s)) if s.chars().all(|c| c.is_alphabetic()) => {
            // Just treating the simple case for now. It should be expanded to more than 26 columns.
            assert_eq!(s.chars().count(), 1);
            let c1: char = s.to_lowercase().chars().next().unwrap();
            Ok((c1 as usize) - ('a' as usize))
        }
        Some(JSValue::String(s)) => s.parse::<usize>().ok().context(ParsingJsonNumberSnafu {}),
        _ => None.context(ParsingJsonNumberSnafu {}),
    }
}
