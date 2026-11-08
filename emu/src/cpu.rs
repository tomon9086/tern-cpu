//! ISA レベル エミュレータ(M2)。仕様は docs/isa.md。

use std::fmt;

use crate::trit::{max_range, trits_value, Trit, Word9};

pub const WORD_WIDTH: usize = 9;
pub const OP_WIDTH: usize = 3;
pub const ADDR_WIDTH: usize = 6;
/// アドレスの最大値 364(最小値は -364)
pub const ADDR_MAX: i16 = max_range(ADDR_WIDTH) as i16;
/// メモリ語数 729
pub const MEM_SIZE: usize = 3usize.pow(ADDR_WIDTH as u32);
pub const OP_MAX: i8 = max_range(OP_WIDTH) as i8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i8)]
pub enum Op {
    Halt = 0,
    Lda = 1,
    Sta = 2,
    Ldi = 3,
    Add = 4,
    Neg = 5,
    Jmp = 6,
    Br3 = 7,
    Out = 8,
}

impl Op {
    pub const ALL: [Op; 9] = [
        Op::Halt,
        Op::Lda,
        Op::Sta,
        Op::Ldi,
        Op::Add,
        Op::Neg,
        Op::Jmp,
        Op::Br3,
        Op::Out,
    ];

    pub fn from_i8(v: i8) -> Option<Op> {
        Op::ALL.into_iter().find(|&o| o as i8 == v)
    }

    pub fn name(self) -> &'static str {
        match self {
            Op::Halt => "HALT",
            Op::Lda => "LDA",
            Op::Sta => "STA",
            Op::Ldi => "LDI",
            Op::Add => "ADD",
            Op::Neg => "NEG",
            Op::Jmp => "JMP",
            Op::Br3 => "BR3",
            Op::Out => "OUT",
        }
    }

    /// ニーモニック(大文字小文字を区別しない)から。
    pub fn from_name(s: &str) -> Option<Op> {
        Op::ALL
            .into_iter()
            .find(|o| o.name().eq_ignore_ascii_case(s))
    }

    /// operand を省略できる命令
    pub fn takes_operand(self) -> bool {
        !matches!(self, Op::Halt | Op::Neg | Op::Out)
    }
}

/// アドレスを -364〜+364 に正規化(ラップ)。
pub fn wrap_addr(addr: i32) -> i16 {
    let m = MEM_SIZE as i32;
    let h = ADDR_MAX as i32;
    ((addr + h).rem_euclid(m) - h) as i16
}

/// 命令語を作る。範囲外なら `None`。
pub fn encode(op: i8, operand: i16) -> Option<Word9> {
    if !(-OP_MAX..=OP_MAX).contains(&op) || !(-ADDR_MAX..=ADDR_MAX).contains(&operand) {
        return None;
    }
    Word9::from_i64(op as i64 * MEM_SIZE as i64 + operand as i64).ok()
}

/// 命令語を作る(`Op` 版)。operand が範囲外なら panic。
pub fn insn(op: Op, operand: i16) -> Word9 {
    encode(op as i8, operand).expect("operand out of range")
}

/// 命令語 → (opcode, operand)
pub fn decode(w: Word9) -> (i8, i16) {
    let t = w.trits();
    (
        trits_value(&t[ADDR_WIDTH..]) as i8,
        trits_value(&t[..ADDR_WIDTH]) as i16,
    )
}

pub fn disassemble(w: Word9) -> String {
    let (op, operand) = decode(w);
    match Op::from_i8(op) {
        Some(o) if !o.takes_operand() && operand == 0 => o.name().to_string(),
        Some(o) => format!("{} {}", o.name(), operand),
        None => format!("??? (op={op}, operand={operand})"),
    }
}

/// ADD の差し替え口。M6 で実機 ALU の実装を渡す。
pub trait Alu {
    /// `a + b == 結果 + キャリー * 3^9`
    fn add(&mut self, a: Word9, b: Word9) -> (Word9, Trit);
}

/// ソフトウェア ALU(既定)
#[derive(Default)]
pub struct SoftAlu;

impl Alu for SoftAlu {
    fn add(&mut self, a: Word9, b: Word9) -> (Word9, Trit) {
        a.add_carry(b, Trit::Z)
    }
}

