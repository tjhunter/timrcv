use calamine::DataType;
use std::collections::HashMap;

use crate::rcv::{
    io_common::{assemble_choices, make_default_id_lineno},
    *,
};

pub fn read_msforms_ranking(path: String, cfs: &FileSource) -> BRcvResult<Vec<ParsedBallot>> {
    let default_id = make_default_id_lineno(&path);

    let wrange = get_range(&path, cfs)?;

    let header = wrange.rows().next().context(EmptyExcelSnafu {})?;
    debug!("read_excel_file: header: {:?}", header);
    let start_range = cfs.first_vote_column_index()? + 1;
    debug!("read_excel_file: start_range: {:?}", start_range);

    let mut iter = wrange.rows();
    // TODO check for correctness
    // Not looking at configuration for now: dropping the first column (id) and assuming that the last column is the weight.
    iter.next();
    let mut res: Vec<ParsedBallot> = Vec::new();
    for (idx, row) in iter.enumerate() {
        debug!(
            "read_excel_file: idx: {:?} row: {:?}",
            idx,
            &row.get(start_range)
        );

        // Hardcode the parsing of the row for now. The CSV crate does not help as
        // much as anticipated in these situations.

        let choices_s = row.get(start_range).context(EmptyExcelSnafu {})?;
        let choices_parsed: Vec<Vec<String>> = match choices_s {
            calamine::DataType::String(s) => s.split(';').map(|s| vec![s.to_string()]).collect(),
            _ => {
                return Err(Box::new(RcvError::ExcelWrongCellType {
                    lineno: idx as u64,
                    content: format!("{:?}", row),
                }));
            }
        };

        debug!("read_excel_file: idx: {:?} row: {:?}", idx, &choices_parsed);

        let pb = ParsedBallot {
            id: Some(default_id(idx)),
            // MS forms are not expected to handle weights for the time being.
            count: Some(1),
            choices: choices_parsed,
        };
        res.push(pb);
    }
    Ok(res)
}

pub fn read_msforms_likert(
    path: String,
    cfs: &FileSource,
    candidate_names: &[String],
) -> BRcvResult<Vec<ParsedBallot>> {
    let default_id = make_default_id_lineno(&path);

    let wrange = get_range(&path, cfs)?;

    let header = wrange.rows().next().context(EmptyExcelSnafu {})?;
    debug!("read_msforms_likert: header: {:?}", header);

    // Find the mapping between the columns and the candidate names.
    // Every candidate should have its name associated to a column
    let col_indexes = get_col_index(candidate_names, header)?;

    debug!("read_msforms_likert: col_indexes: {:?}", col_indexes);

    let ranked_choices: HashMap<String, u32> = get_ranked_choices(cfs)?.iter().cloned().collect();

    debug!("read_msforms_likert: ranked_choices: {:?}", ranked_choices);

    let mut iter = wrange.rows();
    // TODO check for correctness
    // Not looking at configuration for now: dropping the first column (id) and assuming that the last column is the weight.
    iter.next();
    let mut res: Vec<ParsedBallot> = Vec::new();
    for (idx, row) in iter.enumerate() {
        debug!("read_msforms_likert: idx: {:?} row: {:?}", idx, &row);

        let mut choices: Vec<(String, u32)> = Vec::new();

        for (idx, cand_name) in col_indexes.iter() {
            let v: calamine::DataType = row.get(*idx).cloned().context(EmptyExcelSnafu {})?;
            match v {
                calamine::DataType::String(s) => {
                    let choice_index = ranked_choices
                        .get(&s)
                        .cloned()
                        .context(EmptyExcelSnafu {})?;
                    choices.push((cand_name.clone(), choice_index as u32));
                }
                calamine::DataType::Empty => {
                    // No choice made, skip.
                }
                _ => {
                    return Err(Box::new(RcvError::ExcelWrongCellType {
                        lineno: *idx as u64,
                        content: format!("{:?} IN {:?}", v, row),
                    }));
                }
            };
        }

        debug!(
            "read_msforms_likert: idx: {:?} choices: {:?} row: {:?}",
            idx, &choices, &row
        );

        let choices_parsed = assemble_choices(&choices);

        let pb = ParsedBallot {
            id: Some(default_id(idx)),
            // MS forms are not expected to handle weights for the time being.
            count: Some(1),
            choices: choices_parsed,
        };
        res.push(pb);
    }
    Ok(res)
}

