// Primitives for reading CSV files.

use std::collections::HashMap;
use std::fs::File;

use csv::Reader;

use crate::rcv::{
    io_common::{assemble_choices, simplify_file_name},
    *,
};

pub fn read_csv_ranking(path: String, cfs: &FileSource) -> BRcvResult<Vec<ParsedBallot>> {
    let default_id = make_default_id(&path);

    let id_idx_o = cfs.id_column_index_int()?;
    let choices_start_col = cfs.first_vote_column_index()?;
    let count_idx_o = cfs.count_column_index_int()?;

    let mut res: Vec<ParsedBallot> = Vec::new();
    let (records, row_offset) = get_records(&path, cfs)?;

    for (idx, line_r) in records.enumerate() {
        let lineno = idx + row_offset + 1;
        debug!("{:?} {:?}", lineno, line_r);
        let line = line_r.context(CsvLineParseSnafu {})?;
        let id = if let Some(id_idx) = id_idx_o {
            line.get(id_idx)
                .context(CsvLineToShortSnafu { lineno })?
                .to_string()
        } else {
            default_id(lineno)
        };

        let count: Option<u64> = if let Some(count_idx) = count_idx_o {
            line.get(count_idx - 1)
                .context(CsvLineToShortSnafu { lineno })?
                .parse::<u64>()
                .ok()
                .map(Some)
                // TODO: this is the wrong error to return here
                .context(CsvLineToShortSnafu { lineno })?
        } else {
            Some(1)
        };

        let choices_parsed: Vec<Vec<String>> = line
            .iter()
            .skip(choices_start_col)
            .map(|s| vec![s.to_string()])
            .collect();
        debug!(
            "read_csv_ranking: lineno: {:?} row: {:?}",
            lineno, &choices_parsed
        );

        let pb = ParsedBallot {
            id: Some(id),
            count,
            choices: choices_parsed,
        };
        res.push(pb);
    }
    Ok(res)
}

fn get_records(
    path: &String,
    cfs: &FileSource,
) -> RcvResult<(csv::StringRecordsIntoIter<File>, usize)> {
    let first_row = cfs.first_vote_row_index()?;
    let rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)
        .context(CsvOpenSnafu {})?;
    let mut records = rdr.into_records();
    // The index starts at 1 to respect most conventions in the excel world
    for _ in 1..first_row {
        _ = records.next();
    }
    Ok((records, first_row))
}

fn make_default_id(path: &String) -> impl Fn(usize) -> String {
    let simplified_file_name = simplify_file_name(path.as_str());
    move |lineno| format!("{}-{:08}", simplified_file_name, lineno)
}