impl<F: FnMut(Word9, Word9) -> (Word9, Trit)> Alu for F {
    fn add(&mut self, a: Word9, b: Word9) -> (Word9, Trit) {
        self(a, b)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CpuError {
    IllegalOpcode { op: i8, pc: i16 },
    StepLimit { limit: u64, pc: i16 },
}

impl fmt::Display for CpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CpuError::IllegalOpcode { op, pc } => write!(f, "illegal opcode {op} at {pc}"),
            CpuError::StepLimit { limit, pc } => {
                write!(f, "step limit {limit} exceeded (pc={pc})")
            }
        }
    }
}

impl std::error::Error for CpuError {}

pub struct Cpu<A: Alu = SoftAlu> {
    mem: Vec<Word9>,
    pub acc: Word9,
    pub carry: Trit,
    pub pc: i16,
    pub halted: bool,
    pub steps: u64,
    pub output: Vec<Word9>,
    alu: A,
}

impl Cpu<SoftAlu> {
    pub fn new() -> Self {
        Cpu::with_alu(SoftAlu)
    }
}

impl Default for Cpu<SoftAlu> {
    fn default() -> Self {
        Cpu::new()
    }
}

impl<A: Alu> Cpu<A> {
    pub fn with_alu(alu: A) -> Self {
        Cpu {
            mem: vec![Word9::ZERO; MEM_SIZE],
            acc: Word9::ZERO,
            carry: Trit::Z,
            pc: 0,
            halted: false,
            steps: 0,
            output: Vec::new(),
            alu,
        }
    }

    pub fn reset(&mut self) {
        self.mem.fill(Word9::ZERO);
        self.acc = Word9::ZERO;
        self.carry = Trit::Z;
        self.pc = 0;
        self.halted = false;
        self.steps = 0;
        self.output.clear();
    }

    fn index(addr: i16) -> usize {
        (wrap_addr(addr as i32) + ADDR_MAX) as usize
    }

    pub fn read(&self, addr: i16) -> Word9 {
        self.mem[Self::index(addr)]
    }

    pub fn write(&mut self, addr: i16, value: Word9) {
        self.mem[Self::index(addr)] = value;
    }

    /// (アドレス, 語) の組を書き込む。
    pub fn load<I: IntoIterator<Item = (i16, Word9)>>(&mut self, image: I) {
        for (addr, w) in image {
            self.write(addr, w);
        }
    }

    pub fn step(&mut self) -> Result<(), CpuError> {
        if self.halted {
            return Ok(());
        }
        let pc = self.pc;
        let (op, operand) = decode(self.read(pc));
        self.pc = wrap_addr(pc as i32 + 1);
        self.steps += 1;

        let Some(op) = Op::from_i8(op) else {
            self.halted = true;
            return Err(CpuError::IllegalOpcode { op, pc });
        };
        match op {
            Op::Halt => self.halted = true,
            Op::Lda => self.acc = self.read(operand),
            Op::Sta => self.write(operand, self.acc),
            Op::Ldi => self.acc = Word9::from_i64_wrapping(operand as i64),
            Op::Add => {
                let b = self.read(operand);
                (self.acc, self.carry) = self.alu.add(self.acc, b);
            }
            Op::Neg => self.acc = self.acc.negate(),
            Op::Jmp => self.pc = operand,
            Op::Br3 => self.pc = wrap_addr(operand as i32 + self.acc.sign() as i32),
            Op::Out => self.output.push(self.acc),
        }
        Ok(())
    }

    /// HALT まで実行。
    pub fn run(&mut self, max_steps: u64) -> Result<(), CpuError> {
        self.run_traced(max_steps, |_, _, _| {})
    }

    /// HALT まで実行し、各命令の実行後に `trace(cpu, 実行前PC, 命令語)` を呼ぶ。
    pub fn run_traced<F: FnMut(&Self, i16, Word9)>(
        &mut self,
        max_steps: u64,
        mut trace: F,
    ) -> Result<(), CpuError> {
        while !self.halted {
            if self.steps >= max_steps {
                return Err(CpuError::StepLimit {
                    limit: max_steps,
                    pc: self.pc,
                });
            }
            let pc = self.pc;
            let w = self.read(pc);
            self.step()?;
            trace(self, pc, w);
        }
        Ok(())
    }

    pub fn state_line(&self) -> String {
        format!(
            "PC={:+4} ACC={} ({:+6}) C={:+}",
            self.pc,
            self.acc,
            self.acc.to_i64(),
            self.carry as i8
        )
    }

    pub fn format_trace(&self, pc: i16, w: Word9) -> String {
        format!(
            "{:5}  [{:+4}] {}  {:<10} -> {}",
            self.steps,
            pc,
            w,
            disassemble(w),
            self.state_line()
        )
    }
}
