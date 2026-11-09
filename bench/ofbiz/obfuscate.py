#!/usr/bin/env python3
"""Rewrite a benchmark dataset with opaque names: tables T0001.., columns C01.. (by position).

Usage: obfuscate.py <in-dir> <out-dir> [--shuffle-columns]
Values, row order, column order and the declared relationships are preserved; only names change. This
simulates schemas whose names carry no meaning (T001/C1 style), so name evidence is truly absent.
With --shuffle-columns the column order of every table is also permuted (deterministically), removing the
"keys come first" positional cue as well.
"""
import csv, hashlib, os, sys

def main(src, dst, shuffle=False):
    os.makedirs(os.path.join(dst, "tables"), exist_ok=True)
    names = sorted(f[:-4] for f in os.listdir(os.path.join(src, "tables")) if f.endswith(".tsv"))
    tmap = {n: f"T{i + 1:04d}" for i, n in enumerate(names)}
    cmap = {}
    for n in names:
        with open(os.path.join(src, "tables", n + ".tsv")) as f:
            lines = f.read().split("\n")
        header = lines[0].split("\t")
        order = list(range(len(header)))
        if shuffle:
            order.sort(key=lambda i: hashlib.sha256((n + "/" + header[i]).encode()).hexdigest())
        cmap[n] = {header[i]: f"C{j + 1:02d}" for j, i in enumerate(order)}
        out_lines = ["\t".join(cmap[n][header[i]] for i in order)]
        for line in lines[1:]:
            if line == "":
                continue
            cells = line.split("\t")
            out_lines.append("\t".join(cells[i] for i in order))
        with open(os.path.join(dst, "tables", tmap[n] + ".tsv"), "w") as g:
            g.write("\n".join(out_lines) + "\n")
    with open(os.path.join(src, "tables.tsv")) as f, open(os.path.join(dst, "tables.tsv"), "w") as g:
        for row in csv.reader(f, delimiter="\t"):
            g.write("\t".join([tmap.get(row[0], row[0])] + row[1:]) + "\n")
    with open(os.path.join(src, "truth.tsv")) as f, open(os.path.join(dst, "truth.tsv"), "w") as g:
        rows = list(csv.reader(f, delimiter="\t"))
        g.write("\t".join(rows[0]) + "\n")
        for r in rows[1:]:
            ct, cc, pt, pc = r[0], r[1].split(","), r[2], r[3].split(",")
            r[0], r[1] = tmap[ct], ",".join(cmap[ct][c] for c in cc)
            r[2], r[3] = tmap[pt], ",".join(cmap[pt][c] for c in pc)
            g.write("\t".join(r) + "\n")
    print(f"obfuscated {len(names)} tables")

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], "--shuffle-columns" in sys.argv)
