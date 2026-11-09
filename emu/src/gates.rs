//! ゲートレベル エミュレータ(M3)。仕様は docs/gates.md。
//!
//! [`Circuit`] は基本ゲート([`Prim`])のネットリスト。ゲートは追加順に並び、
//! 入力は常に既存の配線なので、追加順に評価すればよい(組み合わせ回路のみ)。

use std::collections::BTreeMap;
use std::ops::{Add, AddAssign};

use crate::cpu::{Alu, WORD_WIDTH};
use crate::trit::{Trit, Word, Word9};

// --- 基本ゲート ----------------------------------------------------------

/// 基本ゲート。MIN / MAX の引数は入力数。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Prim {
    Sti,
    Nti,
    Pti,
    /// 2 進比較用のインバータ(入力は - / + のみ)
    Bnot,
    /// 3 値の出力段 ENC(p, n)(実験的、ADR 0007)。入力は 2 値、p と n の同時 + は禁止
    Enc,
    Min(u8),
    Max(u8),
}

/// 部品数
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cost {
    pub transistors: u32,
    pub diodes: u32,
    pub resistors: u32,
}

impl Cost {
    pub const fn new(transistors: u32, diodes: u32, resistors: u32) -> Cost {
        Cost {
            transistors,
            diodes,
            resistors,
        }
    }

    pub fn total(self) -> u32 {
        self.transistors + self.diodes + self.resistors
    }
}

impl Add for Cost {
    type Output = Cost;
    fn add(self, o: Cost) -> Cost {
        Cost::new(
            self.transistors + o.transistors,
            self.diodes + o.diodes,
            self.resistors + o.resistors,
        )
    }
}

impl AddAssign for Cost {
    fn add_assign(&mut self, o: Cost) {
        *self = *self + o;
    }
}

impl Prim {
    /// 部品コスト(暫定。docs/gates.md 2.2。M4 で見直す)
    pub const fn cost(self) -> Cost {
        match self {
            Prim::Sti => Cost::new(2, 0, 3),
            Prim::Nti | Prim::Pti => Cost::new(1, 1, 2),
            Prim::Bnot => Cost::new(1, 0, 2),
            Prim::Enc => Cost::new(2, 0, 3),
            Prim::Min(n) | Prim::Max(n) => Cost::new(0, n as u32, 1),
        }
    }

    /// トランジスタを含み、レベルを再生するゲートか
    pub const fn is_active(self) -> bool {
        !matches!(self, Prim::Min(_) | Prim::Max(_))
    }

    pub const fn name(self) -> &'static str {
        match self {
            Prim::Sti => "STI",
            Prim::Nti => "NTI",
            Prim::Pti => "PTI",
            Prim::Bnot => "BNOT",
            Prim::Enc => "ENC",
            Prim::Min(_) => "MIN",
            Prim::Max(_) => "MAX",
        }
    }

    fn eval(self, ins: &[Trit]) -> Trit {
        match self {
            Prim::Sti => ins[0].sti(),
            Prim::Nti => ins[0].nti(),
            Prim::Pti => ins[0].pti(),
            Prim::Bnot => {
                debug_assert_ne!(ins[0], Trit::Z, "BNOT input must be binary");
                ins[0].sti()
            }
            Prim::Enc => {
                let (p, n) = (ins[0], ins[1]);
                debug_assert!(p != Trit::Z && n != Trit::Z, "ENC inputs must be binary");
                assert!(
                    !(p == Trit::P && n == Trit::P),
                    "ENC short circuit: p and n both on"
                );
                if p == Trit::P {
                    Trit::P
                } else if n == Trit::P {
                    Trit::N
                } else {
                    Trit::Z
                }
            }
            Prim::Min(_) => *ins.iter().min().expect("MIN needs inputs"),
            Prim::Max(_) => *ins.iter().max().expect("MAX needs inputs"),
        }
    }
}

// --- ネットリスト --------------------------------------------------------

/// 配線(ネット)の番号
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Net(u32);

impl Net {
    fn idx(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug)]
enum Driver {
    Input,
    Const(Trit),
    Gate,
}

