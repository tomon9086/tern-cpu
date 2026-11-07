"""平衡三進のトリット・ワード演算(M1)。

仕様は docs/balanced-ternary.md。ワードは LSB first のタプルで持つ(ADR 0002)。
"""

from __future__ import annotations

from typing import Iterable

NEG, ZERO, POS = -1, 0, 1
TRITS = (NEG, ZERO, POS)

DEFAULT_WIDTH = 9

_TO_CHAR = {NEG: "-", ZERO: "0", POS: "+"}
_FROM_CHAR = {"-": NEG, "T": NEG, "t": NEG, "0": ZERO, "+": POS, "1": POS}


def max_range(width: int) -> int:
    """width トリットで表せる最大値 (3^width - 1) / 2。最小値はその符号反転。"""
    return (3 ** width - 1) // 2


# --- 1 トリット演算 -------------------------------------------------------

def add_trit(a: int, b: int, cin: int = 0) -> tuple[int, int]:
    """1 トリット全加算器。a + b + cin == sum + 3 * carry となる (sum, carry) を返す。"""
    s = a + b + cin
    total = (s + 1) % 3 - 1
    return total, (s - total) // 3


def sti(x: int) -> int:
    """標準インバータ: -x"""
    return -x


def nti(x: int) -> int:
    """負インバータ: x が - なら +、それ以外は -"""
    return POS if x == NEG else NEG


def pti(x: int) -> int:
    """正インバータ: x が + なら -、それ以外は +"""
    return NEG if x == POS else POS


def trit_min(a: int, b: int) -> int:
    return a if a < b else b


def trit_max(a: int, b: int) -> int:
    return a if a > b else b


# --- ワード ----------------------------------------------------------------

class Word:
    """固定長の平衡三進ワード(不変)。trits[i] は 3^i の桁。"""

    __slots__ = ("trits",)

    def __init__(self, trits: Iterable[int]):
        t = tuple(trits)
        if not t:
            raise ValueError("word must have at least one trit")
        for x in t:
            if x not in TRITS:
                raise ValueError(f"invalid trit: {x!r}")
        object.__setattr__(self, "trits", t)

    def __setattr__(self, name, value):
        raise AttributeError("Word is immutable")

    # --- 生成 ---

    @classmethod
    def from_int(cls, value: int, width: int = DEFAULT_WIDTH, *, wrap: bool = False) -> "Word":
        """整数から生成。範囲外は OverflowError(wrap=True なら 3^width を法として丸める)。"""
        if width < 1:
            raise ValueError("width must be >= 1")
        v = value
        trits = []
        for _ in range(width):
            r = v % 3
            if r == 2:
                r = NEG
            trits.append(r)
            v = (v - r) // 3
        if v != 0 and not wrap:
            raise OverflowError(f"{value} does not fit in {width} trits "
                                f"(range ±{max_range(width)})")
        return cls(trits)

    @classmethod
    def zero(cls, width: int = DEFAULT_WIDTH) -> "Word":
        return cls((ZERO,) * width)

    @classmethod
    def parse(cls, text: str, width: int | None = None) -> "Word":
        """MSB first の文字列('+0-' や '10T')から生成。width 指定時は上位を 0 で埋める。"""
        s = text.strip().replace("_", "")
        if not s:
            raise ValueError("empty trit string")
        try:
            trits = [_FROM_CHAR[c] for c in reversed(s)]
        except KeyError as e:
            raise ValueError(f"invalid trit character {e.args[0]!r} in {text!r}") from None
        if width is not None:
            if len(trits) > width:
                raise ValueError(f"{text!r} is longer than {width} trits")
            trits += [ZERO] * (width - len(trits))
        return cls(trits)

    # --- 基本情報 ---

    @property
    def width(self) -> int:
        return len(self.trits)

    def __int__(self) -> int:
        v = 0
        for t in reversed(self.trits):
            v = v * 3 + t
        return v

    def __index__(self) -> int:
        return int(self)

    def __str__(self) -> str:
        return "".join(_TO_CHAR[t] for t in reversed(self.trits))

    def __repr__(self) -> str:
        return f"Word('{self}' = {int(self)})"

    def __eq__(self, other) -> bool:
        return isinstance(other, Word) and self.trits == other.trits

    def __hash__(self) -> int:
        return hash(self.trits)

    def __getitem__(self, i: int) -> int:
        return self.trits[i]

    def sign(self) -> int:
        """0 でない最上位トリット(全 0 なら 0)。BR3 の分岐判定に使う。"""
        for t in reversed(self.trits):
            if t != ZERO:
                return t
        return ZERO

    # --- 算術 ---

    def add(self, other: "Word", cin: int = ZERO) -> tuple["Word", int]:
        """リップルキャリー加算。self + other + cin == result + cout * 3^width。"""
        if self.width != other.width:
            raise ValueError(f"width mismatch: {self.width} vs {other.width}")
        if cin not in TRITS:
            raise ValueError(f"invalid carry: {cin!r}")
        out = []
        c = cin
        for a, b in zip(self.trits, other.trits):
            s, c = add_trit(a, b, c)
            out.append(s)
        return Word(out), c

    def neg(self) -> "Word":
        return Word(sti(t) for t in self.trits)

    def __add__(self, other: "Word") -> "Word":
        return self.add(other)[0]

    def __sub__(self, other: "Word") -> "Word":
        return self.add(other.neg())[0]

    def __neg__(self) -> "Word":
        return self.neg()

    def shl(self, k: int = 1) -> "Word":
        """左シフト(×3^k、上位はあふれて捨てる)。"""
        if k < 0:
            raise ValueError("shift amount must be >= 0")
        k = min(k, self.width)
        return Word((ZERO,) * k + self.trits[: self.width - k])

    def shr(self, k: int = 1) -> "Word":
        """右シフト(3^k で割って最も近い整数に丸める)。"""
        if k < 0:
            raise ValueError("shift amount must be >= 0")
        k = min(k, self.width)
        return Word(self.trits[k:] + (ZERO,) * k)

    # --- 分割・連結(多倍長用) ---

    def split(self, low_width: int) -> tuple["Word", "Word"]:
        """(下位 low_width トリット, 残りの上位) に分ける。"""
        if not 0 < low_width < self.width:
            raise ValueError("low_width out of range")
        return Word(self.trits[:low_width]), Word(self.trits[low_width:])

    @staticmethod
    def concat(low: "Word", high: "Word") -> "Word":
        return Word(low.trits + high.trits)
