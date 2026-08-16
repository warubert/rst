use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Please provide a command-line argument.");
        return;
    }

    let input = args[1].clone();
    let reversed = input.chars().rev().collect::<String>();
    println!("You typed: {}", input);
    println!("Reversed: {}", reversed);
}