#[derive(Clone, Debug)]
struct Gate {
    prim: Prim,
    inputs: Vec<Net>,
    out: Net,
    block: &'static str,
}

#[derive(Clone, Debug, Default)]
pub struct Circuit {
    drivers: Vec<Driver>,
    gates: Vec<Gate>,
    inputs: Vec<(String, Net)>,
    outputs: Vec<(String, Net)>,
    consts: [Option<Net>; 3],
    block: &'static str,
}

impl Circuit {
    pub fn new() -> Circuit {
        Circuit {
            block: "-",
            ..Default::default()
        }
    }

    fn new_net(&mut self, d: Driver) -> Net {
        self.drivers.push(d);
        Net(self.drivers.len() as u32 - 1)
    }

    pub fn input(&mut self, name: impl Into<String>) -> Net {
        let n = self.new_net(Driver::Input);
        self.inputs.push((name.into(), n));
        n
    }

    /// 定数(電源レール)。同じ値は 1 本の配線を共有する。
    pub fn konst(&mut self, t: Trit) -> Net {
        let slot = (t as i8 + 1) as usize;
        if let Some(n) = self.consts[slot] {
            return n;
        }
        let n = self.new_net(Driver::Const(t));
        self.consts[slot] = Some(n);
        n
    }

    pub fn output(&mut self, name: impl Into<String>, net: Net) {
        self.outputs.push((name.into(), net));
    }

    pub fn gate(&mut self, prim: Prim, inputs: &[Net]) -> Net {
        let arity = match prim {
            Prim::Min(n) | Prim::Max(n) => n as usize,
            Prim::Enc => 2,
            _ => 1,
        };
        assert_eq!(
            inputs.len(),
            arity,
            "{} expects {arity} inputs",
            prim.name()
        );
        assert!((1..=16).contains(&arity), "gates take 1 to 16 inputs");
        assert!(
            inputs.iter().all(|n| n.idx() < self.drivers.len()),
            "unknown net"
        );
        let out = self.new_net(Driver::Gate);
        self.gates.push(Gate {
            prim,
            inputs: inputs.to_vec(),
            out,
            block: self.block,
        });
        out
    }

    pub fn sti(&mut self, x: Net) -> Net {
        self.gate(Prim::Sti, &[x])
    }
    pub fn nti(&mut self, x: Net) -> Net {
        self.gate(Prim::Nti, &[x])
    }
    pub fn pti(&mut self, x: Net) -> Net {
        self.gate(Prim::Pti, &[x])
    }
    pub fn bnot(&mut self, x: Net) -> Net {
        self.gate(Prim::Bnot, &[x])
    }
    pub fn enc(&mut self, p: Net, n: Net) -> Net {
        self.gate(Prim::Enc, &[p, n])
    }
    pub fn min(&mut self, xs: &[Net]) -> Net {
        self.gate(Prim::Min(xs.len() as u8), xs)
    }
    pub fn max(&mut self, xs: &[Net]) -> Net {
        self.gate(Prim::Max(xs.len() as u8), xs)
    }

    /// `f` の中で追加したゲートに部品名 `label` を付ける(内訳の集計用)。入れ子では内側が優先。
    pub fn block<R>(&mut self, label: &'static str, f: impl FnOnce(&mut Circuit) -> R) -> R {
        let saved = std::mem::replace(&mut self.block, label);
        let r = f(self);
        self.block = saved;
        r
    }

    pub fn num_inputs(&self) -> usize {
        self.inputs.len()
    }
    pub fn num_outputs(&self) -> usize {
        self.outputs.len()
    }
    pub fn num_gates(&self) -> usize {
        self.gates.len()
    }
    pub fn input_names(&self) -> impl Iterator<Item = &str> {
        self.inputs.iter().map(|(n, _)| n.as_str())
    }
    pub fn output_names(&self) -> impl Iterator<Item = &str> {
        self.outputs.iter().map(|(n, _)| n.as_str())
    }

    // --- 評価 ---

