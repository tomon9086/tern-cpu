"""M1: 平衡三進 数値ライブラリのテスト(docs/balanced-ternary.md が仕様)。"""

import itertools
import unittest

from emu.trit import (
    NEG, ZERO, POS, TRITS,
    Word, add_trit, max_range, trit_max, trit_min, sti, nti, pti,
)


def balanced_mod(v, width):
    """v を 3^width を法として平衡範囲に正規化した値(期待値計算用)。"""
    m = 3 ** width
    h = max_range(width)
    return (v + h) % m - h


class TestRange(unittest.TestCase):
    def test_max_range(self):
        self.assertEqual(max_range(1), 1)
        self.assertEqual(max_range(3), 13)
        self.assertEqual(max_range(6), 364)
        self.assertEqual(max_range(9), 9841)


class TestConversion(unittest.TestCase):
    def test_round_trip_exhaustive_small_widths(self):
        for width in range(1, 8):
            h = max_range(width)
            seen = set()
            for v in range(-h, h + 1):
                w = Word.from_int(v, width)
                self.assertEqual(w.width, width)
                self.assertEqual(int(w), v)
                self.assertTrue(all(t in TRITS for t in w.trits))
                seen.add(w.trits)
            # 表現が一意: 3^width 通りすべて異なる
            self.assertEqual(len(seen), 3 ** width)

    def test_round_trip_9_trits_exhaustive(self):
        h = max_range(9)
        for v in range(-h, h + 1):
            self.assertEqual(int(Word.from_int(v, 9)), v)

    def test_all_trit_patterns_map_back(self):
        for trits in itertools.product(TRITS, repeat=4):
            w = Word(trits)
            self.assertEqual(Word.from_int(int(w), 4), w)

    def test_out_of_range_raises(self):
        with self.assertRaises(OverflowError):
            Word.from_int(14, 3)
        with self.assertRaises(OverflowError):
            Word.from_int(-14, 3)

    def test_wrap(self):
        self.assertEqual(int(Word.from_int(14, 3, wrap=True)), -13)
        self.assertEqual(int(Word.from_int(-14, 3, wrap=True)), 13)
        for v in range(-100, 101):
            self.assertEqual(int(Word.from_int(v, 3, wrap=True)), balanced_mod(v, 3))

    def test_known_values(self):
        self.assertEqual(Word.from_int(8, 3).trits, (-1, 0, 1))  # LSB first
        self.assertEqual(str(Word.from_int(8, 3)), "+0-")         # MSB first
        self.assertEqual(str(Word.from_int(0, 3)), "000")
        self.assertEqual(str(Word.from_int(13, 3)), "+++")
        self.assertEqual(str(Word.from_int(-13, 3)), "---")
        self.assertEqual(str(Word.from_int(15, 4)), "+--0")

    def test_parse(self):
        self.assertEqual(int(Word.parse("+0-")), 8)
        self.assertEqual(int(Word.parse("10T")), 8)
        self.assertEqual(Word.parse("+0-").width, 3)
        self.assertEqual(Word.parse("+", width=3), Word.from_int(1, 3))
        with self.assertRaises(ValueError):
            Word.parse("+x-")
        with self.assertRaises(ValueError):
            Word.parse("++++", width=3)

    def test_parse_str_round_trip(self):
        for v in range(-max_range(5), max_range(5) + 1):
            w = Word.from_int(v, 5)
            self.assertEqual(Word.parse(str(w)), w)

    def test_invalid_trit(self):
        with self.assertRaises(ValueError):
            Word((0, 2, 0))
        with self.assertRaises(ValueError):
            Word(())


class TestTritAdder(unittest.TestCase):
    def test_full_adder_all_27(self):
        for a, b, c in itertools.product(TRITS, repeat=3):
            s, carry = add_trit(a, b, c)
            self.assertIn(s, TRITS)
            self.assertIn(carry, TRITS)
            self.assertEqual(a + b + c, s + 3 * carry, (a, b, c))

    def test_half_adder_table(self):
        # docs/balanced-ternary.md 5.2 の表そのもの
        table = {
            (NEG, NEG): (POS, NEG), (NEG, ZERO): (NEG, ZERO), (NEG, POS): (ZERO, ZERO),
            (ZERO, NEG): (NEG, ZERO), (ZERO, ZERO): (ZERO, ZERO), (ZERO, POS): (POS, ZERO),
            (POS, NEG): (ZERO, ZERO), (POS, ZERO): (POS, ZERO), (POS, POS): (NEG, POS),
        }
        for (a, b), expected in table.items():
            self.assertEqual(add_trit(a, b), expected)


