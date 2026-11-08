"""M2: ISA レベル エミュレータとアセンブラのテスト(docs/isa.md が仕様)。"""

import pathlib
import unittest

from emu.asm import AsmError, assemble
from emu.cpu import (ADDR_MAX, CPU, CPUError, Op, decode, disassemble, encode,
                     wrap_addr)
from emu.trit import Word, max_range

PROGRAMS = pathlib.Path(__file__).resolve().parent.parent / "programs"


def run_source(src, **kw):
    cpu = CPU(**kw)
    cpu.load(assemble(src).image)
    cpu.run(max_steps=10_000)
    return cpu


class TestEncoding(unittest.TestCase):
    def test_encode_decode_all(self):
        for op in range(-13, 14):
            for operand in range(-ADDR_MAX, ADDR_MAX + 1):
                w = encode(op, operand)
                self.assertEqual(w.width, 9)
                self.assertEqual(decode(w), (op, operand))
                self.assertEqual(int(w), op * 729 + operand)

    def test_halt_is_zero(self):
        self.assertEqual(encode(Op.HALT), Word.zero(9))

    def test_disassemble(self):
        self.assertEqual(disassemble(encode(Op.ADD, -12)), "ADD -12")
        self.assertEqual(disassemble(encode(Op.NEG)), "NEG")
        self.assertTrue(disassemble(encode(-5, 0)).startswith("???"))

    def test_wrap_addr(self):
        self.assertEqual(wrap_addr(ADDR_MAX + 1), -ADDR_MAX)
        self.assertEqual(wrap_addr(-ADDR_MAX - 1), ADDR_MAX)


class TestInstructions(unittest.TestCase):
    def test_lda_sta_ldi(self):
        cpu = run_source("LDI -100\nSTA -5\nLDI 0\nLDA x\nHALT\nx: .word 9841")
        self.assertEqual(int(cpu.read(-5)), -100)
        self.assertEqual(int(cpu.acc), 9841)

    def test_add_matches_word_add_and_sets_carry(self):
        h = max_range(9)
        for x, y in [(1, 2), (h, 1), (-h, -1), (h, h), (-h, -h), (5000, 5000), (-3, 3)]:
            cpu = run_source(f"LDA a\nADD b\nHALT\na: .word {x}\nb: .word {y}")
            r, c = Word.from_int(x, 9).add(Word.from_int(y, 9))
            self.assertEqual(cpu.acc, r)
            self.assertEqual(cpu.carry, c)
            self.assertEqual(int(cpu.acc) + cpu.carry * 3 ** 9, x + y)

    def test_neg(self):
        cpu = run_source("LDI 123\nNEG\nHALT")
        self.assertEqual(int(cpu.acc), -123)

    def test_jmp(self):
        cpu = run_source("JMP skip\nLDI 1\nHALT\nskip: LDI 2\nHALT")
        self.assertEqual(int(cpu.acc), 2)

    def test_br3_three_way(self):
        src = """
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
            v:  .word {v}
        """
        for v, expected in [(-9841, -1), (-1, -1), (0, 0), (1, 1), (9841, 1), (-27, -1)]:
            self.assertEqual(int(run_source(src.format(v=v)).acc), expected, v)

    def test_out(self):
        cpu = run_source("LDI 3\nOUT\nNEG\nOUT\nHALT")
        self.assertEqual([int(w) for w in cpu.output], [3, -3])

    def test_illegal_opcode(self):
        cpu = CPU()
        cpu.write(0, encode(-1, 0))
        with self.assertRaises(CPUError):
            cpu.run()

    def test_step_limit(self):
        with self.assertRaises(CPUError):
            cpu = CPU()
            cpu.load({0: encode(Op.JMP, 0)})
            cpu.run(max_steps=100)

    def test_pc_wraps(self):
        cpu = CPU()
        cpu.load({ADDR_MAX: encode(Op.LDI, 7)})
        cpu.pc = ADDR_MAX
        cpu.run()                      # 次は -364 の HALT(0)
        self.assertEqual(int(cpu.acc), 7)
        self.assertEqual(cpu.pc, -ADDR_MAX + 1)

    def test_alu_is_pluggable(self):
        calls = []

        def fake_alu(a, b):
            calls.append((int(a), int(b)))
            return a.add(b)

        cpu = run_source("LDI 2\nADD x\nHALT\nx: .word 3", alu_add=fake_alu)
        self.assertEqual(calls, [(2, 3)])
        self.assertEqual(int(cpu.acc), 5)


class TestAssembler(unittest.TestCase):
    def test_labels_and_expressions(self):
        p = assemble("a: LDI 0t+0-\n b: JMP a+2\n.org -10\nc: .word c\n.word 0t-")
        self.assertEqual(p.labels, {"a": 0, "b": 1, "c": -10})
        self.assertEqual(p.image[0], encode(Op.LDI, 8))
        self.assertEqual(p.image[1], encode(Op.JMP, 2))
        self.assertEqual(int(p.image[-10]), -10)
        self.assertEqual(int(p.image[-9]), -1)

    def test_case_insensitive_and_comments(self):
        p = assemble("  ldi 1 ; comment\n; only comment\n\nhalt")
        self.assertEqual(p.image, {0: encode(Op.LDI, 1), 1: encode(Op.HALT)})

    def test_errors(self):
        for src in ["FOO 1", "LDI", "JMP nowhere", "x: HALT\nx: HALT", "LDI 365",
                    ".word 9842", ".org 0\nHALT\n.org 0\nHALT", "1bad: HALT"]:
            with self.assertRaises(AsmError, msg=src):
                assemble(src)


class TestSamplePrograms(unittest.TestCase):
    def run_file(self, name):
        cpu = CPU()
        cpu.load(assemble((PROGRAMS / name).read_text(encoding="utf-8")).image)
        cpu.run()
        return cpu

    def test_sum3x5(self):
        cpu = self.run_file("sum3x5.t3asm")
        self.assertEqual(int(cpu.acc), 15)
        self.assertEqual([int(w) for w in cpu.output], [15])

    def test_countdown(self):
        cpu = self.run_file("countdown.t3asm")
        self.assertEqual([int(w) for w in cpu.output], [5, 4, 3, 2, 1])

    def test_multiply_all_signs(self):
        src = (PROGRAMS / "multiply.t3asm").read_text(encoding="utf-8")
        for a in (-7, -1, 0, 1, 13):
            for b in (-12, -1, 0, 1, 9):
                s = src.replace("a:      .word 7", f"a:      .word {a}") \
                       .replace("b:      .word -12", f"b:      .word {b}")
                cpu = CPU()
                cpu.load(assemble(s).image)
                cpu.run()
                self.assertEqual([int(w) for w in cpu.output], [a * b], (a, b))


if __name__ == "__main__":
    unittest.main()
