//! M1: 平衡三進 数値ライブラリのテスト(docs/balanced-ternary.md が仕様)。

use std::collections::HashSet;

use tern_emu::trit::{
    add_trit, max_range, ParseError, Trit, Word, Word3, Word4, Word5, Word6, Word9,
};

const ALL: [Trit; 3] = Trit::ALL;

/// v を 3^n を法として平衡範囲に正規化した値(期待値計算用)
fn bmod(v: i64, n: usize) -> i64 {
    let h = max_range(n);
    (v + h).rem_euclid(3i64.pow(n as u32)) - h
}

fn w<const N: usize>(v: i64) -> Word<N> {
    Word::from_i64(v).unwrap()
}

#[test]
fn max_range_values() {
    assert_eq!(max_range(1), 1);
    assert_eq!(max_range(3), 13);
    assert_eq!(max_range(6), 364);
    assert_eq!(max_range(9), 9841);
    assert_eq!(Word9::MAX, 9841);
}

// --- 変換 ---

fn round_trip_exhaustive<const N: usize>() {
    let mut seen = HashSet::new();
    for v in -Word::<N>::MAX..=Word::<N>::MAX {
        let x = Word::<N>::from_i64(v).unwrap();
        assert_eq!(x.to_i64(), v);
        seen.insert(x);
    }
    // 表現が一意: 3^N 通りすべて異なる
    assert_eq!(seen.len() as i64, Word::<N>::MODULUS);
}

#[test]
fn round_trip_widths_1_to_9() {
    round_trip_exhaustive::<1>();
    round_trip_exhaustive::<2>();
    round_trip_exhaustive::<3>();
    round_trip_exhaustive::<4>();
    round_trip_exhaustive::<5>();
    round_trip_exhaustive::<6>();
    round_trip_exhaustive::<7>();
    round_trip_exhaustive::<9>();
}

#[test]
fn all_trit_patterns_map_back() {
    for a in ALL {
        for b in ALL {
            for c in ALL {
                for d in ALL {
                    let x = Word4::from_slice(&[a, b, c, d]).unwrap();
                    assert_eq!(Word4::from_i64(x.to_i64()).unwrap(), x);
                }
            }
        }
    }
}

#[test]
fn out_of_range() {
    assert!(Word3::from_i64(14).is_err());
    assert!(Word3::from_i64(-14).is_err());
}

#[test]
fn wrapping() {
    assert_eq!(Word3::from_i64_wrapping(14).to_i64(), -13);
    assert_eq!(Word3::from_i64_wrapping(-14).to_i64(), 13);
    for v in -100..=100 {
        assert_eq!(Word3::from_i64_wrapping(v).to_i64(), bmod(v, 3));
    }
}

#[test]
fn known_values() {
    assert_eq!(*w::<3>(8).trits(), [Trit::N, Trit::Z, Trit::P]); // LSB first
    assert_eq!(w::<3>(8).to_string(), "+0-"); // MSB first
    assert_eq!(w::<3>(0).to_string(), "000");
    assert_eq!(w::<3>(13).to_string(), "+++");
    assert_eq!(w::<3>(-13).to_string(), "---");
    assert_eq!(w::<4>(15).to_string(), "+--0");
}

#[test]
fn parse() {
    assert_eq!(Word3::parse("+0-").unwrap().to_i64(), 8);
    assert_eq!(Word3::parse("10T").unwrap().to_i64(), 8);
    assert_eq!(Word3::parse("+").unwrap(), w::<3>(1));
    assert_eq!(Word3::parse("+x-"), Err(ParseError::BadChar('x')));
    assert!(matches!(
        Word3::parse("++++"),
        Err(ParseError::TooLong { .. })
    ));
    assert_eq!(Word3::parse(""), Err(ParseError::Empty));
}

#[test]
fn parse_display_round_trip() {
    for v in -Word5::MAX..=Word5::MAX {
        let x = w::<5>(v);
        assert_eq!(Word5::parse(&x.to_string()).unwrap(), x);
    }
}

// --- 1 トリット加算 ---

#[test]
fn full_adder_all_27() {
    for a in ALL {
        for b in ALL {
            for c in ALL {
                let (s, carry) = add_trit(a, b, c);
                assert_eq!(a as i8 + b as i8 + c as i8, s as i8 + 3 * carry as i8);
            }
        }
    }
}

#[test]
fn half_adder_table() {
    use Trit::{N, P, Z};
    // docs/balanced-ternary.md 5.2 の表そのもの: (a, b) -> (sum, carry)
    let table = [
        ((N, N), (P, N)),
        ((N, Z), (N, Z)),
        ((N, P), (Z, Z)),
        ((Z, N), (N, Z)),
        ((Z, Z), (Z, Z)),
        ((Z, P), (P, Z)),
        ((P, N), (Z, Z)),
        ((P, Z), (P, Z)),
        ((P, P), (N, P)),
    ];
    for ((a, b), expected) in table {
        assert_eq!(add_trit(a, b, Z), expected, "{a:?} + {b:?}");
    }
}

// --- ワード加算 ---

