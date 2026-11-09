#!/usr/bin/env python3
"""Convert a SQLite database with declared foreign keys into an FK-stripping benchmark.

Usage: convert.py <database.sqlite> <out-dir> <corpus-name>
Writes the same layout as bench/ofbiz/convert.py: tables/<Table>.tsv (values only, NULL as \\N),
truth.tsv (declared foreign keys, for scoring only) and tables.tsv (table, module, rows).
Research tooling; standard library only. Databases are downloaded at run time, never committed.
"""
import os, sqlite3, sys

def cell(v):
    if v is None or isinstance(v, bytes):
        return "\\N"
    return str(v).replace("\t", " ").replace("\n", " ").replace("\r", " ")

def main(db, out, corpus):
    con = sqlite3.connect(db)
    tables = [r[0] for r in con.execute("select name from sqlite_master where type='table' and name not like 'sqlite_%' order by name")]
    info = {t: con.execute(f'pragma table_info("{t}")').fetchall() for t in tables}
    os.makedirs(os.path.join(out, "tables"), exist_ok=True)
    with open(os.path.join(out, "tables.tsv"), "w") as meta:
        for t in tables:
            cols = [c[1] for c in info[t]]
            rows = con.execute(f'select * from "{t}"').fetchall()
            with open(os.path.join(out, "tables", t + ".tsv"), "w") as f:
                f.write("\t".join(cols) + "\n")
                for r in rows:
                    f.write("\t".join(cell(v) for v in r) + "\n")
            meta.write(f"{t}\t{corpus}\t{len(rows)}\n")
    n = 0
    with open(os.path.join(out, "truth.tsv"), "w") as truth:
        truth.write("child_table\tchild_cols\tparent_table\tparent_cols\tmodule\tkind\n")
        for t in tables:
            groups = {}
            for fid, seq, parent, frm, to, *_ in con.execute(f'pragma foreign_key_list("{t}")'):
                groups.setdefault(fid, []).append((seq, parent, frm, to))
            for parts in groups.values():
                parts.sort()
                parent = parts[0][1]
                if parent not in info:
                    continue
                pk = [c[1] for c in sorted((c for c in info[parent] if c[5] > 0), key=lambda c: c[5])]
                ccols = [p[2] for p in parts]
                pcols = [p[3] if p[3] else (pk[i] if i < len(pk) else None) for i, p in enumerate(parts)]
                if None in pcols:
                    continue
                truth.write(f"{t}\t{','.join(ccols)}\t{parent}\t{','.join(pcols)}\t{corpus}\tdeclared\n")
                n += 1
    print(f"{corpus}: tables={len(tables)} rows={sum(len(open(os.path.join(out,'tables',t+'.tsv')).readlines())-1 for t in tables)} declared_fks={n}")

if __name__ == "__main__":
    main(*sys.argv[1:4])
