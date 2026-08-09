use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct Column {
    pub name: String,
    pub values: Vec<f64>, // NaN for missing / non-numeric
    pub numeric: bool,
}

pub struct DataFile {
    pub path: PathBuf,
    pub name: String,
    pub columns: Vec<Column>,
    pub selected: Vec<bool>,
    pub visible: bool,
    pub nrows: usize,
    /// On-disk size when this snapshot was loaded.
    pub file_len: u64,
    /// On-disk mtime when this snapshot was loaded.
    pub mtime: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    Unchanged,
    Reloaded,
}

impl DataFile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let mut file = Self::parse(path, &content)?;
        file.capture_disk_meta()?;
        Ok(file)
    }

    /// Parse tabular content. Exposed for unit tests.
    pub fn parse(path: &Path, content: &str) -> Result<Self, String> {
        let delimiter = sniff_delimiter(content);
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(content.as_bytes());
        let names: Vec<String> = rdr
            .headers()
            .map_err(|e| e.to_string())?
            .iter()
            .map(|s| s.to_string())
            .collect();
        if names.is_empty() {
            return Err("no header row found".into());
        }
        let ncols = names.len();
        let mut raw: Vec<Vec<f64>> = vec![Vec::new(); ncols];
        let mut nrows = 0usize;
        for rec in rdr.records() {
            let rec = rec.map_err(|e| e.to_string())?;
            for (i, col) in raw.iter_mut().enumerate().take(ncols) {
                let cell = rec.get(i).unwrap_or("");
                col.push(cell.parse::<f64>().unwrap_or(f64::NAN));
            }
            nrows += 1;
        }
        let columns: Vec<Column> = names
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                let values = std::mem::take(&mut raw[i]);
                let finite = values.iter().filter(|v| v.is_finite()).count();
                let numeric = finite > 0 && finite * 2 >= nrows.max(1);
                Column {
                    name,
                    values,
                    numeric,
                }
            })
            .collect();
        let selected = vec![false; ncols];
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        Ok(Self {
            path: path.to_path_buf(),
            name,
            columns,
            selected,
            visible: true,
            nrows,
            file_len: 0,
            mtime: None,
        })
    }

    fn capture_disk_meta(&mut self) -> Result<(), String> {
        let meta =
            std::fs::metadata(&self.path).map_err(|e| format!("{}: {}", self.path.display(), e))?;
        self.file_len = meta.len();
        self.mtime = meta.modified().ok();
        Ok(())
    }

    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    /// Re-read from disk if size or mtime changed. Preserves visibility and
    /// column selection (matched by column name).
    pub fn refresh_from_disk(&mut self) -> Result<RefreshOutcome, String> {
        let meta =
            std::fs::metadata(&self.path).map_err(|e| format!("{}: {}", self.path.display(), e))?;
        let len = meta.len();
        let mtime = meta.modified().ok();
        if len == self.file_len && mtime == self.mtime {
            return Ok(RefreshOutcome::Unchanged);
        }

        let mut fresh = Self::load(&self.path)?;
        fresh.visible = self.visible;
        for (i, col) in fresh.columns.iter().enumerate() {
            if let Some(old_i) = self.column_index(&col.name)
                && let Some(sel) = self.selected.get(old_i)
            {
                fresh.selected[i] = *sel;
            }
        }
        *self = fresh;
        Ok(RefreshOutcome::Reloaded)
    }
}

pub fn sniff_delimiter(content: &str) -> u8 {
    let first = content.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let tabs = first.matches('\t').count();
    let commas = first.matches(',').count();
    let semis = first.matches(';').count();
    if tabs >= commas && tabs >= semis && tabs > 0 {
        b'\t'
    } else if semis > commas && semis > 0 {
        b';'
    } else {
        b','
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::Path;

    #[test]
    fn sniff_csv_comma() {
        assert_eq!(sniff_delimiter("a,b,c\n1,2,3\n"), b',');
    }

    #[test]
    fn sniff_tsv_tab() {
        assert_eq!(sniff_delimiter("a\tb\tc\n1\t2\t3\n"), b'\t');
    }

    #[test]
    fn sniff_semicolon() {
        assert_eq!(sniff_delimiter("a;b;c\n1;2;3\n"), b';');
    }

    #[test]
    fn parse_basic_csv() {
        let csv = "step,loss,lr\n1,0.5,0.01\n2,0.4,0.01\n3,0.3,0.005\n";
        let f = DataFile::parse(Path::new("run.csv"), csv).unwrap();
        assert_eq!(f.name, "run.csv");
        assert_eq!(f.nrows, 3);
        assert_eq!(f.columns.len(), 3);
        assert!(f.columns.iter().all(|c| c.numeric));
        assert_eq!(f.columns[1].values[0], 0.5);
        assert_eq!(f.column_index("loss"), Some(1));
    }

    #[test]
    fn parse_marks_non_numeric_columns() {
        let csv = "name,score\nalice,1\nbob,2\n";
        let f = DataFile::parse(Path::new("x.csv"), csv).unwrap();
        assert!(!f.columns[0].numeric);
        assert!(f.columns[1].numeric);
    }

    #[test]
    fn parse_empty_headers_errors() {
        match DataFile::parse(Path::new("x.csv"), "\n") {
            Ok(_) => panic!("expected error for empty input"),
            Err(err) => assert!(!err.is_empty()),
        }
    }

    #[test]
    fn parse_missing_cells_as_nan() {
        let csv = "a,b\n1,\n2,3\n";
        let f = DataFile::parse(Path::new("x.csv"), csv).unwrap();
        assert!(f.columns[1].values[0].is_nan());
        assert_eq!(f.columns[1].values[1], 3.0);
    }

    #[test]
    fn refresh_reloads_appended_rows_and_keeps_selection() {
        let dir = std::env::temp_dir().join(format!("tview-refresh-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("run.csv");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "step,loss").unwrap();
            writeln!(f, "1,1.0").unwrap();
            writeln!(f, "2,0.5").unwrap();
        }

        let mut file = DataFile::load(&path).unwrap();
        file.selected[1] = true; // loss
        assert_eq!(file.nrows, 2);
        assert_eq!(file.refresh_from_disk().unwrap(), RefreshOutcome::Unchanged);

        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "3,0.25").unwrap();
        }
        // Ensure mtime/size differ on fast filesystems.
        let _ = std::fs::File::open(&path).and_then(|f| f.sync_all());

        assert_eq!(file.refresh_from_disk().unwrap(), RefreshOutcome::Reloaded);
        assert_eq!(file.nrows, 3);
        assert!(file.selected[1]);
        assert_eq!(file.columns[1].values[2], 0.25);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}
