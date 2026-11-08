"""アセンブラ(M2)。文法は docs/isa.md 5 章。2 パスでラベルを解決する。"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from .cpu import ADDR_MAX, NO_OPERAND, WORD_WIDTH, Op, encode
from .trit import Word, max_range

_LABEL = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


class AsmError(Exception):
    def __init__(self, msg: str, line: int | None = None):
        super().__init__(f"line {line}: {msg}" if line else msg)
        self.line = line


@dataclass
class Program:
    image: dict[int, Word] = field(default_factory=dict)
    labels: dict[str, int] = field(default_factory=dict)
    source_lines: dict[int, int] = field(default_factory=dict)  # アドレス → 行番号


@dataclass
class _Item:
    lineno: int
    addr: int
    kind: str          # "insn" | "word"
    op: Op | None
    operand: str | None


def _parse_number(tok: str) -> int | None:
    tok = tok.strip()
    if tok.lower().startswith("0t"):
        return int(Word.parse(tok[2:]))
    if re.fullmatch(r"[+\-]?\d+", tok):
        return int(tok)
    return None


def _eval(expr: str, labels: dict[str, int], lineno: int) -> int:
    expr = expr.strip()
    n = _parse_number(expr)
    if n is not None:
        return n
    m = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*)\s*(?:([+\-])\s*(\d+|0t[+0\-T1]+))?", expr)
    if not m:
        raise AsmError(f"bad operand {expr!r}", lineno)
    name, sign, off = m.groups()
    if name not in labels:
        raise AsmError(f"undefined label {name!r}", lineno)
    v = labels[name]
    if sign:
        k = _parse_number(off)
        v = v + k if sign == "+" else v - k
    return v


def assemble(source: str) -> Program:
    prog = Program()
    items: list[_Item] = []
    addr = 0

    # パス 1: ラベルとアドレスの割り当て
    for lineno, raw in enumerate(source.splitlines(), 1):
        line = raw.split(";", 1)[0].strip()
        while ":" in line:
            label, line = line.split(":", 1)
            label, line = label.strip(), line.strip()
            if not _LABEL.match(label):
                raise AsmError(f"bad label {label!r}", lineno)
            if label in prog.labels:
                raise AsmError(f"duplicate label {label!r}", lineno)
            prog.labels[label] = addr
        if not line:
            continue
        parts = line.split(None, 1)
        mnem = parts[0].upper()
        arg = parts[1].strip() if len(parts) > 1 else None

        if mnem == ".ORG":
            if arg is None:
                raise AsmError(".org needs an address", lineno)
            n = _parse_number(arg)
            if n is None or not -ADDR_MAX <= n <= ADDR_MAX:
                raise AsmError(f"bad .org address {arg!r}", lineno)
            addr = n
            continue
        if mnem == ".WORD":
            if arg is None:
                raise AsmError(".word needs a value", lineno)
            items.append(_Item(lineno, addr, "word", None, arg))
        else:
            try:
                op = Op[mnem]
            except KeyError:
                raise AsmError(f"unknown mnemonic {parts[0]!r}", lineno) from None
            if arg is None and op not in NO_OPERAND:
                raise AsmError(f"{op.name} needs an operand", lineno)
            items.append(_Item(lineno, addr, "insn", op, arg))
        if addr > ADDR_MAX:
            raise AsmError("program exceeds memory", lineno)
        addr += 1

    # パス 2: 符号化
    for it in items:
        if it.addr in prog.image:
            raise AsmError(f"address {it.addr} assigned twice", it.lineno)
        if it.kind == "word":
            v = _eval(it.operand, prog.labels, it.lineno)
            if abs(v) > max_range(WORD_WIDTH):
                raise AsmError(f"value {v} does not fit in {WORD_WIDTH} trits", it.lineno)
            w = Word.from_int(v, WORD_WIDTH)
        else:
            v = 0 if it.operand is None else _eval(it.operand, prog.labels, it.lineno)
            if abs(v) > ADDR_MAX:
                raise AsmError(f"operand {v} out of range ±{ADDR_MAX}", it.lineno)
            w = encode(it.op, v)
        prog.image[it.addr] = w
        prog.source_lines[it.addr] = it.lineno
    return prog