    /// 全配線の値を `vals` に求める。`inputs` は `input()` を呼んだ順。
    pub fn eval_into(&self, inputs: &[Trit], vals: &mut Vec<Trit>) {
        assert_eq!(inputs.len(), self.inputs.len(), "input count mismatch");
        vals.clear();
        vals.resize(self.drivers.len(), Trit::Z);
        for ((_, n), &v) in self.inputs.iter().zip(inputs) {
            vals[n.idx()] = v;
        }
        for (i, d) in self.drivers.iter().enumerate() {
            if let Driver::Const(t) = d {
                vals[i] = *t;
            }
        }
        let mut buf: [Trit; 16] = [Trit::Z; 16];
        for g in &self.gates {
            let k = g.inputs.len();
            for (slot, n) in buf.iter_mut().zip(&g.inputs) {
                *slot = vals[n.idx()];
            }
            vals[g.out.idx()] = g.prim.eval(&buf[..k]);
        }
    }

    /// 出力の値を `output()` を呼んだ順に返す。
    pub fn outputs_of(&self, vals: &[Trit]) -> Vec<Trit> {
        self.outputs.iter().map(|(_, n)| vals[n.idx()]).collect()
    }

    pub fn run(&self, inputs: &[Trit]) -> Vec<Trit> {
        let mut vals = Vec::new();
        self.eval_into(inputs, &mut vals);
        self.outputs_of(&vals)
    }

    // --- 集計 ---

    pub fn cost(&self) -> Cost {
        self.gates
            .iter()
            .fold(Cost::default(), |c, g| c + g.prim.cost())
    }

