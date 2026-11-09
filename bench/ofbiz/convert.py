#!/usr/bin/env python3
"""Convert Apache OFBiz entity definitions and demo/seed data into an FK-stripping benchmark.

Reads  <ofbiz-checkout>/{applications,framework}/**/entitydef/*entitymodel*.xml  (schema, declared relations)
       <ofbiz-checkout>/{applications,framework}/**/data/**/*.xml                (rows)
Writes <out>/tables/<Entity>.tsv   header + rows, NULL as \\N, no keys, no relations (what the profiler sees)
       <out>/truth.tsv             declared relationships (what the profiler must rediscover)
       <out>/tables.tsv            table -> module

Research tooling, not product code: it exists so the benchmark is reproducible from a pinned OFBiz commit.
Rows follow the loader's upsert semantics (one row per primary key). Only the standard library is used.
"""
import os, sys, glob, xml.etree.ElementTree as ET

def module_of(path):
    base = os.path.basename(path)
    return base.replace("-entitymodel", "").replace("_test", "").replace(".xml", "") if "applications" in path else "framework"

def main(checkout, out):
    entities = {}   # name -> {"fields":[...], "module":..., "relations":[...]}
    for path in sorted(glob.glob(os.path.join(checkout, "**", "entitydef", "*entitymodel*.xml"), recursive=True)):
        if "entitymodel_test" in path or "entitymodel-secret" in path:
            continue
        root = ET.parse(path).getroot()
        for e in root.iter("entity"):
            name = e.get("entity-name")
            fields = [f.get("name") for f in e.findall("field")]
            pk = [k.get("field") for k in e.findall("prim-key")]
            rels = []
            for r in e.findall("relation"):
                if r.get("type") not in ("one", "one-nofk", "many"):
                    continue
                kms = [(k.get("field-name"), k.get("rel-field-name") or k.get("field-name")) for k in r.findall("key-map")]
                if kms:
                    rels.append((r.get("rel-entity-name"), kms, r.get("type")))
            entities[name] = {"fields": fields, "module": module_of(path), "relations": rels, "rows": [], "pk": pk, "index": {}}
    unknown = 0
    for path in sorted(glob.glob(os.path.join(checkout, "**", "data", "**", "*.xml"), recursive=True)):
        try:
            root = ET.parse(path).getroot()
        except ET.ParseError:
            continue
        if root.tag != "entity-engine-xml":
            continue
        for row in root:
            ent = entities.get(row.tag)
            if ent is None:
                unknown += 1
                continue
            vals = dict(row.attrib)
            for child in row:
                if child.text and child.text.strip():
                    vals[child.tag] = child.text.strip()
            # entity-engine-xml is an upsert: a later element with the same primary key updates the
            # row, so the database holds one row per key. Merge the same way.
            if ent["pk"] and all(k in vals for k in ent["pk"]):
                key = tuple(vals[k] for k in ent["pk"])
                if key in ent["index"]:
                    ent["index"][key].update(vals)
                    continue
                ent["index"][key] = vals
            ent["rows"].append(vals)
    os.makedirs(os.path.join(out, "tables"), exist_ok=True)
    def cell(v):
        return "\\N" if v is None else v.replace("\t", " ").replace("\n", " ").replace("\r", " ")
    with open(os.path.join(out, "tables.tsv"), "w") as meta:
        for name, ent in sorted(entities.items()):
            meta.write(f"{name}\t{ent['module']}\t{len(ent['rows'])}\n")
            with open(os.path.join(out, "tables", name + ".tsv"), "w") as f:
                f.write("\t".join(ent["fields"]) + "\n")
                for vals in ent["rows"]:
                    f.write("\t".join(cell(vals.get(c)) for c in ent["fields"]) + "\n")
    # Normalise to (child, child_cols, parent, parent_cols, kind). A parent-side "many" relation declares the
    # same foreign key from the other end: the related entity is the child.
    declared = []
    for name, ent in sorted(entities.items()):
        for rel, kms, kind in ent["relations"]:
            if kind == "many":
                declared.append((rel, [p for _, p in kms], name, [c for c, _ in kms], "many-reverse"))
            else:
                declared.append((name, [c for c, _ in kms], rel, [p for _, p in kms], kind))
    seen, n_rel, skipped = set(), 0, 0
    with open(os.path.join(out, "truth.tsv"), "w") as t:
        t.write("child_table\tchild_cols\tparent_table\tparent_cols\tmodule\tkind\n")
        # forward declarations first, so a relationship declared both ways keeps its forward kind
        for child, ccols, parent, pcols, kind in sorted(declared, key=lambda d: (d[4] == "many-reverse", d[0], d[1])):
            if child not in entities or parent not in entities:
                skipped += 1
                continue
            if any(c not in entities[child]["fields"] for c in ccols) or any(p not in entities[parent]["fields"] for p in pcols):
                skipped += 1
                continue
            key = (child, tuple(ccols), parent, tuple(pcols))
            if key in seen:
                continue
            seen.add(key)
            n_rel += 1
            t.write(f"{child}\t{','.join(ccols)}\t{parent}\t{','.join(pcols)}\t{entities[child]['module']}\t{kind}\n")
    nonempty = sum(1 for e in entities.values() if e["rows"])
    total_rows = sum(len(e["rows"]) for e in entities.values())
    print(f"entities={len(entities)} nonempty={nonempty} rows={total_rows} declared_relations={n_rel} skipped_relations={skipped} data_elements_for_unknown_entities={unknown}")

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
