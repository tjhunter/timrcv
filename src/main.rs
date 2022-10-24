use clap::Parser;
use env_logger::Env;

mod args;
pub mod rcv;
use crate::args::Args;
use crate::rcv::run_election;
use crate::rcv::RcvResult;

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

    let args2 = args.clone();

    run_election(
        args.config,
        args.reference,
        args.input,
        args.out,
        false,
        Some(args2),
    )
    .map(|_| ())
}
