//! M3: ゲートレベル エミュレータのテスト(docs/gates.md が仕様)。
//! 正解は M1 の数値ライブラリ(Word::add_carry)。

use tern_emu::asm::assemble;
use tern_emu::cpu::Cpu;
use tern_emu::gates::{
    any, binary_ripple_adder, buf, cons, decode, eval_adder, full_adder, ripple_adder, sum,
    AdderOptions, Circuit, Cost, GateAlu, Prim,
};
use tern_emu::{Trit, Word, Word9};

const T: [Trit; 3] = Trit::ALL;
const OPTS: [AdderOptions; 2] = [
    AdderOptions { restore: false },
    AdderOptions { restore: true },
];

/// 2 入力の複合ゲートを 1 つだけ持つ回路を作り、9 通りの真理値表を返す。
fn table2(
    build: impl Fn(&mut Circuit, tern_emu::gates::Net, tern_emu::gates::Net) -> tern_emu::gates::Net,
) -> Vec<Trit> {
    let mut c = Circuit::new();
    let a = c.input("a");
    let b = c.input("b");
    let y = build(&mut c, a, b);
    c.output("y", y);
    T.iter()
        .flat_map(|&x| T.iter().map(move |&y| (x, y)))
        .map(|(x, y)| c.run(&[x, y])[0])
        .collect()
}

fn expected2(f: impl Fn(i8, i8) -> i8) -> Vec<Trit> {
    T.iter()
        .flat_map(|&x| T.iter().map(move |&y| (x, y)))
        .map(|(x, y)| Trit::from_i8(f(x as i8, y as i8)).unwrap())
        .collect()
}

// --- 基本ゲート・複合ゲート ---

#[test]
fn primitives_match_trit_functions() {
    for x in T {
        for (p, f) in [
            (Prim::Sti, Trit::sti as fn(Trit) -> Trit),
            (Prim::Nti, Trit::nti),
            (Prim::Pti, Trit::pti),
        ] {
            let mut c = Circuit::new();
            let i = c.input("x");
            let y = c.gate(p, &[i]);
            c.output("y", y);
            assert_eq!(c.run(&[x])[0], f(x));
        }
    }
    assert_eq!(table2(|c, a, b| c.min(&[a, b])), expected2(|a, b| a.min(b)));
    assert_eq!(table2(|c, a, b| c.max(&[a, b])), expected2(|a, b| a.max(b)));
}

#[test]
fn decoder_is_one_hot() {
    for x in T {
        let mut c = Circuit::new();
        let i = c.input("x");
        let d = decode(&mut c, i);
        c.output("n", d.n);
        c.output("z", d.z);
        c.output("p", d.p);
        let out = c.run(&[x]);
        let want: Vec<Trit> = T
            .iter()
            .map(|&v| if v == x { Trit::P } else { Trit::N })
            .collect();
        assert_eq!(out, want, "x={x:?}");
    }
}

#[test]
fn sum_gate() {
    let got = table2(|c, a, b| {
        let (da, db) = (decode(c, a), decode(c, b));
        sum(c, da, db)
    });
    assert_eq!(got, expected2(|a, b| (a + b + 4) % 3 - 1));
}

#[test]
fn cons_gate() {
    assert_eq!(table2(cons), expected2(|a, b| if a == b { a } else { 0 }));
}

#[test]
fn any_gate() {
    let f = |a: i8, b: i8| {
        if a == 0 {
            b
        } else if b == 0 || a == b {
            a
        } else {
            0
        }
    };
    assert_eq!(table2(any), expected2(f));
}

#[test]
fn buf_is_identity() {
    assert_eq!(table2(|c, a, _| buf(c, a)), expected2(|a, _| a));
}

#[test]
fn cons_uses_no_transistors() {
    let mut c = Circuit::new();
    let (a, b) = (c.input("a"), c.input("b"));
    cons(&mut c, a, b);
    assert_eq!(c.cost().transistors, 0);
}

// --- 加算器 ---

