// Primitives for reading CSV files.

use std::fs::File;

use crate::rcv::io_common::{assemble_choices, make_default_id_lineno};
use crate::rcv::io_msforms::get_col_index_mapping;
use crate::rcv::*;

pub fn read_csv_ranking(path: String, cfs: &FileSource) -> BRcvResult<Vec<ParsedBallot>> {
    let get_id = make_get_id(&path);

    let id_idx_o = cfs.id_column_index_int()?;
    let choices_start_col = cfs.first_vote_column_index()?;
    let count_idx_o = cfs.count_column_index_int()?;

    let mut res: Vec<ParsedBallot> = Vec::new();
    // No header expected in the simple format
    let (records, row_offset) = get_records(&path, cfs)?;

    for (idx, line_r) in records.enumerate() {
        let lineno = idx + row_offset + 1;
        debug!("{:?} {:?}", lineno, line_r);
        let line = line_r.context(CsvLineParseSnafu {})?;
        let id = get_id(&line, &id_idx_o, lineno)?;

        let count = get_count_csv(&line, &count_idx_o, lineno)?;

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

pub fn read_csv_likert(
    path: String,
    cfs: &FileSource,
    candidate_names: &[String],
) -> BRcvResult<Vec<ParsedBallot>> {
    let get_id = make_get_id(&path);

    let id_idx_o = cfs.id_column_index_int()?;
    let count_idx_o = cfs.count_column_index_int()?;

    let mappings: Vec<(usize, String)> = {
        // has_header=false because we want to read the header
        let reader = get_reader(&path)?;
        let header_r = reader.into_records().next().context(CsvEmptySnafu {})?;
        let header = header_r.context(CsvLineParseSnafu {})?;
        let col_names: Vec<Option<String>> =
            header.into_iter().map(|s| Some(s.to_string())).collect();
        get_col_index_mapping(candidate_names, &col_names)?
    };
    debug!("read_csv_likert: mappings: {:?}", &mappings);

    let mut res: Vec<ParsedBallot> = Vec::new();

    let (records, row_offset) = get_records(&path, cfs)?;
    for (idx, line_r) in records.enumerate() {
        let lineno = idx + row_offset + 1;
        debug!("{:?} {:?}", lineno, line_r);
        let line = line_r.context(CsvLineParseSnafu {})?;
        let id = get_id(&line, &id_idx_o, lineno)?;
        let count = get_count_csv(&line, &count_idx_o, lineno)?;

        let mut ranks: Vec<(String, u32)> = Vec::new();
        for (pos, cname) in mappings.iter() {
            let rank_str = line
                .get(*pos)
                .context(CsvLineToShortSnafu { lineno })?
                .trim();
            if !rank_str.is_empty() {
                let rank = rank_str
                    .parse::<u32>()
                    // TODO: could return the parsing error as well
                    .ok()
                    .context(LineParseSnafu { lineno, col: *pos })?;
                ranks.push((cname.clone(), rank));
            }
        }

        let choices_parsed = assemble_choices(&ranks);

        let pb = ParsedBallot {
            id: Some(id),
            count,
            choices: choices_parsed,
        };
        res.push(pb);
    }
    Ok(res)
}

fn get_count_csv(
    line: &csv::StringRecord,
    count_idx_o: &Option<usize>,
    lineno: usize,
) -> RcvResult<Option<u64>> {
    let count: Option<u64> = if let Some(count_idx) = count_idx_o {
        line.get(count_idx - 1)
            .context(CsvLineToShortSnafu { lineno })?
            .parse::<u64>()
            .ok()
            .map(Some)
            .context(LineParseSnafu {
                lineno,
                col: *count_idx,
            })?
    } else {
        Some(1)
    };
    Ok(count)
}

fn make_get_id(
    path: &str,
) -> impl Fn(&csv::StringRecord, &Option<usize>, usize) -> RcvResult<String> {
    let default_id = make_default_id_lineno(path);
    move |line, id_idx_o, lineno| {
        let id = if let Some(id_idx) = id_idx_o {
            line.get(*id_idx)
                .context(CsvLineToShortSnafu { lineno })?
                .to_string()
        } else {
            default_id(lineno)
        };
        Ok(id)
    }
}

fn get_reader(path: &String) -> RcvResult<csv::Reader<File>> {
    csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)
        .context(CsvOpenSnafu {})
}

fn get_records(
    path: &String,
    cfs: &FileSource,
) -> RcvResult<(csv::StringRecordsIntoIter<File>, usize)> {
    let first_row = cfs.first_vote_row_index()?;
    debug!("get_records: first_row: {:?}", first_row);
    let reader = get_reader(path)?;
    let mut records = reader.into_records();
    // The index starts at 1 to respect most conventions in the excel world
    for _ in 0..first_row {
        _ = records.next();
    }
    Ok((records, first_row))
}
