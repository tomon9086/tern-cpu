"""コマンドライン実行: python3 -m emu.run programs/sum3x5.t3asm [--trace]"""

from __future__ import annotations

import argparse
import sys

from .asm import AsmError, assemble
from .cpu import CPU, CPUError, disassemble, format_trace


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="平衡三進 CPU エミュレータ (ISA v0)")
    ap.add_argument("source", help=".t3asm ファイル")
    ap.add_argument("--trace", action="store_true", help="1 命令ごとに状態を表示")
    ap.add_argument("--list", action="store_true", help="アセンブル結果を表示して終了")
    ap.add_argument("--max-steps", type=int, default=100_000)
    args = ap.parse_args(argv)

    with open(args.source, encoding="utf-8") as f:
        src = f.read()
    try:
        prog = assemble(src)
    except AsmError as e:
        print(f"{args.source}: {e}", file=sys.stderr)
        return 1

    if args.list:
        names = {v: k for k, v in prog.labels.items()}
        for addr in sorted(prog.image):
            w = prog.image[addr]
            print(f"[{addr:+4d}] {w} {int(w):+6d}  {disassemble(w):<10s} {names.get(addr, '')}")
        return 0

    cpu = CPU()
    cpu.load(prog.image)
    trace = (lambda c, pc, insn: print(format_trace(c, pc, insn))) if args.trace else None
    try:
        cpu.run(max_steps=args.max_steps, trace=trace)
    except CPUError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    for w in cpu.output:
        print(f"OUT {w} {int(w):+d}")
    print(f"HALT after {cpu.steps} steps: {cpu.state_line()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