#[test]
fn full_adder_all_27() {
    for opt in OPTS {
        let mut c = Circuit::new();
        let (a, b, ci) = (c.input("a"), c.input("b"), c.input("cin"));
        let (s, co) = full_adder(&mut c, a, b, ci, opt);
        c.output("s", s);
        c.output("cout", co);
        for x in T {
            for y in T {
                for z in T {
                    let out = c.run(&[x, y, z]);
                    assert_eq!(x as i8 + y as i8 + z as i8, out[0] as i8 + 3 * out[1] as i8);
                }
            }
        }
    }
}

fn adder_exhaustive<const N: usize>(opt: AdderOptions) {
    let c = ripple_adder(N, opt);
    let mut vals = Vec::new();
    let h = Word::<N>::MAX;
    for x in -h..=h {
        let a = Word::<N>::from_i64(x).unwrap();
        for y in -h..=h {
            let b = Word::<N>::from_i64(y).unwrap();
            for cin in T {
                assert_eq!(
                    eval_adder(&c, a, b, cin, &mut vals),
                    a.add_carry(b, cin),
                    "{x}+{y}+{cin:?}"
                );
            }
        }
    }
}

#[test]
fn ripple_adder_exhaustive_1_to_4_trits() {
    for opt in OPTS {
        adder_exhaustive::<1>(opt);
        adder_exhaustive::<2>(opt);
        adder_exhaustive::<3>(opt);
        adder_exhaustive::<4>(opt);
    }
}

/// 729 x 729 x 3 ≈ 160 万通り
#[test]
fn ripple_adder_exhaustive_6_trits() {
    adder_exhaustive::<6>(AdderOptions { restore: true });
}

/// 19683² ≈ 3.9 億通り。重いので通常は除外: cargo test --release -- --ignored
#[test]
#[ignore]
fn ripple_adder_exhaustive_9_trits() {
    let h = Word9::MAX;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()) as i64;
    std::thread::scope(|s| {
        for k in 0..threads {
            s.spawn(move || {
                let c = ripple_adder(9, AdderOptions { restore: true });
                let mut vals = Vec::new();
                let mut x = -h + k;
                while x <= h {
                    let a = Word9::from_i64(x).unwrap();
                    for y in -h..=h {
                        let b = Word9::from_i64(y).unwrap();
                        assert_eq!(
                            eval_adder(&c, a, b, Trit::Z, &mut vals),
                            a.add_carry(b, Trit::Z)
                        );
                    }
                    x += threads;
                }
            });
        }
    });
}

