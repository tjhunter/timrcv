use snafu::OptionExt;

use crate::rcv::*;
use std::collections::HashSet;

use std::path::Path;

pub fn read_excel_file(path: String, cfs: &FileSource) -> BRcvResult<Vec<ParsedBallot>> {
    let p = path.clone();
    let mut workbook: Xlsx<_> =
        open_workbook(p).context(OpeningExcelSnafu { path: path.clone() })?;
    let wrange = workbook
        .worksheet_range_at(0)
        .context(EmptyExcelSnafu {})?
        .context(OpeningExcelSnafu { path: path.clone() })?;

    // TODO: no unwrap
    // The filename to add as a ballot id
    let simplified_file_name = Path::new(path.as_str())
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    let header = wrange.rows().next().context(EmptyExcelSnafu {})?;
    debug!("read_excel_file: header: {:?}", header);
    let start_range = cfs.first_vote_column_index()?;
    debug!("read_excel_file: start_range: {:?}", start_range);

    let mut iter = wrange.rows();
    // TODO check for correctness
    iter.next();
    let mut res: Vec<ParsedBallot> = Vec::new();
    for (idx, row) in iter.enumerate() {
        debug!("read_excel_file: workbook: {:?}", row);
        // Not looking at configuration for now: dropping the first column (id) and assuming that the last column is the weight.
        let choices = &row[start_range..];
        let mut cs: Vec<Vec<String>> = Vec::new();
        let num_row_choices = choices.len();
        for (idx, elt) in choices.iter().enumerate() {
            let bco = read_choice_calamine2(elt, idx == num_row_choices - 1)?;
            if let Some(bc) = bco {
                // TODO: justify why the whitespaces are removed.
                // This is required for test 2015_portland_mayor.
                cs.push(vec![bc.trim().to_string()]);
            }
        }
        // Count: look for it at the last cell.
        let last_elt = choices.last().context(EmptyExcelSnafu {})?;
        // TODO implement count
        let count: Option<u64> = match last_elt {
            calamine::DataType::Float(f) => Some(*f as u64),
            calamine::DataType::Int(i) => Some(*i as u64),
            calamine::DataType::String(_) => None,
            calamine::DataType::Empty => None,
            _ => {
                return Err(Box::new(RcvError::ExcelWrongCellType {
                    lineno: (idx + 2) as u64,
                    content: format!("{:?}", last_elt),
                }));
            }
        };
        let pb = ParsedBallot {
            id: Some(format!("{}-{:08}", simplified_file_name, idx)),
            count,
            choices: cs,
        };
        debug!("read_excel_file: ballot: {:?}", pb.clone());
        res.push(pb);
    }
    Ok(res)
}

fn read_choice_calamine2(
    cell: &calamine::DataType,
    is_last_column: bool,
) -> RcvResult<Option<String>> {
    match cell {
        calamine::DataType::String(s) => Ok(Some(s.clone())),
        calamine::DataType::Empty => Ok(Some("".to_string())),
        // The last column may contain the count in the ESS format -> drop it in this case.
        calamine::DataType::Float(_) if is_last_column => Ok(None),
        calamine::DataType::Int(_) if is_last_column => Ok(None),
        _ => whatever!(
            "TODO MSG:read_choice_calamine: could not understand cell {:?}",
            cell
        ),
    }
}

pub fn read_excel_file0(
    path: String,
    cfs: &FileSource,
    candidates: &[RcvCandidate],
    rules: &RcvRules,
) -> RcvResult<Vec<ranked_voting::Vote>> {
    let p = path.clone();
    let mut workbook: Xlsx<_> =
        open_workbook(p).context(OpeningExcelSnafu { path: path.clone() })?;
    let wrange = workbook
        .worksheet_range_at(0)
        .context(EmptyExcelSnafu {})?
        .context(OpeningExcelSnafu { path })?;

    // .ok_or(CError::Msg("Missing first sheet"))??;
    let header = wrange.rows().next().context(EmptyExcelSnafu {})?;
    debug!("header: {:?}", header);
    let start_range = cfs.first_vote_column_index()?;

    let candidate_names: HashSet<String> = candidates.iter().map(|c| c.name.clone()).collect();

    let mut iter = wrange.rows();
    // TODO check for correctness
    iter.next();
    let mut res: Vec<Vote> = Vec::new();
    for row in iter {
        debug!("workbook: {:?}", row);
        // Not looking at configuration for now: dropping the first column (id) and assuming that the last column is the weight.
        let choices = &row[start_range..];
        let mut cs: Vec<BallotChoice> = Vec::new();
        for elt in choices {
            let bc = read_choice_calamine(elt, &candidate_names, cfs)?;
            cs.push(bc)
        }
        // TODO implement count
        let count: u64 = match None {
            Some(calamine::DataType::Float(f)) => f as u64,
            Some(calamine::DataType::Int(i)) => i as u64,
            Some(_) => {
                whatever!("wrong type")
            }
            None => 1,
        };
        if let Some(v) = create_vote(&"NO ID".to_string(), count, &cs, rules)? {
            res.push(v);
        }
    }
    Ok(res)
}

fn read_choice_calamine(
    cell: &calamine::DataType,
    candidates: &HashSet<String>,
    source_setting: &FileSource,
) -> RcvResult<BallotChoice> {
    match cell {
        calamine::DataType::String(s) if candidates.contains(s) => {
            Ok(BallotChoice::Candidate(s.clone()))
        }
        calamine::DataType::String(s) if s == "UWI" => Ok(BallotChoice::UndeclaredWriteIn),
        calamine::DataType::String(s)
            if s.is_empty()
                && source_setting
                    .treat_blank_as_undeclared_write_in
                    .unwrap_or(false) =>
        {
            Ok(BallotChoice::UndeclaredWriteIn)
        }
        calamine::DataType::String(s) if source_setting.undervote_label == Some(s.clone()) => {
            Ok(BallotChoice::Undervote)
        }
        calamine::DataType::String(s) => {
            if let Some(delim) = source_setting.overvote_delimiter.clone() {
                if s.contains(&delim) {
                    return Ok(BallotChoice::Overvote);
                }
            }
            whatever!("Wrong data type: {:?}", s)
        }
        calamine::DataType::Empty => Ok(BallotChoice::Undervote),
        _ => whatever!(
            "TODO MSG:read_choice_calamine: could not understand cell {:?}",
            cell
        ),
    }
}
