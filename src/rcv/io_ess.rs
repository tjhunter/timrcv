use snafu::OptionExt;

use crate::rcv::{io_common::make_default_id_lineno, *};

pub fn read_excel_file(path: String, cfs: &FileSource) -> BRcvResult<Vec<ParsedBallot>> {
    let p = path.clone();
    let mut workbook: Xlsx<_> =
        open_workbook(p).context(OpeningExcelSnafu { path: path.clone() })?;
    let wrange = workbook
        .worksheet_range_at(0)
        .context(EmptyExcelSnafu {})?
        .context(OpeningExcelSnafu { path: path.clone() })?;

    let default_id = make_default_id_lineno(&path);

    let header = wrange.rows().next().context(EmptyExcelSnafu {})?;
    debug!("read_excel_file: header: {:?}", header);
    let start_range = cfs.first_vote_column_index()?;
    debug!("read_excel_file: start_range: {:?}", start_range);

    let mut iter = wrange.rows();
    // TODO check for correctness
    iter.next();
    let mut res: Vec<ParsedBallot> = Vec::new();
    for (idx, row) in iter.enumerate() {
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
            id: Some(default_id(idx)),
            count,
            choices: cs,
        };
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