fn xorshift(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

#[test]
fn ripple_adder_9_trits_sampled() {
    let h = Word9::MAX as u64;
    let mut r = 12345u64;
    for opt in OPTS {
        let c = ripple_adder(9, opt);
        let mut vals = Vec::new();
        for _ in 0..100_000 {
            let a = Word9::from_i64((xorshift(&mut r) % (2 * h + 1)) as i64 - h as i64).unwrap();
            let b = Word9::from_i64((xorshift(&mut r) % (2 * h + 1)) as i64 - h as i64).unwrap();
            let cin = T[(xorshift(&mut r) % 3) as usize];
            assert_eq!(eval_adder(&c, a, b, cin, &mut vals), a.add_carry(b, cin));
        }
    }
}

/// 2 進加算器: - を 0、+ を 1 として、整数の加算と一致すること
fn binary_check(n: usize, opt: AdderOptions, pairs: impl Iterator<Item = (u64, u64, u64)>) {
    let c = binary_ripple_adder(n, opt);
    let bit = |v: u64, i: usize| if v >> i & 1 == 1 { Trit::P } else { Trit::N };
    for (x, y, ci) in pairs {
        let mut ins: Vec<Trit> = (0..n).map(|i| bit(x, i)).collect();
        ins.extend((0..n).map(|i| bit(y, i)));
        ins.push(bit(ci, 0));
        let out = c.run(&ins);
        let got: u64 = out
            .iter()
            .enumerate()
            .map(|(i, &t)| ((t == Trit::P) as u64) << i)
            .sum();
        assert_eq!(got, x + y + ci);
    }
}

#[test]
fn binary_adder() {
    for opt in OPTS {
        binary_check(
            4,
            opt,
            (0..16).flat_map(|x| (0..16).flat_map(move |y| (0..2).map(move |c| (x, y, c)))),
        );
        let mut r = 777u64;
        let v: Vec<_> = (0..20_000)
            .map(|_| {
                (
                    xorshift(&mut r) & 0x7FFF,
                    xorshift(&mut r) & 0x7FFF,
                    xorshift(&mut r) & 1,
                )
            })
            .collect();
        binary_check(15, opt, v.into_iter());
    }
}

// --- 指標 ---

#[test]
fn cost_model_and_breakdown() {
    let c = ripple_adder(1, AdderOptions { restore: false });
    let by_block = c.cost_by_block();
    // デコーダ 4 個(a, b, s1, cin)× トランジスタ 4
    assert_eq!(by_block["DEC"].1.transistors, 16);
    assert_eq!(by_block["ANY"].1.transistors, 4);
    assert_eq!(by_block["SUM"].1.transistors, 0);
    let total = by_block
        .values()
        .fold(Cost::default(), |a, (_, k)| a + *k);
    assert_eq!(total, c.cost());
    assert_eq!(
        ripple_adder(9, AdderOptions::default()).cost().transistors,
        9 * c.cost().transistors
    );
}

#[test]
fn restore_bounds_passive_depth() {
    let on = AdderOptions { restore: true };
    let off = AdderOptions { restore: false };
    // レベル再生ありなら段数を増やしても受動段数は一定
    assert_eq!(
        ripple_adder(3, on).passive_depth(),
        ripple_adder(9, on).passive_depth()
    );
    // なしだとキャリーの経路で伸びる
    assert!(ripple_adder(9, off).passive_depth() > ripple_adder(3, off).passive_depth());
    assert!(
        binary_ripple_adder(15, off).passive_depth() > binary_ripple_adder(3, off).passive_depth()
    );
}

#[test]
fn binary_net_observation() {
    let bin = binary_ripple_adder(2, AdderOptions::default());
    let vecs: Vec<Vec<Trit>> = (0..32u32)
        .map(|v| {
            (0..5)
                .map(|i| if v >> i & 1 == 1 { Trit::P } else { Trit::N })
                .collect()
        })
        .collect();
    let o = bin.observe(vecs.iter().map(|v| v.as_slice()));
    assert_eq!(o.binary_nets, o.gate_nets);

    // 3 進の全加算器は、デコーダの出力などが 2 値、和やキャリーは 3 値
    let tern = ripple_adder(1, AdderOptions::default());
    let vecs: Vec<Vec<Trit>> = T
        .iter()
        .flat_map(|&a| {
            T.iter()
                .flat_map(move |&b| T.iter().map(move |&c| vec![a, b, c]))
        })
        .collect();
    let o = tern.observe(vecs.iter().map(|v| v.as_slice()));
    assert_eq!(o.vectors, 27);
    assert!(o.binary_nets > 0 && o.binary_nets < o.gate_nets);
}

// --- ISA との接続 ---

#[test]
fn gate_alu_runs_sample_programs() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../programs");
    for name in ["sum3x5.t3asm", "countdown.t3asm", "multiply.t3asm"] {
        let prog = assemble(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap();
        let mut soft = Cpu::new();
        soft.load(prog.image.clone());
        soft.run(10_000).unwrap();
        for opt in OPTS {
            let mut gate = Cpu::with_alu(GateAlu::new(opt));
            gate.load(prog.image.clone());
            gate.run(10_000).unwrap();
            assert_eq!(gate.output, soft.output, "{name}");
            assert_eq!(
                (gate.acc, gate.carry, gate.steps),
                (soft.acc, soft.carry, soft.steps)
            );
        }
    }
}
