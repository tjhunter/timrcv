pub mod rcv;

use env_logger;
use std::process::ExitCode;

const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    println!(
        "This is ranking_vote version {}",
        VERSION.unwrap_or("unknown")
    );
    env_logger::init();

    let r = rcv::run_election("/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data/duplicate_test/duplicate_test_config.json".to_string(),
     Some("/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data/duplicate_test/duplicate_test_expected_summary.json".to_string()));

    if r.is_err() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
