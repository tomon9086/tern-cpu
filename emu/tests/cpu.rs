//! M2: ISA レベル エミュレータとアセンブラのテスト(docs/isa.md が仕様)。

use tern_emu::asm::assemble;
use tern_emu::cpu::{decode, disassemble, encode, insn, wrap_addr, Cpu, CpuError, Op, ADDR_MAX};
use tern_emu::{Trit, Word9};

fn programs_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../programs")
}

fn run_source(src: &str) -> Cpu {
    let mut cpu = Cpu::new();
    cpu.load(assemble(src).unwrap().image);
    cpu.run(10_000).unwrap();
    cpu
}

fn outputs<A: tern_emu::cpu::Alu>(cpu: &Cpu<A>) -> Vec<i64> {
    cpu.output.iter().map(|w| w.to_i64()).collect()
}

// --- 符号化 ---

#[test]
fn encode_decode_all() {
    for op in -13..=13i8 {
        for operand in -ADDR_MAX..=ADDR_MAX {
            let w = encode(op, operand).unwrap();
            assert_eq!(decode(w), (op, operand));
            assert_eq!(w.to_i64(), op as i64 * 729 + operand as i64);
        }
    }
    assert!(encode(14, 0).is_none());
    assert!(encode(0, 365).is_none());
}

#[test]
fn halt_is_zero() {
    assert_eq!(insn(Op::Halt, 0), Word9::ZERO);
}

#[test]
fn disassemble_format() {
    assert_eq!(disassemble(insn(Op::Add, -12)), "ADD -12");
    assert_eq!(disassemble(insn(Op::Neg, 0)), "NEG");
    assert!(disassemble(encode(-5, 0).unwrap()).starts_with("???"));
}

#[test]
fn wrap_addr_edges() {
    assert_eq!(wrap_addr(ADDR_MAX as i32 + 1), -ADDR_MAX);
    assert_eq!(wrap_addr(-(ADDR_MAX as i32) - 1), ADDR_MAX);
}

// --- 命令 ---

#[test]
fn lda_sta_ldi() {
    let cpu = run_source("LDI -100\nSTA -5\nLDI 0\nLDA x\nHALT\nx: .word 9841");
    assert_eq!(cpu.read(-5).to_i64(), -100);
    assert_eq!(cpu.acc.to_i64(), 9841);
}

#[test]
fn add_matches_word_add_and_sets_carry() {
    let h = Word9::MAX;
    for (x, y) in [
        (1, 2),
        (h, 1),
        (-h, -1),
        (h, h),
        (-h, -h),
        (5000, 5000),
        (-3, 3),
    ] {
        let cpu = run_source(&format!("LDA a\nADD b\nHALT\na: .word {x}\nb: .word {y}"));
        let (r, c) = Word9::from_i64(x)
            .unwrap()
            .add_carry(Word9::from_i64(y).unwrap(), Trit::Z);
        assert_eq!((cpu.acc, cpu.carry), (r, c));
        assert_eq!(cpu.acc.to_i64() + cpu.carry as i64 * Word9::MODULUS, x + y);
    }
}

#[test]
fn neg() {
    assert_eq!(run_source("LDI 123\nNEG\nHALT").acc.to_i64(), -123);
}

#[test]
fn jmp() {
    assert_eq!(
        run_source("JMP skip\nLDI 1\nHALT\nskip: LDI 2\nHALT")
            .acc
            .to_i64(),
        2
    );
}

#[test]
fn br3_three_way() {
    let src = "
            LDA v
            BR3 t
            JMP n
        t:  JMP z
            JMP p
        n:  LDI -1
            HALT
        z:  LDI 0
            HALT
        p:  LDI 1
            HALT
        v:  .word ";
    for (v, expected) in [(-9841, -1), (-1, -1), (0, 0), (1, 1), (9841, 1), (-27, -1)] {
        assert_eq!(
            run_source(&format!("{src}{v}")).acc.to_i64(),
            expected,
            "v={v}"
        );
    }
}

#[test]
fn out() {
    assert_eq!(outputs(&run_source("LDI 3\nOUT\nNEG\nOUT\nHALT")), [3, -3]);
}