fn add_exhaustive<const N: usize>() {
    let h = Word::<N>::MAX;
    for x in -h..=h {
        for y in -h..=h {
            for cin in ALL {
                let (r, cout) = w::<N>(x).add_carry(w::<N>(y), cin);
                let total = x + y + cin as i64;
                assert_eq!(r.to_i64() + cout as i64 * Word::<N>::MODULUS, total);
                assert_eq!(r.to_i64(), bmod(total, N));
                assert_eq!(cout != Trit::Z, total.abs() > h);
            }
        }
    }
}

#[test]
fn add_1_trit_exhaustive() {
    add_exhaustive::<1>();
}

/// 27 x 27 x 3(キャリー入力)= 2187 通り
#[test]
fn add_3_trits_exhaustive() {
    add_exhaustive::<3>();
}

#[test]
fn add_4_trits_exhaustive() {
    add_exhaustive::<4>();
}

/// 729 x 729 x 3 ≈ 160 万通り
#[test]
fn add_6_trits_exhaustive() {
    add_exhaustive::<6>();
}

/// 19683² ≈ 3.9 億通り。重いので通常は除外: cargo test --release -- --ignored
#[test]
#[ignore]
fn add_9_trits_exhaustive() {
    let h = Word9::MAX;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()) as i64;
    std::thread::scope(|s| {
        for k in 0..threads {
            s.spawn(move || {
                let mut x = -h + k;
                while x <= h {
                    let a = w::<9>(x);
                    for y in -h..=h {
                        let (r, c) = a.add_carry(w::<9>(y), Trit::Z);
                        assert_eq!(r.to_i64() + c as i64 * Word9::MODULUS, x + y);
                    }
                    x += threads;
                }
            });
        }
    });
}

#[test]
fn add_operator_wraps() {
    assert_eq!((w::<3>(13) + w::<3>(1)).to_i64(), -13);
    assert_eq!((w::<3>(3) + w::<3>(5)).to_i64(), 8);
}

/// 2 ワード x 3 トリット = 6 トリット加算と一致すること(多倍長加算)
#[test]
fn multiword_add_via_carry() {
    let h6 = Word6::MAX;
    for x in (-h6..=h6).step_by(7) {
        for y in (-h6..=h6).step_by(11) {
            let (xw, yw) = (w::<6>(x), w::<6>(y));
            let lo = |v: Word6| Word3::from_slice(&v.trits()[..3]).unwrap();
            let hi = |v: Word6| Word3::from_slice(&v.trits()[3..]).unwrap();
            let (l, c) = lo(xw).add_carry(lo(yw), Trit::Z);
            let (u, cout) = hi(xw).add_carry(hi(yw), c);
            assert_eq!(l.to_i64() + u.to_i64() * 27 + cout as i64 * 729, x + y);
        }
    }
}

#[test]
fn sub() {
    let h = Word3::MAX;
    for x in -h..=h {
        for y in -h..=h {
            assert_eq!((w::<3>(x) - w::<3>(y)).to_i64(), bmod(x - y, 3));
        }
    }
}

#[test]
fn three_added_five_times() {
    let mut acc = Word4::ZERO;
    for _ in 0..5 {
        let (r, c) = acc.add_carry(w::<4>(3), Trit::Z);
        assert_eq!(c, Trit::Z);
        acc = r;
    }
    assert_eq!(acc.to_i64(), 15);
}

// --- 符号反転・符号 ---

fn neg_sign_exhaustive<const N: usize>() {
    for v in -Word::<N>::MAX..=Word::<N>::MAX {
        let x = w::<N>(v);
        assert_eq!((-x).to_i64(), -v);
        assert_eq!(*(-x).trits(), x.trits().map(Trit::sti));
        assert_eq!(x.sign() as i64, v.signum());
    }
}

#[test]
fn neg_and_sign_exhaustive() {
    neg_sign_exhaustive::<1>();
    neg_sign_exhaustive::<3>();
    neg_sign_exhaustive::<6>();
    neg_sign_exhaustive::<9>();
}

// --- シフト ---

#[test]
fn shl() {
    for v in -Word5::MAX..=Word5::MAX {
        for k in 0..7 {
            assert_eq!((w::<5>(v) << k).to_i64(), bmod(v * 3i64.pow(k as u32), 5));
        }
    }
}

#[test]
fn shr_rounds_to_nearest() {
    for v in -Word5::MAX..=Word5::MAX {
        for k in 0..7 {
            let q = (w::<5>(v) >> k).to_i64();
            let p = 3i64.pow(k as u32);
            // 最も近い整数(誤差は 1/2 未満で、同点は発生しない)
            assert!((v - q * p).abs() * 2 < p, "v={v} k={k} q={q}");
        }
    }
}

// --- 3 値論理 ---

#[test]
fn inverters() {
    use Trit::{N, P, Z};
    assert_eq!(ALL.map(Trit::sti), [P, Z, N]);
    assert_eq!(ALL.map(Trit::nti), [P, N, N]);
    assert_eq!(ALL.map(Trit::pti), [P, P, N]);
}

#[test]
fn min_max() {
    for a in ALL {
        for b in ALL {
            assert_eq!(a.min3(b) as i8, (a as i8).min(b as i8));
            assert_eq!(a.max3(b) as i8, (a as i8).max(b as i8));
            // De Morgan: STI(MIN(a,b)) == MAX(STI a, STI b)
            assert_eq!(a.min3(b).sti(), a.sti().max3(b.sti()));
        }
    }
}
