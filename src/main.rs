pub mod rcv;

use std::process::ExitCode;

use crate::rcv::test_wrapper;

use clap::Parser;

/// This is a ranked voting tabulation program.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// First argument
    #[clap(short, long, value_parser)]
    name: Option<String>,

    /// Number of times to greet
    #[clap(short, long, value_parser, default_value_t = 1)]
    count: u8,
}

const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    println!("This is timrcv version {}", VERSION.unwrap_or("unknown"));
    env_logger::init();

    let _args = Args::parse();

    test_wrapper("test_set_1_exhaust_at_overvote");
    ExitCode::SUCCESS

    // let r = rcv::run_election("/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data/duplicate_test/duplicate_test_config.json".to_string(),
    //  Some("/home/tjhunter/work/elections/rcv/src/test/resources/network/brightspots/rcv/test_data/duplicate_test/duplicate_test_expected_summary.json".to_string()));

    // if r.is_err() {
    //     ExitCode::FAILURE
    // } else {
    //     ExitCode::SUCCESS
    // }
}