#[test]
fn illegal_opcode() {
    let mut cpu = Cpu::new();
    cpu.write(0, encode(-1, 0).unwrap());
    assert_eq!(cpu.run(100), Err(CpuError::IllegalOpcode { op: -1, pc: 0 }));
}

#[test]
fn step_limit() {
    let mut cpu = Cpu::new();
    cpu.write(0, insn(Op::Jmp, 0));
    assert!(matches!(cpu.run(100), Err(CpuError::StepLimit { .. })));
}

#[test]
fn pc_wraps() {
    let mut cpu = Cpu::new();
    cpu.write(ADDR_MAX, insn(Op::Ldi, 7));
    cpu.pc = ADDR_MAX;
    cpu.run(10).unwrap(); // 次は -364 の HALT(0)
    assert_eq!(cpu.acc.to_i64(), 7);
    assert_eq!(cpu.pc, -ADDR_MAX + 1);
}

#[test]
fn alu_is_pluggable() {
    let mut calls = Vec::new();
    let mut cpu = Cpu::with_alu(|a: Word9, b: Word9| {
        calls.push((a.to_i64(), b.to_i64()));
        a.add_carry(b, Trit::Z)
    });
    cpu.load(assemble("LDI 2\nADD x\nHALT\nx: .word 3").unwrap().image);
    cpu.run(100).unwrap();
    assert_eq!(cpu.acc.to_i64(), 5);
    drop(cpu);
    assert_eq!(calls, [(2, 3)]);
}

// --- アセンブラ ---

#[test]
fn labels_and_expressions() {
    let p = assemble("a: LDI 0t+0-\n b: JMP a+2\n.org -10\nc: .word c\n.word 0t-").unwrap();
    assert_eq!(p.labels.len(), 3);
    assert_eq!((p.labels["a"], p.labels["b"], p.labels["c"]), (0, 1, -10));
    assert_eq!(p.image[&0], insn(Op::Ldi, 8));
    assert_eq!(p.image[&1], insn(Op::Jmp, 2));
    assert_eq!(p.image[&-10].to_i64(), -10);
    assert_eq!(p.image[&-9].to_i64(), -1);
}

#[test]
fn case_insensitive_and_comments() {
    let p = assemble("  ldi 1 ; comment\n; only comment\n\nhalt").unwrap();
    let image: Vec<_> = p.image.into_iter().collect();
    assert_eq!(image, [(0, insn(Op::Ldi, 1)), (1, insn(Op::Halt, 0))]);
}

#[test]
fn assembler_errors() {
    for src in [
        "FOO 1",
        "LDI",
        "JMP nowhere",
        "x: HALT\nx: HALT",
        "LDI 365",
        ".word 9842",
        ".org 0\nHALT\n.org 0\nHALT",
        "1bad: HALT",
        "JMP x+-1\nx: HALT",
    ] {
        assert!(assemble(src).is_err(), "{src:?} should fail");
    }
}

// --- サンプルプログラム ---

fn run_file(name: &str) -> Cpu {
    run_source(&std::fs::read_to_string(programs_dir().join(name)).unwrap())
}

#[test]
fn sample_sum3x5() {
    let cpu = run_file("sum3x5.t3asm");
    assert_eq!(cpu.acc.to_i64(), 15);
    assert_eq!(outputs(&cpu), [15]);
}

#[test]
fn sample_countdown() {
    assert_eq!(outputs(&run_file("countdown.t3asm")), [5, 4, 3, 2, 1]);
}

#[test]
fn sample_multiply_all_signs() {
    let src = std::fs::read_to_string(programs_dir().join("multiply.t3asm")).unwrap();
    assert!(src.contains("a:      .word 7") && src.contains("b:      .word -12"));
    for a in [-7, -1, 0, 1, 13] {
        for b in [-12, -1, 0, 1, 9] {
            let s = src
                .replace("a:      .word 7", &format!("a:      .word {a}"))
                .replace("b:      .word -12", &format!("b:      .word {b}"));
            assert_eq!(outputs(&run_source(&s)), [a * b], "a={a} b={b}");
        }
    }
}
