use std::fmt;

#[derive(Debug, Clone)]
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

    pub fn push(&mut self, row: impl IntoIterator<Item = impl Into<String>>) {
        self.rows.push(row.into_iter().map(Into::into).collect());
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let columns = self
            .headers
            .len()
            .max(self.rows.iter().map(Vec::len).max().unwrap_or(0));
        let mut widths = vec![0usize; columns];
        for (index, header) in self.headers.iter().enumerate() {
            widths[index] = widths[index].max(header.len());
        }
        for row in &self.rows {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(cell.len());
            }
        }

        write_row(f, &self.headers, &widths)?;
        for (index, width) in widths.iter().enumerate() {
            if index > 0 {
                f.write_str("  ")?;
            }
            for _ in 0..*width {
                f.write_str("-")?;
            }
        }
        f.write_str("\n")?;
        for row in &self.rows {
            write_row(f, row, &widths)?;
        }
        Ok(())
    }
}

fn write_row(f: &mut fmt::Formatter<'_>, row: &[String], widths: &[usize]) -> fmt::Result {
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            f.write_str("  ")?;
        }
        let cell = row.get(index).map_or("", String::as_str);
        write!(f, "{cell:<width$}")?;
    }
    f.write_str("\n")
}