class TestWordAdd(unittest.TestCase):
    def check_add_exhaustive(self, width):
        h = max_range(width)
        words = [Word.from_int(v, width) for v in range(-h, h + 1)]
        for a in words:
            for b in words:
                for cin in TRITS:
                    r, cout = a.add(b, cin)
                    total = int(a) + int(b) + cin
                    self.assertEqual(int(r) + cout * 3 ** width, total)
                    self.assertEqual(int(r), balanced_mod(total, width))
                    self.assertEqual(cout != 0, abs(total) > h)

    def test_add_1_trit_exhaustive(self):
        self.check_add_exhaustive(1)

    def test_add_3_trits_exhaustive(self):
        # 27 x 27 x 3(キャリー入力)= 2187 通り
        self.check_add_exhaustive(3)

    def test_add_4_trits_exhaustive(self):
        self.check_add_exhaustive(4)

    def test_add_operator_wraps(self):
        a = Word.from_int(13, 3)
        self.assertEqual(int(a + Word.from_int(1, 3)), -13)
        self.assertEqual(int(Word.from_int(3, 3) + Word.from_int(5, 3)), 8)

    def test_width_mismatch(self):
        with self.assertRaises(ValueError):
            Word.from_int(1, 3).add(Word.from_int(1, 4))

    def test_multiword_add_via_carry(self):
        # 2 ワード x 3 トリット = 6 トリット加算と一致すること
        h6 = max_range(6)
        for x in range(-h6, h6 + 1, 7):
            for y in range(-h6, h6 + 1, 11):
                xl, xh = Word.from_int(x, 6).split(3)
                yl, yh = Word.from_int(y, 6).split(3)
                lo, c = xl.add(yl)
                hi, cout = xh.add(yh, c)
                self.assertEqual(int(lo) + int(hi) * 27 + cout * 729, x + y)

    def test_sub(self):
        h = max_range(3)
        for x in range(-h, h + 1):
            for y in range(-h, h + 1):
                self.assertEqual(int(Word.from_int(x, 3) - Word.from_int(y, 3)),
                                 balanced_mod(x - y, 3))

    def test_three_added_five_times(self):
        acc = Word.from_int(0, 4)
        three = Word.from_int(3, 4)
        for _ in range(5):
            acc, cout = acc.add(three)
            self.assertEqual(cout, 0)
        self.assertEqual(int(acc), 15)


class TestNegSign(unittest.TestCase):
    def test_neg_exhaustive(self):
        for width in range(1, 7):
            h = max_range(width)
            for v in range(-h, h + 1):
                w = Word.from_int(v, width)
                self.assertEqual(int(-w), -v)
                self.assertEqual((-w).trits, tuple(-t for t in w.trits))

    def test_sign_exhaustive(self):
        for width in range(1, 7):
            h = max_range(width)
            for v in range(-h, h + 1):
                self.assertEqual(Word.from_int(v, width).sign(), (v > 0) - (v < 0))


class TestShift(unittest.TestCase):
    def test_shl(self):
        h = max_range(5)
        for v in range(-h, h + 1):
            w = Word.from_int(v, 5)
            for k in range(0, 7):
                self.assertEqual(int(w.shl(k)), balanced_mod(v * 3 ** k, 5))

    def test_shr_rounds_to_nearest(self):
        h = max_range(5)
        for v in range(-h, h + 1):
            w = Word.from_int(v, 5)
            for k in range(0, 7):
                q = int(w.shr(k))
                # 最も近い整数(誤差は 1/2 未満で、同点は発生しない)
                self.assertLess(abs(v - q * 3 ** k) * 2, 3 ** k)

    def test_shift_negative_raises(self):
        with self.assertRaises(ValueError):
            Word.from_int(1, 3).shl(-1)


class TestTritLogic(unittest.TestCase):
    def test_inverters(self):
        self.assertEqual([sti(x) for x in (NEG, ZERO, POS)], [POS, ZERO, NEG])
        self.assertEqual([nti(x) for x in (NEG, ZERO, POS)], [POS, NEG, NEG])
        self.assertEqual([pti(x) for x in (NEG, ZERO, POS)], [POS, POS, NEG])

    def test_min_max(self):
        for a, b in itertools.product(TRITS, repeat=2):
            self.assertEqual(trit_min(a, b), min(a, b))
            self.assertEqual(trit_max(a, b), max(a, b))
            # De Morgan: STI(MIN(a,b)) == MAX(STI a, STI b)
            self.assertEqual(sti(trit_min(a, b)), trit_max(sti(a), sti(b)))


if __name__ == "__main__":
    unittest.main()
