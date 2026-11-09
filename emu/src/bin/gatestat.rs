//! ゲートレベル回路の部品数・段数・2 値信号線の割合を集計する(docs/gates.md 5 章)。
//!
//! `gatestat [--breakdown]`

use tern_emu::gates::{
    binary_full_adder, binary_ripple_adder, full_adder, ripple_adder, AdderOptions, Circuit, Net,
};
use tern_emu::Trit;

/// 全網羅の入力ベクタ(値の候補 `alphabet` の直積)
fn exhaustive(n: usize, alphabet: &[Trit]) -> Vec<Vec<Trit>> {
    let mut out = vec![vec![]];
    for _ in 0..n {
        out = out
            .into_iter()
            .flat_map(|v| {
                alphabet.iter().map(move |&t| {
                    let mut w = v.clone();
                    w.push(t);
                    w
                })
            })
            .collect();
    }
    out
}

/// 決定的な疑似乱数による入力ベクタ
fn sampled(n: usize, alphabet: &[Trit], count: usize) -> Vec<Vec<Trit>> {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    (0..count)
        .map(|_| {
            (0..n)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    alphabet[(x % alphabet.len() as u64) as usize]
                })
                .collect()
        })
        .collect()
}

fn cell(f: impl Fn(&mut Circuit, Net, Net, Net)) -> Circuit {
    let mut c = Circuit::new();
    let a = c.input("a");
    let b = c.input("b");
    let cin = c.input("cin");
    f(&mut c, a, b, cin);
    c
}

struct Row {
    name: String,
    c: Circuit,
    vectors: Vec<Vec<Trit>>,
    vectors_note: &'static str,
}

fn main() {
    let breakdown = std::env::args().any(|a| a == "--breakdown");
    let tern = &Trit::ALL[..];
    let bin = &[Trit::N, Trit::P][..];
    let mut rows = Vec::new();

    for restore in [false, true] {
        let opt = AdderOptions { restore };
        let tag = if restore {
            "再生あり"
        } else {
            "再生なし"
        };
        let fa = cell(|c, a, b, cin| {
            let (s, co) = full_adder(c, a, b, cin, opt);
            c.output("s", s);
            c.output("cout", co);
        });
        rows.push(Row {
            name: format!("3進 全加算器 ({tag})"),
            c: fa,
            vectors: exhaustive(3, tern),
            vectors_note: "全網羅",
        });
        let bfa = cell(|c, a, b, cin| {
            let (s, co) = binary_full_adder(c, a, b, cin, opt);
            c.output("s", s);
            c.output("cout", co);
        });
        rows.push(Row {
            name: format!("2進 全加算器 ({tag})"),
            c: bfa,
            vectors: exhaustive(3, bin),
            vectors_note: "全網羅",
        });
        rows.push(Row {
            name: format!("3進 9トリット ({tag})"),
            c: ripple_adder(9, opt),
            vectors: sampled(19, tern, 20_000),
            vectors_note: "2万件",
        });
        rows.push(Row {
            name: format!("2進 15ビット ({tag})"),
            c: binary_ripple_adder(15, opt),
            vectors: sampled(31, bin, 20_000),
            vectors_note: "2万件",
        });
    }

    println!("| 回路 | トランジスタ | ダイオード | 抵抗 | 部品計 | 受動段数 | ゲート段数 | 能動段数 | 2値信号線 |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|");
    for r in &rows {
        let k = r.c.cost();
        let o = r.c.observe(r.vectors.iter().map(|v| v.as_slice()));
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {:.0}% ({}/{}, {}) |",
            r.name,
            k.transistors,
            k.diodes,
            k.resistors,
            k.total(),
            r.c.passive_depth(),
            r.c.gate_levels(),
            r.c.active_levels(),
            o.binary_ratio() * 100.0,
            o.binary_nets,
            o.gate_nets,
            r.vectors_note,
        );
    }

    if breakdown {
        for r in rows.iter().filter(|r| r.name.contains("全加算器")) {
            println!("\n### {}\n", r.name);
            println!("| 部品 | ゲート数 | トランジスタ | ダイオード | 抵抗 |");
            println!("|---|---:|---:|---:|---:|");
            for (block, (n, k)) in r.c.cost_by_block() {
                println!(
                    "| {block} | {n} | {} | {} | {} |",
                    k.transistors, k.diodes, k.resistors
                );
            }
        }
    }
}
