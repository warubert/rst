use std::env;
use std::process::Command;

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

    let output = Command::new("echo")
        .arg("Comando echo executado pelo Rust")
        .output()
        .expect("Falha ao executar comando");

    if !output.status.success() {
        eprintln!("Comando terminou com erro: {}", output.status);
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("Saida do comando: {}", stdout);

    let ls_output = Command::new("ls")
        .output()
        .expect("Falha ao executar ls");

    if !ls_output.status.success() {
        eprintln!("Comando ls terminou com erro: {}", ls_output.status);
        return;
    }

    let ls_stdout = String::from_utf8_lossy(&ls_output.stdout);
    println!("Saida do ls:\n{}", ls_stdout);
}
