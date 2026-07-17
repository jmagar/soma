//! A minimal fixed-width table renderer for terminal output.
//!
//! This is deliberately simple — left-aligned columns sized to their widest
//! cell, two-space gutters, no box-drawing. It is meant for CLI status/list
//! output, not full terminal UI.

/// A table with a header row and any number of body rows.
#[derive(Debug, Clone, Default)]
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            headers: headers.into_iter().map(Into::into).collect(),
            rows: Vec::new(),
        }
    }

    /// Append a row. Panics in debug builds if the row width does not match
    /// the header width — a malformed table is a programming error, not
    /// something to render silently wrong.
    pub fn push_row(&mut self, row: impl IntoIterator<Item = impl Into<String>>) -> &mut Self {
        let row: Vec<String> = row.into_iter().map(Into::into).collect();
        debug_assert_eq!(
            row.len(),
            self.headers.len(),
            "table row width must match header width"
        );
        self.rows.push(row);
        self
    }

    /// Render the table as left-aligned, two-space-gutter columns.
    pub fn render(&self) -> String {
        let columns = self.headers.len();
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.chars().count()).collect();
        for row in &self.rows {
            for (index, cell) in row.iter().enumerate().take(columns) {
                let width = cell.chars().count();
                if width > widths[index] {
                    widths[index] = width;
                }
            }
        }

        let mut lines = Vec::with_capacity(self.rows.len() + 1);
        lines.push(render_row(&self.headers, &widths));
        for row in &self.rows {
            lines.push(render_row(row, &widths));
        }
        lines.join("\n")
    }
}

fn render_row(cells: &[String], widths: &[usize]) -> String {
    cells
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            let width = widths.get(index).copied().unwrap_or(0);
            if index + 1 == cells.len() {
                // Last column: no trailing padding.
                cell.clone()
            } else {
                format!("{cell:<width$}")
            }
        })
        .collect::<Vec<_>>()
        .join("  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_aligned_columns() {
        let mut table = Table::new(["NAME", "STATUS"]);
        table.push_row(["alpha", "ok"]);
        table.push_row(["beta-long", "failed"]);
        let rendered = table.render();
        assert_eq!(
            rendered,
            "NAME       STATUS\nalpha      ok\nbeta-long  failed"
        );
    }

    #[test]
    fn empty_table_renders_header_only() {
        let table = Table::new(["A", "B"]);
        assert_eq!(table.render(), "A  B");
    }
}
