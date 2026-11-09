//! A column-major, in-memory table: the input of statistical discovery.
//!
//! Connectors will fill these from a source database (sampling or sketching for very large tables
//! is a connector concern); the loader here reads the tab-separated form the benchmark uses.

use std::fmt;

/// One column: a name and one optional value per row (`None` is SQL `NULL`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// Column name as the source spells it.
    pub name: String,
    /// One entry per row.
    pub values: Vec<Option<String>>,
}

/// A table: a name and its columns, all the same length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    /// Table name as the source spells it.
    pub name: String,
    /// Columns in source order.
    pub columns: Vec<Column>,
}

/// Why a table could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableError {
    /// No header line.
    Empty,
    /// A data line has a different number of cells than the header (line numbers start at 1).
    RaggedRow {
        /// 1-based line number.
        line: usize,
        /// Cells the header declares.
        expected: usize,
        /// Cells the line has.
        found: usize,
    },
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "no header line"),
            Self::RaggedRow {
                line,
                expected,
                found,
            } => {
                write!(f, "line {line}: expected {expected} cells, found {found}")
            }
        }
    }
}

impl std::error::Error for TableError {}

impl Table {
    /// Number of rows (zero for a table with no columns).
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.columns.first().map_or(0, |c| c.values.len())
    }

    /// Reads tab-separated text: a header line, then one line per row; the two characters `\N`
    /// mean `NULL`. Blank trailing lines are ignored.
    ///
    /// # Errors
    ///
    /// [`TableError::Empty`] without a header, [`TableError::RaggedRow`] when a row's width differs.
    pub fn from_tsv(name: &str, text: &str) -> Result<Self, TableError> {
        let mut lines = text.lines();
        let header = lines
            .next()
            .filter(|h| !h.is_empty())
            .ok_or(TableError::Empty)?;
        let mut columns: Vec<Column> = header
            .split('\t')
            .map(|n| Column {
                name: n.to_owned(),
                values: Vec::new(),
            })
            .collect();
        for (i, line) in lines.enumerate() {
            if line.is_empty() {
                continue;
            }
            let cells: Vec<&str> = line.split('\t').collect();
            if cells.len() != columns.len() {
                return Err(TableError::RaggedRow {
                    line: i + 2,
                    expected: columns.len(),
                    found: cells.len(),
                });
            }
            for (col, cell) in columns.iter_mut().zip(cells) {
                col.values.push(if cell == "\\N" {
                    None
                } else {
                    Some(cell.to_owned())
                });
            }
        }
        Ok(Self {
            name: name.to_owned(),
            columns,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Table, TableError};

    #[test]
    fn reads_values_and_nulls() {
        let t = Table::from_tsv("T", "a\tb\n1\t\\N\n2\tx\n\n").unwrap();
        assert_eq!(t.row_count(), 2);
        assert_eq!(
            t.columns[0].values,
            vec![Some("1".into()), Some("2".into())]
        );
        assert_eq!(t.columns[1].values, vec![None, Some("x".into())]);
    }

    #[test]
    fn header_only_is_an_empty_table() {
        assert_eq!(Table::from_tsv("T", "a\tb\n").unwrap().row_count(), 0);
    }

    #[test]
    fn rejects_missing_header_and_ragged_rows() {
        assert_eq!(Table::from_tsv("T", ""), Err(TableError::Empty));
        assert_eq!(
            Table::from_tsv("T", "a\tb\n1\n"),
            Err(TableError::RaggedRow {
                line: 2,
                expected: 2,
                found: 1
            })
        );
    }
}
