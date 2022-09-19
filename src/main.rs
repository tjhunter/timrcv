pub mod rcv;

use crate::rcv::run_election;
use crate::rcv::RcvResult;

use clap::Parser;

use env_logger::Env;

/// This is a ranked voting tabulation program.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// (file path, optional) The file containing the election data. (Only JSON election descriptions are currently supported)
    #[clap(short, long, value_parser)]
    config: String,
    /// (file path) A reference file containing the outcome of an election in JSON format. If provided, timrcv will
    /// check that the tabulated output matches the reference.
    #[clap(short, long, value_parser)]
    reference: Option<String>,

    /// (file path) If specified, the summary of the election will be written in JSON format to the given
    /// location. Setting this option overrides what may be specified with the --data option.
    #[clap(short, long, value_parser)]
    out: Option<String>,

    /// If passed as an argument, will turn on verbose logging to the standard output.
    #[clap(long, takes_value = false)]
    verbose: bool,
}

const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");

fn main() -> RcvResult<()> {
    println!("This is timrcv version {}", VERSION.unwrap_or("unknown"));
    println!("This software is not certificed. It may have some bugs. Do not use for official tabulation and certification of an election.");
    println!("For official needs, consider using RCTab https://www.rcvresources.org/rctab");

    let args = Args::parse();
    let env = Env::new().default_filter_or({
        if args.verbose {
            "debug"
        } else {
            "info"
        }
    });
    let _ = env_logger::try_init_from_env(env);

    run_election(args.config, args.reference, args.out, false).map(|_| ())
}
