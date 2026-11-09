# Evaluation guide for pilot partners

This guide covers the `sov` evaluation build. It is not a production product; read [`STATUS.md`](STATUS.md) and `EVALUATION-NOTICE.md` first.

## 1. Verify what you received

```sh
sha256sum -c SHA256SUMS --ignore-missing      # or: ./sov digest <file>
tar -xzf sov-0.1.0-linux-x86_64.tar.gz && cd sov-0.1.0-linux-x86_64
./sov version
```
Compare the checksum with the one we sent you by a separate channel. Signed releases will also carry a Sigstore bundle and build provenance; the first builds may not.

## 2. See what it does (no data of yours needed)

```sh
./sov demo                       # a tour on a synthetic database; prints a digest
./sov demo --export ./sample     # writes that database as tab-separated files
./sov profile ./sample           # the same report, from files, like you will run on yours
```
Run the demo twice: the digest at the end is identical. That is deliberate: reports are reproducible.

## 3. Prepare your own export

The tool reads a **directory of `.tsv` files, one per table**. The file name (without `.tsv`) is the table name. First line is the column names; fields are separated by tabs; write `\N` for NULL; no quoting. Include the Maximo dictionary as `MAXRELATIONSHIP.tsv` (columns `NAME`, `PARENT`, `CHILD`, `WHERECLAUSE`), or pass `--dictionary FILE`.

- Export from a **restored backup or reporting replica**, not live production.
- **Leave out sensitive columns** you do not want analysed (personal data, free-text notes). Relationship discovery does not need them.
- Any tool that writes tab-separated text will do. We have only tested files produced by our own exporter, so please tell us about every file that fails to load.

## 4. Run and read the report

```sh
./sov profile ./your-export                 # text report
./sov profile ./your-export --json          # same facts as JSON
./sov profile ./your-export --max-rows 50000
```
Each relationship line shows `FK_TABLE(cols) -> REF_TABLE(cols)`, where it came from (`dictionary`, `inferred` or `both`), whether it is `many-to-one` or `one-to-one`, and **orphans** (rows whose key matches nothing). `REVIEW` means the data could not settle something and a person should confirm. `Not understood` lists dictionary clauses outside the supported subset, with the reason. Tables cut by the row limit are named in the report: results for them come from their first rows only.

## 5. Check stored balances against transactions

```sh
./sov audit ./your-export --tx INVTRANS --tx-keys ITEMNUM,STORELOC,SITEID --tx-amount QUANTITY \
  --bal INVBALANCES --bal-keys ITEMNUM,LOCATION,SITEID --bal-amount CURBAL
```
The audit prints the key values of balances that disagree: review it before sharing.

## 6. Known limits

Everything runs in memory; very large tables are cut to the row limit. Inference misses links whose column names share nothing with their target (for example `REPORTEDBY` to `PERSON`) and links into very small tables; the dictionary covers declared ones. Keys of three or more columns are found only through the dictionary. We have not yet run the tool on a real Maximo database, so treat early results as a conversation starter, and tell us where they are wrong.

## 7. What to send us

Whatever you are comfortable with. The text and JSON reports contain table and column names, counts and a digest but no row values (check yours before sending; the audit report does print key values). Tell us what you expected, what you saw, and the tool version (`./sov version`).
