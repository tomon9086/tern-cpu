#!/usr/bin/env python3
"""M4: 3 値ゲートの SPICE 特性評価(docs/spice.md)。

STI の各方式について、温度・hFE・負荷を振って DC 特性を測り、最悪条件のノイズマージンを出す。
使い方: python3 spice/sim.py [--detail]
必要なもの: ngspice(標準ライブラリ以外の Python パッケージは不要)
"""

import itertools
import pathlib
import re
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
LIB = HERE / "models" / "discrete.lib"

CELLS = {
    "STI 抵抗分圧": ("STI_DIV", HERE / "cells" / "sti_div.sub"),
    "STI ツェナー": ("STI_ZENER", HERE / "cells" / "sti_zener.sub"),
}

TEMPS = [0, 25, 70]
BFSCALES = [0.5, 1.0, 2.0]

# 負荷(出力 out にぶら下げるもの)。{cell} は評価中のセル名
LOADS = {
    "無負荷": "",
    "同じゲート3個": "\n".join(f"XL{i} out l{i} vp vn {{cell}}" for i in range(3)),
    # MIN ゲート: 入力側がカソード、共通ノード lm を抵抗で +5V に引く(SPICE の D は アノード カソード の順)
    "MINゲート入力(47k)": "DL lm out 1N4148\nRL vp lm 47k",
    # MAX ゲート: 入力側がアノード、共通ノードを -5V に引く
    "MAXゲート入力(47k)": "DL out lm 1N4148\nRL lm vn 47k",
}

TB = """* testbench
.include {lib}
.include {sub}
.param BFSCALE={bf}
.temp {temp}
VP vp 0 5
VN vn 0 -5
VIN in 0 0
XDUT in out vp vn {cell}
{load}
.control
dc VIN -5.1 5.1 0.01
meas dc tlo when v(out)=2.5 fall=1
meas dc thi when v(out)=-2.5 fall=1
meas dc vpos find v(out) at=-5
meas dc vzero find v(out) at=0
meas dc vneg find v(out) at=5
meas dc ip0 find i(VP) at=0
meas dc ipn find i(VP) at=-5
.endc
.end
"""


def run(cell, sub, load, temp, bf):
    net = TB.format(lib=LIB, sub=sub, cell=cell, load=load.replace("{cell}", cell), temp=temp, bf=bf)
    with tempfile.NamedTemporaryFile("w", suffix=".cir", delete=False) as f:
        f.write(net)
        path = f.name
    out = subprocess.run(["ngspice", "-b", path], capture_output=True, text=True).stdout
    pathlib.Path(path).unlink()
    vals = {}
    for m in re.finditer(r"^(\w+)\s+=\s+([-+0-9.eE]+)", out, re.M):
        vals[m.group(1)] = float(m.group(2))
    missing = {"tlo", "thi", "vpos", "vzero", "vneg"} - vals.keys()
    if missing:
        raise RuntimeError(f"{cell} {temp}C bf={bf}: measurement failed {missing}\n{out[-2000:]}")
    return vals


def margins(v):
    """ノイズマージン: 出力レベルが、同じゲートのしきい値からどれだけ離れているか"""
    return {
        "NM+": v["vpos"] - v["thi"],
        "NM0上": v["thi"] - v["vzero"],
        "NM0下": v["vzero"] - v["tlo"],
        "NM-": v["tlo"] - v["vneg"],
    }


def main():
    detail = "--detail" in sys.argv
    print("| セル | 負荷 | 出力+ (最小) | 出力0 (範囲) | 出力- (最大) | しきい値- (範囲) | しきい値+ (範囲) | 最悪ノイズマージン |")
    print("|---|---|---:|---:|---:|---:|---:|---:|")
    for name, (cell, sub) in CELLS.items():
        for lname, load in LOADS.items():
            rs = []
            for temp, bf in itertools.product(TEMPS, BFSCALES):
                v = run(cell, sub, load, temp, bf)
                rs.append(((temp, bf), v))
                if detail:
                    m = margins(v)
                    print(f"<!-- {name} {lname} {temp}C bf×{bf}: " + " ".join(f"{k}={x:+.3f}" for k, x in v.items()) + " " + " ".join(f"{k}={x:.2f}" for k, x in m.items()) + " -->")
            col = lambda k: [v[k] for _, v in rs]
            # 最悪条件: 出力レベルとしきい値を別々の条件から組み合わせる(悲観的)
            worst = {
                "NM+": min(col("vpos")) - max(col("thi")),
                "NM0上": min(col("thi")) - max(col("vzero")),
                "NM0下": min(col("vzero")) - max(col("tlo")),
                "NM-": min(col("tlo")) - max(col("vneg")),
            }
            k, nm = min(worst.items(), key=lambda kv: kv[1])
            rng = lambda key: f"{min(col(key)):+.2f}〜{max(col(key)):+.2f}"
            print(
                f"| {name} | {lname} | {min(col('vpos')):+.2f} | {rng('vzero')} | {max(col('vneg')):+.2f} "
                f"| {rng('tlo')} | {rng('thi')} | {nm:.2f}V ({k}) |"
            )


if __name__ == "__main__":
    main()
