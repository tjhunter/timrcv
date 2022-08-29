use env_logger;
use ranked_voting;

fn main() {
    env_logger::init();
    let num = 10;
    println!(
        "Hello, world! {num} plus one is {}!",
        ranked_voting::add_one(num)
    );
}
