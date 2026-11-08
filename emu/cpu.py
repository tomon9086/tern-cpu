"""ISA レベル エミュレータ(M2)。仕様は docs/isa.md。"""

from __future__ import annotations

from enum import IntEnum
from typing import Callable, Iterable, Mapping

from .trit import Word, max_range

WORD_WIDTH = 9
OP_WIDTH = 3
ADDR_WIDTH = 6
ADDR_MAX = max_range(ADDR_WIDTH)          # 364
MEM_SIZE = 3 ** ADDR_WIDTH                # 729


class Op(IntEnum):
    HALT = 0
    LDA = 1
    STA = 2
    LDI = 3
    ADD = 4
    NEG = 5
    JMP = 6
    BR3 = 7
    OUT = 8


# operand を持たない命令
NO_OPERAND = {Op.HALT, Op.NEG, Op.OUT}

# ADD の実装を差し替えるための型: (a, b) -> (結果, キャリー)。M6 で実機 ALU に置き換える
AluAdd = Callable[[Word, Word], "tuple[Word, int]"]


class CPUError(Exception):
    pass


def wrap_addr(addr: int) -> int:
    """アドレスを -364〜+364 に正規化(ラップ)。"""
    return (addr + ADDR_MAX) % MEM_SIZE - ADDR_MAX


def encode(op: int, operand: int = 0) -> Word:
    if not -ADDR_MAX <= operand <= ADDR_MAX:
        raise ValueError(f"operand {operand} out of range ±{ADDR_MAX}")
    if not -max_range(OP_WIDTH) <= op <= max_range(OP_WIDTH):
        raise ValueError(f"opcode {op} out of range")
    return Word.from_int(int(op) * MEM_SIZE + operand, WORD_WIDTH)


def decode(word: Word) -> tuple[int, int]:
    """命令語 → (opcode, operand)。"""
    low, high = word.split(ADDR_WIDTH)
    return int(high), int(low)


def disassemble(word: Word) -> str:
    op, operand = decode(word)
    try:
        o = Op(op)
    except ValueError:
        return f"??? (op={op}, operand={operand})"
    if o in NO_OPERAND and operand == 0:
        return o.name
    return f"{o.name} {operand}"


def software_add(a: Word, b: Word) -> tuple[Word, int]:
    return a.add(b)


class CPU:
    def __init__(self, alu_add: AluAdd = software_add):
        self.alu_add = alu_add
        self.reset()

    def reset(self) -> None:
        self.mem: list[Word] = [Word.zero(WORD_WIDTH)] * MEM_SIZE
        self.acc = Word.zero(WORD_WIDTH)
        self.carry = 0
        self.pc = 0
        self.halted = False
        self.steps = 0
        self.output: list[Word] = []

    # --- メモリ ---

    def read(self, addr: int) -> Word:
        return self.mem[wrap_addr(addr) + ADDR_MAX]

    def write(self, addr: int, value: Word) -> None:
        if value.width != WORD_WIDTH:
            raise ValueError("word width mismatch")
        self.mem[wrap_addr(addr) + ADDR_MAX] = value

    def load(self, image: Mapping[int, Word] | Iterable[Word], origin: int = 0) -> None:
        """{アドレス: 語} または語の列(origin から連続配置)を書き込む。"""
        if isinstance(image, Mapping):
            for addr, w in image.items():
                self.write(addr, w)
        else:
            for i, w in enumerate(image):
                self.write(origin + i, w)

    # --- 実行 ---

    def step(self) -> None:
        if self.halted:
            return
        pc = self.pc
        op, operand = decode(self.read(pc))
        self.pc = wrap_addr(pc + 1)
        self.steps += 1

        if op == Op.HALT:
            self.halted = True
        elif op == Op.LDA:
            self.acc = self.read(operand)
        elif op == Op.STA:
            self.write(operand, self.acc)
        elif op == Op.LDI:
            self.acc = Word.from_int(operand, WORD_WIDTH)
        elif op == Op.ADD:
            self.acc, self.carry = self.alu_add(self.acc, self.read(operand))
        elif op == Op.NEG:
            self.acc = self.acc.neg()
        elif op == Op.JMP:
            self.pc = operand
        elif op == Op.BR3:
            self.pc = wrap_addr(operand + self.acc.sign())
        elif op == Op.OUT:
            self.output.append(self.acc)
        else:
            self.halted = True
            raise CPUError(f"illegal opcode {op} at {pc}")

    def run(self, max_steps: int = 100_000,
            trace: Callable[["CPU", int, Word], None] | None = None) -> None:
        """HALT まで実行。trace(cpu, 実行前PC, 命令語) は各命令の実行後に呼ばれる。"""
        while not self.halted:
            if self.steps >= max_steps:
                raise CPUError(f"step limit {max_steps} exceeded (pc={self.pc})")
            pc = self.pc
            insn = self.read(pc)
            self.step()
            if trace:
                trace(self, pc, insn)

    def state_line(self) -> str:
        return (f"PC={self.pc:+4d} ACC={self.acc} ({int(self.acc):+6d}) "
                f"C={self.carry:+d}")


def format_trace(cpu: CPU, pc: int, insn: Word) -> str:
    return f"{cpu.steps:5d}  [{pc:+4d}] {insn}  {disassemble(insn):<10s} -> {cpu.state_line()}"
