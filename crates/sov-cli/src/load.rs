//! Reading table exports from disk.

use sov_profiler::maximo::RelationshipRow;
use sov_profiler::table::Table;
use std::path::Path;

/// Rows kept per table unless the user asks otherwise. Discovery compares every column pair, so very
/// large tables are cut to their first rows and the report says so.
pub const DEFAULT_MAX_ROWS: usize = 200_000;

/// Everything read from a directory of exports.
pub struct Loaded {
    /// Every table, including a dictionary table if present.
    pub all: Vec<Table>,
    /// Tables used for relationship inference (the dictionary table is excluded).
    pub inferable: Vec<Table>,
    /// Relationship rows from a table named `MAXRELATIONSHIP`, if there is one.
    pub dictionary: Vec<RelationshipRow>,
    /// Tables cut to the row limit.
    pub truncated: Vec<String>,
    /// Total rows kept.
    pub rows: usize,
}

fn is_dictionary(t: &Table) -> bool {
    t.name.eq_ignore_ascii_case("MAXRELATIONSHIP")
}

fn column<'a>(t: &'a Table, name: &str) -> Result<&'a [Option<String>], String> {
    t.columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(name))
        .map(|c| c.values.as_slice())
        .ok_or_else(|| format!("dictionary {} has no column {name}", t.name))
}

fn dictionary_from_table(t: &Table) -> Result<Vec<RelationshipRow>, String> {
    let (name, parent, child) = (
        column(t, "NAME")?,
        column(t, "PARENT")?,
        column(t, "CHILD")?,
    );
    let clause = column(t, "WHERECLAUSE").or_else(|_| column(t, "WHERE_CLAUSE"))?;
    let text = |c: &[Option<String>], i: usize| c[i].clone().unwrap_or_default();
    Ok((0..t.row_count())
        .map(|i| RelationshipRow {
            name: text(name, i),
            parent: text(parent, i),
            child: text(child, i),
            where_clause: text(clause, i),
        })
        .collect())
}

fn read_table(path: &Path, max_rows: usize) -> Result<(Table, bool), String> {
    let name = path
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut table =
        Table::from_tsv(&name, &text).map_err(|e| format!("{}: {e}", path.display()))?;
    let cut = table.row_count() > max_rows;
    if cut {
        for c in &mut table.columns {
            c.values.truncate(max_rows);
        }
    }
    Ok((table, cut))
}

/// Reads a dictionary exported as its own tab-separated file.
pub fn dictionary_file(path: &Path) -> Result<Vec<RelationshipRow>, String> {
    let (table, _) = read_table(path, usize::MAX)?;
    dictionary_from_table(&table)
}

/// Reads every `*.tsv` in `dir`, sorted by file name.
pub fn tables(dir: &Path, max_rows: usize) -> Result<Loaded, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("tsv")))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("no .tsv files in {}", dir.display()));
    }
    let (mut all, mut truncated) = (Vec::new(), Vec::new());
    for f in &files {
        let (t, cut) = read_table(f, max_rows)?;
        if cut {
            truncated.push(t.name.clone());
        }
        all.push(t);
    }
    let dictionary = all
        .iter()
        .find(|t| is_dictionary(t))
        .map(dictionary_from_table)
        .transpose()?
        .unwrap_or_default();
    let inferable: Vec<Table> = all.iter().filter(|t| !is_dictionary(t)).cloned().collect();
    let rows = all.iter().map(Table::row_count).sum();
    Ok(Loaded {
        all,
        inferable,
        dictionary,
        truncated,
        rows,
    })
}