    /// 部品名ごとの (ゲート数, コスト)
    pub fn cost_by_block(&self) -> BTreeMap<&'static str, (usize, Cost)> {
        let mut m: BTreeMap<&'static str, (usize, Cost)> = BTreeMap::new();
        for g in &self.gates {
            let e = m.entry(g.block).or_default();
            e.0 += 1;
            e.1 += g.prim.cost();
        }
        m
    }

    /// 基本ゲートの種類ごとの個数(MIN/MAX は入力数別)
    pub fn prim_counts(&self) -> BTreeMap<Prim, usize> {
        let mut m = BTreeMap::new();
        for g in &self.gates {
            *m.entry(g.prim).or_insert(0) += 1;
        }
        m
    }

    /// 各配線について、入力から辿った指標を求める。`step(prim, 入力側の最大値)` で出力側の値を決める。
    fn longest(&self, step: impl Fn(Prim, u32) -> u32) -> Vec<u32> {
        let mut d = vec![0u32; self.drivers.len()];
        for g in &self.gates {
            let m = g.inputs.iter().map(|n| d[n.idx()]).max().unwrap_or(0);
            d[g.out.idx()] = step(g.prim, m);
        }
        d
    }

    /// 受動段数: 能動ゲートを挟まずに連続して通る MIN/MAX の最大段数
    pub fn passive_depth(&self) -> u32 {
        self.longest(|p, m| if p.is_active() { 0 } else { m + 1 })
            .into_iter()
            .max()
            .unwrap_or(0)
    }

    /// 出力までの最長経路のゲート数
    pub fn gate_levels(&self) -> u32 {
        let d = self.longest(|_, m| m + 1);
        self.outputs
            .iter()
            .map(|(_, n)| d[n.idx()])
            .max()
            .unwrap_or(0)
    }

    /// 出力までの最長経路上の能動ゲート数
    pub fn active_levels(&self) -> u32 {
        let d = self.longest(|p, m| m + p.is_active() as u32);
        self.outputs
            .iter()
            .map(|(_, n)| d[n.idx()])
            .max()
            .unwrap_or(0)
    }

    /// 入力ベクタの列を流し、各ゲート出力の配線が 0 をとったかを調べる。
    pub fn observe<'a, I>(&self, vectors: I) -> Observation
    where
        I: IntoIterator<Item = &'a [Trit]>,
    {
        let mut seen = vec![0u8; self.drivers.len()];
        let mut vals = Vec::new();
        let mut count = 0;
        for v in vectors {
            self.eval_into(v, &mut vals);
            for (s, t) in seen.iter_mut().zip(&vals) {
                *s |= 1 << (*t as i8 + 1);
            }
            count += 1;
        }
        let gate_nets: Vec<u8> = self.gates.iter().map(|g| seen[g.out.idx()]).collect();
        let binary = gate_nets
            .iter()
            .filter(|&&m| m != 0 && m & 0b010 == 0)
            .count();
        let ternary = gate_nets.iter().filter(|&&m| m == 0b111).count();
        Observation {
            vectors: count,
            gate_nets: gate_nets.len(),
            binary_nets: binary,
            ternary_nets: ternary,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
    /// 流した入力ベクタの数
    pub vectors: usize,
    /// ゲート出力の配線数
    pub gate_nets: usize,
    /// 一度も 0 をとらなかった配線数
    pub binary_nets: usize,
    /// -, 0, + のすべてをとった配線数
    pub ternary_nets: usize,
}

impl Observation {
    pub fn binary_ratio(&self) -> f64 {
        self.binary_nets as f64 / self.gate_nets.max(1) as f64
    }
}

// --- 3 進の複合ゲート(docs/gates.md 3 章) -------------------------------

/// x を 3 本の 2 値信号に分けたもの(該当すれば +、それ以外は -)
#[derive(Clone, Copy, Debug)]
pub struct Decoded {
    pub n: Net,
    pub z: Net,
    pub p: Net,
}

pub fn decode(c: &mut Circuit, x: Net) -> Decoded {
    c.block("DEC", |c| {
        let is_n = c.nti(x);
        let not_n = c.nti(is_n);
        let not_p = c.pti(x);
        let is_p = c.pti(not_p);
        let is_z = c.min(&[not_n, not_p]);
        Decoded {
            n: is_n,
            z: is_z,
            p: is_p,
        }
    })
}

/// mod 3 の和(積和形)
pub fn sum(c: &mut Circuit, a: Decoded, b: Decoded) -> Net {
    c.block("SUM", |c| {
        let zero = c.konst(Trit::Z);
        let terms = [
            c.min(&[a.n, b.n]),
            c.min(&[a.z, b.p]),
            c.min(&[a.p, b.z]),
            c.min(&[a.n, b.p, zero]),
            c.min(&[a.z, b.z, zero]),
            c.min(&[a.p, b.n, zero]),
        ];
        c.max(&terms)
    })
}

/// 和の 2 値 3 本(結果が -, 0, + の各行の OR)。`with_zero` が偽なら z は作らない(z = p を返す)
pub fn sum_terms(c: &mut Circuit, a: Decoded, b: Decoded, with_zero: bool) -> Decoded {
    c.block("SUM", |c| {
        let p = [c.min(&[a.n, b.n]), c.min(&[a.z, b.p]), c.min(&[a.p, b.z])];
        let n = [c.min(&[a.n, b.z]), c.min(&[a.z, b.n]), c.min(&[a.p, b.p])];
        let p = c.max(&p);
        let n = c.max(&n);
        let z = if with_zero {
            let t = [c.min(&[a.n, b.p]), c.min(&[a.z, b.z]), c.min(&[a.p, b.n])];
            c.max(&t)
        } else {
            p
        };
        Decoded { n, z, p }
    })
}

/// 2 値 3 本から 3 値をダイオードで作る: MAX(p, MIN(z, 0))
pub fn from_decoded(c: &mut Circuit, d: Decoded) -> Net {
    c.block("SUM", |c| {
        let zero = c.konst(Trit::Z);
        let z0 = c.min(&[d.z, zero]);
        c.max(&[d.p, z0])
    })
}

/// 出力段付きの SUM(docs/gates.md 3.2b)
pub fn sum_enc(c: &mut Circuit, a: Decoded, b: Decoded) -> Net {
    let t = sum_terms(c, a, b, false);
    c.block("SUM", |c| c.enc(t.p, t.n))
}

/// 一致: a = b なら a、そうでなければ 0
pub fn cons(c: &mut Circuit, a: Net, b: Net) -> Net {
    c.block("CONS", |c| {
        let zero = c.konst(Trit::Z);
        let lo = c.min(&[a, b]);
        let hi = c.max(&[a, b]);
        let hi0 = c.min(&[hi, zero]);
        c.max(&[lo, hi0])
    })
}

/// 0 でない方を通す(異符号なら 0)
pub fn any(c: &mut Circuit, a: Net, b: Net) -> Net {
    c.block("ANY", |c| {
        let zero = c.konst(Trit::Z);
        let hi = c.max(&[a, b]);
        let lo = c.min(&[a, b]);
        let lo_is_n = c.nti(lo);
        let lo_not_n = c.nti(lo_is_n);
        let hi_not_p = c.pti(hi);
        let hi_is_p = c.pti(hi_not_p);
        let t1 = c.min(&[hi, lo_not_n]);
        let t2 = c.min(&[hi_is_p, lo_is_n, zero]);
        c.max(&[t1, t2])
    })
}

/// 出力段付きの ANY(docs/gates.md 3.4)
pub fn any_enc(c: &mut Circuit, a: Net, b: Net) -> Net {
    c.block("ANY", |c| {
        let hi = c.max(&[a, b]);
        let lo = c.min(&[a, b]);
        let lo_is_n = c.nti(lo);
        let lo_not_n = c.nti(lo_is_n);
        let hi_not_p = c.pti(hi);
        let hi_is_p = c.pti(hi_not_p);
        let p = c.min(&[hi_is_p, lo_not_n]);
        let n = c.min(&[lo_is_n, hi_not_p]);
        c.enc(p, n)
    })
}

/// 3 値のレベル再生
pub fn buf(c: &mut Circuit, x: Net) -> Net {
    c.block("BUF", |c| {
        let y = c.sti(x);
        c.sti(y)
    })
}

/// 半加算器 → (sum, carry)
pub fn half_adder(c: &mut Circuit, a: Net, b: Net) -> (Net, Net) {
    let da = decode(c, a);
    let db = decode(c, b);
    (sum(c, da, db), cons(c, a, b))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdderOptions {
    /// 段の出力(3 進: s1, s, cout / 2 進: s, cout)にレベル再生を入れる。`enc` のときは無視
    pub restore: bool,
    /// 改善候補 1: s1 の 2 値 3 本を 2 段目にそのまま渡し、DEC(s1) を省く(3 進のみ)
    pub reuse_s1: bool,
    /// 改善候補 3: SUM と ANY を ENC 版にする(3 進のみ)
    pub enc: bool,
}

impl AdderOptions {
    pub const BASE: AdderOptions = AdderOptions {
        restore: false,
        reuse_s1: false,
        enc: false,
    };

    /// 表示用の名前
    pub fn label(self) -> String {
        let mut v = Vec::new();
        if self.reuse_s1 {
            v.push("s1再利用");
        }
        if self.enc {
            v.push("ENC");
        } else if self.restore {
            v.push("再生あり");
        }
        if v.is_empty() {
            "素直".to_string()
        } else {
            v.join("+")
        }
    }
}

/// 3 進 全加算器 → (sum, cout)。構成は docs/gates.md 4.1
pub fn full_adder(c: &mut Circuit, a: Net, b: Net, cin: Net, opt: AdderOptions) -> (Net, Net) {
    let restore = opt.restore && !opt.enc;
    let da = decode(c, a);
    let db = decode(c, b);

    // 1 段目: s1(3 値)と、必要なら s1 の 2 値 3 本
    let (mut s1, ds1) = if opt.reuse_s1 {
        let t = sum_terms(c, da, db, true);
        let s1 = if opt.enc {
            c.block("SUM", |c| c.enc(t.p, t.n))
        } else {
            from_decoded(c, t)
        };
        (s1, Some(t))
    } else if opt.enc {
        (sum_enc(c, da, db), None)
    } else {
        (sum(c, da, db), None)
    };
    if restore {
        s1 = buf(c, s1);
    }
    let c1 = cons(c, a, b);

    // 2 段目
    let ds1 = match ds1 {
        Some(d) => d,
        None => decode(c, s1),
    };
    let dcin = decode(c, cin);
    let mut s = if opt.enc {
        sum_enc(c, ds1, dcin)
    } else {
        sum(c, ds1, dcin)
    };
    let c2 = cons(c, s1, cin);
    let mut cout = if opt.enc {
        any_enc(c, c1, c2)
    } else {
        any(c, c1, c2)
    };
    if restore {
        s = buf(c, s);
        cout = buf(c, cout);
    }
    (s, cout)
}

/// N 桁の加算器を組む。入力 a0.., b0.., cin / 出力 s0.., cout
fn ripple(n: usize, cell: impl Fn(&mut Circuit, Net, Net, Net) -> (Net, Net)) -> Circuit {
    let mut c = Circuit::new();
    let a: Vec<Net> = (0..n).map(|i| c.input(format!("a{i}"))).collect();
    let b: Vec<Net> = (0..n).map(|i| c.input(format!("b{i}"))).collect();
    let mut carry = c.input("cin");
    let mut s = Vec::with_capacity(n);
    for i in 0..n {
        let (si, ci) = cell(&mut c, a[i], b[i], carry);
        s.push(si);
        carry = ci;
    }
    for (i, si) in s.into_iter().enumerate() {
        c.output(format!("s{i}"), si);
    }
    c.output("cout", carry);
    c
}

/// N トリット リップルキャリー加算器
pub fn ripple_adder(n: usize, opt: AdderOptions) -> Circuit {
    ripple(n, |c, a, b, cin| full_adder(c, a, b, cin, opt))
}

// --- 2 進の比較用回路(docs/gates.md 4.2) --------------------------------

/// 2 値のレベル再生
pub fn binary_buf(c: &mut Circuit, x: Net) -> Net {
    c.block("BUF", |c| {
        let y = c.bnot(x);
        c.bnot(y)
    })
}

/// 2 進 全加算器(- を 0、+ を 1 とする)→ (sum, cout)
pub fn binary_full_adder(
    c: &mut Circuit,
    a: Net,
    b: Net,
    cin: Net,
    opt: AdderOptions,
) -> (Net, Net) {
    let (na, nb, nc) = c.block("NOT", |c| (c.bnot(a), c.bnot(b), c.bnot(cin)));
    let mut s = c.block("SUM", |c| {
        let t = [
            c.min(&[a, nb, nc]),
            c.min(&[na, b, nc]),
            c.min(&[na, nb, cin]),
            c.min(&[a, b, cin]),
        ];
        c.max(&t)
    });
    let mut cout = c.block("CARRY", |c| {
        let t = [c.min(&[a, b]), c.min(&[a, cin]), c.min(&[b, cin])];
        c.max(&t)
    });
    if opt.restore {
        s = binary_buf(c, s);
        cout = binary_buf(c, cout);
    }
    (s, cout)
}

/// N ビット リップルキャリー加算器(2 進)
pub fn binary_ripple_adder(n: usize, opt: AdderOptions) -> Circuit {
    ripple(n, |c, a, b, cin| binary_full_adder(c, a, b, cin, opt))
}

// --- ワード演算との接続 ----------------------------------------------------

/// `ripple_adder(N, _)` の回路で `a + b + cin` を計算する。
pub fn eval_adder<const N: usize>(
    circuit: &Circuit,
    a: Word<N>,
    b: Word<N>,
    cin: Trit,
    vals: &mut Vec<Trit>,
) -> (Word<N>, Trit) {
    let mut ins = [Trit::Z; 64];
    ins[..N].copy_from_slice(a.trits());
    ins[N..2 * N].copy_from_slice(b.trits());
    ins[2 * N] = cin;
    circuit.eval_into(&ins[..2 * N + 1], vals);
    let mut s = [Trit::Z; N];
    for (slot, (_, net)) in s.iter_mut().zip(&circuit.outputs) {
        *slot = vals[net.idx()];
    }
    (Word(s), vals[circuit.outputs[N].1.idx()])
}

/// ゲートレベルの加算器を使う ALU。CPU の ADD をこれに差し替えられる。
pub struct GateAlu {
    circuit: Circuit,
    vals: Vec<Trit>,
}

impl GateAlu {
    pub fn new(opt: AdderOptions) -> GateAlu {
        GateAlu {
            circuit: ripple_adder(WORD_WIDTH, opt),
            vals: Vec::new(),
        }
    }

    pub fn circuit(&self) -> &Circuit {
        &self.circuit
    }
}

impl Alu for GateAlu {
    fn add(&mut self, a: Word9, b: Word9) -> (Word9, Trit) {
        eval_adder(&self.circuit, a, b, Trit::Z, &mut self.vals)
    }
}