pub fn read_msforms_likert_transpose(
    path: String,
    cfs: &FileSource,
) -> BRcvResult<Vec<ParsedBallot>> {
    let default_id = make_default_id_lineno(&path);
    let wrange = get_range(&path, cfs)?;

    let header = wrange.rows().next().context(EmptyExcelSnafu {})?;
    debug!("read_msforms_likert_transpose: header: {:?}", header);

    let ranked_choices: Vec<(String, u32)> = get_ranked_choices(cfs)?;
    let choice_names: Vec<String> = ranked_choices.iter().map(|p| p.0.clone()).collect();

    debug!(
        "read_msforms_likert_transpose: ranked_choices: {:?}",
        ranked_choices
    );

    // Find the mapping between the columns and the candidate names.
    // Every candidate should have its name associated to a column
    let col_indexes = get_col_index_choices(&choice_names, header)?;

    debug!(
        "read_msforms_likert_transpose: col_indexes: {:?}",
        col_indexes
    );

    let mut iter = wrange.rows();
    // TODO check for correctness
    // Not looking at configuration for now: dropping the first column (id) and assuming that the last column is the weight.
    iter.next();
    let mut res: Vec<ParsedBallot> = Vec::new();
    for (idx, row) in iter.enumerate() {
        debug!(
            "read_msforms_likert_transpose: idx: {:?} row: {:?}",
            idx, &row
        );

        let mut choices: Vec<(String, u32)> = Vec::new();
        for (col_idx, rank) in col_indexes.iter() {
            let v: calamine::DataType = row.get(*col_idx).cloned().context(EmptyExcelSnafu {})?;
            match v {
                calamine::DataType::String(cand_name) => {
                    choices.push((cand_name.clone(), *rank));
                }
                calamine::DataType::Empty => {
                    // No choice made, skip.
                }
                _ => {
                    return Err(Box::new(RcvError::ExcelWrongCellType {
                        lineno: idx as u64,
                        content: format!("{:?} IN {:?}", v, row),
                    }));
                }
            };
        }
        debug!(
            "read_msforms_likert_transpose: idx: {:?} choices: {:?}",
            idx, &choices
        );
        let choices_parsed = assemble_choices(&choices);

        let pb = ParsedBallot {
            id: Some(default_id(idx)),
            // MS forms are not expected to handle weights for the time being.
            count: Some(1),
            choices: choices_parsed,
        };
        res.push(pb);
    }
    Ok(res)
}

/// Given the header of a file (names of each of the columns), and the names of the candidates,
/// finds the mapping from each candidate to a column index position.
pub fn get_col_index_mapping(
    req_col_names: &[String],
    header: &[Option<String>],
) -> BRcvResult<Vec<(usize, String)>> {
    let col_names: HashMap<String, usize> = header
        .iter()
        .enumerate()
        .filter_map(|(idx, x)| x.as_ref().map(|s| (s.clone(), idx)))
        .collect();

    debug!("read_msforms_likert: col_names: {:?}", col_names);

    let mut col_indexes: Vec<(usize, String)> = Vec::new();
    for cname in req_col_names {
        let idx = col_names
            .get(cname)
            .context(ExcelCannotFindCandidateInHeaderSnafu {
                candidate_name: cname,
            })?;
        col_indexes.push((*idx, cname.clone()));
    }
    Ok(col_indexes)
}

fn get_col_index(
    req_col_names: &[String],
    header: &[DataType],
) -> BRcvResult<Vec<(usize, String)>> {
    let remapped: Vec<Option<String>> = header
        .iter()
        .map(|dt| match dt {
            calamine::DataType::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    get_col_index_mapping(req_col_names, &remapped)
}

// Maps a column index to a rank
fn get_col_index_choices(
    choice_names: &[String],
    header: &[DataType],
) -> BRcvResult<Vec<(usize, u32)>> {
    // Find the mapping between the columns and the candidate names.
    // Every candidate should have its name associated to a column
    let col_names: HashMap<String, usize> = header
        .iter()
        .enumerate()
        .filter_map(|(idx, x)| match x {
            calamine::DataType::String(s) => Some((s.clone(), idx)),
            _ => None,
        })
        .collect();

    debug!("read_msforms_likert: col_names: {:?}", col_names);

    let mut col_indexes: Vec<(usize, u32)> = Vec::new();
    for (idx, cname) in choice_names.iter().enumerate() {
        let col_idx = col_names
            .get(cname)
            .context(ExcelCannotFindCandidateInHeaderSnafu {
                candidate_name: cname,
            })?;
        col_indexes.push((*col_idx, (idx + 1) as u32));
    }
    Ok(col_indexes)
}

fn get_ranked_choices(cfs: &FileSource) -> BRcvResult<Vec<(String, u32)>> {
    let res = cfs
        .choices
        .clone()
        .context(MissingChoicesSnafu {})?
        .iter()
        .enumerate()
        // The ranks start at 1
        .map(|(idx, s)| (s.clone(), (idx + 1) as u32))
        .collect();
    Ok(res)
}

fn get_range(path: &String, cfs: &FileSource) -> BRcvResult<calamine::Range<DataType>> {
    let worksheet_name_o = cfs.excel_worksheet_name.clone();
    debug!(
        "read_excel_file: path: {:?} worksheet: {:?}",
        &path, &worksheet_name_o
    );
    let p = path.clone();
    let mut workbook: Xlsx<_> =
        open_workbook(p).context(OpeningExcelSnafu { path: path.clone() })?;

    // A worksheet name was provided, use it.
    if let Some(worksheet_name) = worksheet_name_o {
        let wrange = workbook
            .worksheet_range(&worksheet_name)
            .context(EmptyExcelSnafu {})?
            .context(OpeningExcelSnafu { path: path.clone() })?;

        Ok(wrange)
    } else {
        let all_worksheets = workbook.worksheets();
        match all_worksheets.as_slice() {
            [] => unimplemented!("Empty worksheet"),
            [(worksheet_name, wrange)] => {
                debug!(
                    "read_excel_file: path: {:?} worksheet: {:?}",
                    &path, &worksheet_name
                );
                Ok(wrange.clone())
            }
            _ => {
                unimplemented!(
                    "read_excel_file: too many worksheets, the worksheet name must be provided"
                );
            }
        }
    }
}
