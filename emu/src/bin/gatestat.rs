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

/// 1 桁分の回路(入力 a, b, cin / 出力 s, cout)
fn cell(f: impl Fn(&mut Circuit, Net, Net, Net) -> (Net, Net)) -> Circuit {
    let mut c = Circuit::new();
    let a = c.input("a");
    let b = c.input("b");
    let cin = c.input("cin");
    let (s, co) = f(&mut c, a, b, cin);
    c.output("s", s);
    c.output("cout", co);
    c
}

struct Row {
    name: String,
    /// 1 桁分(2 値信号線の割合を全網羅で測る)
    cell: Circuit,
    alphabet: &'static [Trit],
    /// 比較用のワード幅の加算器
    word: Circuit,
}

fn main() {
    let breakdown = std::env::args().any(|a| a == "--breakdown");
    const TERN: &[Trit] = &Trit::ALL;
    const BIN: &[Trit] = &[Trit::N, Trit::P];

    let o = |restore, reuse_s1, enc| AdderOptions {
        restore,
        reuse_s1,
        enc,
    };
    let ternary = [
        o(false, false, false),
        o(true, false, false),
        o(false, true, false),
        o(true, true, false),
        o(false, false, true),
        o(false, true, true),
    ];

    let mut rows = Vec::new();
    for opt in ternary {
        rows.push(Row {
            name: format!("3進 9トリット ({})", opt.label()),
            cell: cell(|c, a, b, ci| full_adder(c, a, b, ci, opt)),
            alphabet: TERN,
            word: ripple_adder(9, opt),
        });
    }
    for restore in [false, true] {
        let opt = o(restore, false, false);
        rows.push(Row {
            name: format!("2進 15ビット ({})", opt.label()),
            cell: cell(|c, a, b, ci| binary_full_adder(c, a, b, ci, opt)),
            alphabet: BIN,
            word: binary_ripple_adder(15, opt),
        });
    }

    println!("加算器全体(3進 9トリット / 2進 15ビット)。2値信号線(0 を一度もとらない配線の割合)と 3値配線(-,0,+ すべてをとる配線の数)は 1 桁分を全網羅で測定\n");
    println!("| 構成 | トランジスタ | ダイオード | 抵抗 | 部品計 | 受動段数 | ゲート段数 | 能動段数 | 2値信号線 | 3値配線/桁 |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    for r in &rows {
        let k = r.word.cost();
        let vecs = exhaustive(3, r.alphabet);
        let obs = r.cell.observe(vecs.iter().map(|v| v.as_slice()));
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {:.0}% | {}/{} |",
            r.name,
            k.transistors,
            k.diodes,
            k.resistors,
            k.total(),
            r.word.passive_depth(),
            r.word.gate_levels(),
            r.word.active_levels(),
            obs.binary_ratio() * 100.0,
            obs.ternary_nets,
            obs.gate_nets,
        );
    }

    if breakdown {
        for r in &rows {
            println!("\n### 1 桁の内訳: {}\n", r.name);
            println!("| 部品 | ゲート数 | トランジスタ | ダイオード | 抵抗 |");
            println!("|---|---:|---:|---:|---:|");
            for (block, (n, k)) in r.cell.cost_by_block() {
                println!(
                    "| {block} | {n} | {} | {} | {} |",
                    k.transistors, k.diodes, k.resistors
                );
            }
            let k = r.cell.cost();
            println!(
                "| 計 | {} | {} | {} | {} |",
                r.cell.num_gates(),
                k.transistors,
                k.diodes,
                k.resistors
            );
        }
    }
}
