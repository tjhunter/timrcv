use std::path::Path;

pub fn simplify_file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
}

pub fn assemble_choices(ranks: &[(String, u32)]) -> Vec<Vec<String>> {
    // TODO: print something when the ballot is completely empty
    let max_sels = ranks.iter().map(|(_, rank)| *rank).max().unwrap_or(0);
    let mut choices: Vec<Vec<String>> = vec![];
    for _ in 0..max_sels {
        choices.push(vec![]);
    }
    for (cname, rank) in ranks.iter() {
        if let Some(elt) = choices.get_mut((rank - 1) as usize) {
            elt.push(cname.clone());
        }
    }
    choices
}

pub fn get_count(num_votes: &[u64]) -> Option<u64> {
    // TODO: check that all the votes have the same weight
    num_votes.first().cloned()
}
