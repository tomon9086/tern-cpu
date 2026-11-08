//! コマンドライン実行: `t3run programs/sum3x5.t3asm [--trace] [--list] [--max-steps N]`

use std::process::ExitCode;

use tern_emu::asm::assemble;
use tern_emu::cpu::{disassemble, Cpu};

const USAGE: &str = "usage: t3run <source.t3asm> [--trace] [--list] [--max-steps N]";

fn main() -> ExitCode {
    let mut source = None;
    let mut trace = false;
    let mut list = false;
    let mut max_steps: u64 = 100_000;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--trace" => trace = true,
            "--list" => list = true,
            "--max-steps" => match args.next().and_then(|s| s.parse().ok()) {
                Some(n) => max_steps = n,
                None => {
                    eprintln!("--max-steps needs a number\n{USAGE}");
                    return ExitCode::from(64);
                }
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ if source.is_none() && !a.starts_with("--") => source = Some(a),
            _ => {
                eprintln!("unexpected argument {a:?}\n{USAGE}");
                return ExitCode::from(64);
            }
        }
    }
    let Some(path) = source else {
        eprintln!("{USAGE}");
        return ExitCode::from(64);
    };

    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let prog = match assemble(&src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    if list {
        let mut names: Vec<(&i16, &String)> = prog.labels.iter().map(|(k, v)| (v, k)).collect();
        names.sort();
        for (&addr, &w) in &prog.image {
            let label: Vec<&str> = names
                .iter()
                .filter(|(a, _)| **a == addr)
                .map(|(_, n)| n.as_str())
                .collect();
            println!(
                "[{addr:+4}] {w} {:+6}  {:<10} {}",
                w.to_i64(),
                disassemble(w),
                label.join(" ")
            );
        }
        return ExitCode::SUCCESS;
    }

    let mut cpu = Cpu::new();
    cpu.load(prog.image);
    let result = if trace {
        cpu.run_traced(max_steps, |c, pc, w| println!("{}", c.format_trace(pc, w)))
    } else {
        cpu.run(max_steps)
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        return ExitCode::from(2);
    }
    for w in &cpu.output {
        println!("OUT {w} {:+}", w.to_i64());
    }
    println!("HALT after {} steps: {}", cpu.steps, cpu.state_line());
    ExitCode::SUCCESS
}
