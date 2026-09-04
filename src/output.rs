#[derive(Debug, Clone)]
pub struct Table<const COLUMNS: usize> {
    headers: [String; COLUMNS],
    rows: Vec<[String; COLUMNS]>,
}

impl<const COLUMNS: usize> Table<COLUMNS> {
    pub fn new<T: Into<String>>(headers: [T; COLUMNS]) -> Self {
        Self {
            headers: headers.map(Into::into),
            rows: Vec::new(),
        }
    }

    pub fn push_row<T: Into<String>>(&mut self, row: [T; COLUMNS]) {
        self.rows.push(row.map(Into::into));
    }

    pub fn print(&self) {
        print!("{}", self);
    }

    fn widths(&self) -> Vec<usize> {
        let mut widths = self
            .headers
            .iter()
            .map(|header| header.len())
            .collect::<Vec<_>>();

        for row in &self.rows {
            for (index, value) in row.iter().enumerate() {
                widths[index] = widths[index].max(value.len());
            }
        }

        widths
    }
}

impl<const COLUMNS: usize> std::fmt::Display for Table<COLUMNS> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let widths = self.widths();

        write_row(formatter, &self.headers, &widths)?;
        for (index, width) in widths.iter().enumerate() {
            if index > 0 {
                write!(formatter, "  ")?;
            }
            write!(formatter, "{:-<width$}", "", width = width)?;
        }
        writeln!(formatter)?;

        for row in &self.rows {
            write_row(formatter, row, &widths)?;
        }

        Ok(())
    }
}

fn write_row(
    formatter: &mut std::fmt::Formatter<'_>,
    row: &[String],
    widths: &[usize],
) -> std::fmt::Result {
    for (index, value) in row.iter().enumerate() {
        if index > 0 {
            write!(formatter, "  ")?;
        }
        write!(formatter, "{value:<width$}", width = widths[index])?;
    }
    writeln!(formatter)
}

#[cfg(test)]
mod tests {
    use super::Table;

    #[test]
    fn renders_aligned_tables() {
        let mut table = Table::new(["ID", "STATUS"]);
        table.push_row(["java-21-trixie", "stable"]);
        table.push_row(["scratch-1", "experimental"]);

        assert_eq!(
            table.to_string(),
            "ID              STATUS      \n\
             --------------  ------------\n\
             java-21-trixie  stable      \n\
             scratch-1       experimental\n"
        );
    }
}
