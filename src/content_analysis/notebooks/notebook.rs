//notebook.rs

//! OO-style Jupyter notebook parser and filtered in-memory notebook model.
//!
//! This module intentionally keeps the notebook logic in a single file:
//!
//! - the public [`Notebook`] type,
//! - the public [`NotebookParserConfig`] configuration,
//! - all private parsing and reduction helpers,
//! - and the internal test suite.
//!
//! ## Design intent
//!
//! The parser is not archival. Its purpose is to turn a `.ipynb` file into a
//! compact, AI-friendly in-memory representation that:
//!
//! - drops heavyweight image payloads,
//! - ignores errors,
//! - reduces large outputs,
//! - subsets table-like results,
//! - assigns stable output IDs,
//! - and exposes notebook content in a way that is easy to feed into later AI
//!   analysis stages.
//!
//! ## Public API
//!
//! - [`Notebook::from_file`] parses a notebook file into a filtered [`Notebook`].
//! - [`Notebook::iter_over_markdown`] iterates over markdown cells as `(id, text)`.
//! - [`Notebook::get_for_area`] returns code cells and retained outputs for a
//!   cell range while intentionally ignoring markdown cells.
//!
//! ## Non-goals of this module
//!
//! This module does not perform any AI requests itself. It only prepares the
//! notebook data so that a later helper can ask an AI model useful questions.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

/// Configuration controlling how notebook content is filtered during parsing.
///
/// The defaults aim to preserve useful analytical context while aggressively
/// removing oversized or noisy payloads.
///
/// # Examples
///
/// ```ignore
/// let config = NotebookParserConfig::default();
/// let notebook = Notebook::from_file("analysis.ipynb", config)?;
/// ```
#[derive(Debug, Clone)]
pub struct NotebookParserConfig {
    /// Maximum number of rows retained for a table-like output preview.
    pub max_table_rows: usize,

    /// Maximum number of columns retained for a table-like output preview.
    pub max_table_cols: usize,

    /// Maximum number of lines retained for generic text outputs.
    pub max_text_lines: usize,

    /// Maximum number of characters retained for generic text outputs.
    pub max_text_chars: usize,

    /// Maximum number of items retained for simple list-like outputs.
    pub max_list_items: usize,

    /// Maximum number of characters retained for object-like or unknown outputs.
    pub max_object_chars: usize,
}

impl Default for NotebookParserConfig {
    fn default() -> Self {
        Self {
            max_table_rows: 5,
            max_table_cols: 10,
            max_text_lines: 20,
            max_text_chars: 2_000,
            max_list_items: 20,
            max_object_chars: 1_000,
        }
    }
}

/// Parsed and filtered Jupyter notebook.
///
/// A `Notebook` owns:
///
/// - its source path if loaded from disk,
/// - the original file size,
/// - parsed notebook cells,
/// - and compact retained outputs that can later be referenced by stable IDs.
///
/// The cell list preserves notebook order. Code cells reference retained output
/// IDs instead of embedding large output payloads inline.
#[derive(Debug, Clone)]
pub struct Notebook {
    source_path: PathBuf,
    file_size: usize,
    cells: Vec<NotebookCell>,
    retained_outputs: Vec<RetainedOutput>,
}

/// One parsed notebook cell.
///
/// This type is intentionally kept private to the module. External callers
/// access notebook content through `Notebook` methods rather than mutating or
/// depending on the raw internal representation.
#[derive(Debug, Clone)]
struct NotebookCell {
    id: usize,
    kind: NotebookCellKind,
    source: String,
    output_ids: Vec<String>,
    execution_count: Option<i64>,
}

/// Supported notebook cell kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotebookCellKind {
    Markdown,
    Code,
    Raw,
    Unknown,
}

/// A retained, filtered notebook output.
///
/// Each retained output receives a stable ID of the form `cell{n}_out{m}` so
/// later notebook AI prompts can refer to specific result snippets.
#[derive(Debug, Clone)]
pub struct RetainedOutput {
    /// Stable output ID local to the notebook.
    pub id: String,

    /// Zero-based cell index that produced this output.
    pub cell_id: usize,

    /// Reduced output kind.
    pub kind: RetainedOutputKind,

    /// Short human-readable description of what was retained.
    pub summary: String,

    /// Reduced textual content kept for downstream analysis.
    pub content: String,
}

/// Kind of reduced retained output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedOutputKind {
    Table,
    Text,
    List,
    Json,
    Object,
}

/// AI-friendly view over a notebook range.
///
/// This intentionally excludes markdown. It contains only code cells and the
/// retained outputs associated with them.
#[derive(Debug, Clone, Default)]
pub struct NotebookArea {
    /// Code cells in the requested area.
    pub code_cells: Vec<NotebookCodeBlock>,

    /// Retained outputs attached to those code cells.
    pub outputs: Vec<RetainedOutput>,
}

/// Public view of a code cell returned by [`Notebook::get_for_area`].
#[derive(Debug, Clone)]
pub struct NotebookCodeBlock {
    /// Zero-based notebook cell ID.
    pub id: usize,

    /// Execution count if present in the notebook.
    pub execution_count: Option<i64>,

    /// Source code of the cell.
    pub source: String,

    /// Stable IDs of retained outputs associated with the cell.
    pub output_ids: Vec<String>,
}

use std::fmt;

/// Truncate a string at a character boundary and append an ellipsis marker.
fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }

    let truncated: String = input.chars().take(max_chars).collect();
    format!("{truncated}\n...[truncated]")
}

/// Indent a multi-line block for readable structured rendering.
fn indent_block(input: &str, prefix: &str) -> String {
    input
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

impl fmt::Display for RetainedOutputKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RetainedOutputKind::Text => write!(f, "Text"),
            RetainedOutputKind::Table => write!(f, "Table"),
            RetainedOutputKind::Json => write!(f, "Json"),
            RetainedOutputKind::Object => write!(f, "Object"),
            RetainedOutputKind::List => write!(f, "List"),
        }
    }
}

impl fmt::Display for NotebookCodeBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Cell {}", self.id)?;

        match self.execution_count {
            Some(n) => writeln!(f, "Execution count: {n}")?,
            None => writeln!(f, "Execution count: <none>")?,
        }

        if self.output_ids.is_empty() {
            writeln!(f, "Linked outputs: <none>")?;
        } else {
            writeln!(f, "Linked outputs: {}", self.output_ids.join(", "))?;
        }

        writeln!(f, "Code:")?;
        write!(f, "{}", indent_block(&self.source, "  "))
    }
}

impl fmt::Display for RetainedOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Output {}", self.id)?;
        writeln!(f, "Cell id: {}", self.cell_id)?;
        writeln!(f, "Kind: {}", self.kind)?;

        if self.summary.trim().is_empty() {
            writeln!(f, "Summary: <none>")?;
        } else {
            writeln!(f, "Summary:")?;
            writeln!(f, "{}", indent_block(self.summary.trim(), "  "))?;
        }

        let content = self.content.trim();
        if content.is_empty() {
            write!(f, "Content: <none>")
        } else {
            writeln!(f, "Content:")?;
            write!(f, "{}", indent_block(&truncate_chars(content, 2_000), "  "))
        }
    }
}

impl fmt::Display for NotebookArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Notebook area")?;
        writeln!(f, "Code cells: {}", self.code_cells.len())?;
        writeln!(f, "Retained outputs: {}", self.outputs.len())?;

        if !self.code_cells.is_empty() {
            writeln!(f, "\nCode cells:")?;
            for (idx, cell) in self.code_cells.iter().enumerate() {
                if idx > 0 {
                    writeln!(f)?;
                    writeln!(f, "---")?;
                }
                writeln!(f, "{cell}")?;
            }
        }

        if !self.outputs.is_empty() {
            writeln!(f, "\nOutputs:")?;
            for (idx, output) in self.outputs.iter().enumerate() {
                if idx > 0 {
                    writeln!(f)?;
                    writeln!(f, "---")?;
                }
                writeln!(f, "{output}")?;
            }
        }

        Ok(())
    }
}

impl fmt::Display for NotebookParserConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Notebook parser config:")?;
        writeln!(f, "  max_text_chars: {}", self.max_text_chars)?;
        writeln!(f, "  max_text_lines: {}", self.max_text_lines)?;
        writeln!(f, "  max_table_rows: {}", self.max_table_rows)?;
        writeln!(f, "  max_table_cols: {}", self.max_table_cols)?;
        writeln!(f, "  max_list_items: {}", self.max_list_items)?;
        write!(f, "  max_object_chars: {}", self.max_object_chars)
    }
}

impl fmt::Display for Notebook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Notebook: {}", self.source_path().display())?;
        writeln!(f, "File size: {} bytes", self.file_size())?;
        writeln!(f, "Code cells: {}", self.len())?;
        writeln!(f, "Retained outputs: {}", self.retained_outputs().len())?;

        if self.is_empty() {
            return Ok(());
        }

        let area = self.get_for_area(0, self.len());

        if !area.code_cells.is_empty() {
            writeln!(f, "\nCode cells:")?;
            for (idx, cell) in area.code_cells.iter().enumerate() {
                if idx > 0 {
                    writeln!(f)?;
                    writeln!(f, "==================================================")?;
                }
                writeln!(f, "{cell}")?;
            }
        }

        if !area.outputs.is_empty() {
            writeln!(f, "\nOutputs:")?;
            for (idx, output) in area.outputs.iter().enumerate() {
                if idx > 0 {
                    writeln!(f)?;
                    writeln!(f, "==================================================")?;
                }
                writeln!(f, "{output}")?;
            }
        }

        Ok(())
    }
}

impl Notebook {
    /// Parse a notebook file into a filtered in-memory [`Notebook`].
    ///
    /// The parser performs notebook-specific sanitation during loading:
    ///
    /// - embedded images are dropped,
    /// - errors are ignored,
    /// - table-like outputs are subset,
    /// - long text outputs are clipped,
    /// - and retained outputs are stored separately with stable IDs.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read, the notebook JSON is
    /// invalid, or the notebook root lacks a valid `cells` array.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let notebook = Notebook::from_file("analysis.ipynb", NotebookParserConfig::default())?;
    /// ```
    pub fn from_file<F: AsRef<Path>>(file: F, config: NotebookParserConfig) -> Result<Self> {
        let path = file.as_ref();
        let file_size = fs::metadata(path)
            .with_context(|| format!("failed to read metadata for notebook {}", path.display()))?
            .len() as usize;

        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read notebook {}", path.display()))?;

        let json: Value = serde_json::from_str(&raw)
            .with_context(|| format!("invalid notebook json {}", path.display()))?;

        let cells_json = json
            .get("cells")
            .and_then(Value::as_array)
            .context("notebook missing cells array")?;

        let mut cells = Vec::with_capacity(cells_json.len());
        let mut retained_outputs = Vec::new();

        for (cell_id, cell_json) in cells_json.iter().enumerate() {
            let parsed = parse_cell(cell_id, cell_json, &config, &mut retained_outputs)
                .with_context(|| format!("failed to parse notebook cell {}", cell_id))?;
            cells.push(parsed);
        }

        Ok(Self {
            source_path: path.to_path_buf(),
            file_size,
            cells,
            retained_outputs,
        })
    }

    /// Iterate over markdown cells as `(id, text)` in notebook order.
    ///
    /// This is useful for later AI-assisted boundary detection where only the
    /// markdown narrative of a notebook is needed.
    pub fn iter_over_markdown(&self) -> impl Iterator<Item = (usize, &str)> {
        self.cells
            .iter()
            .filter(|cell| cell.kind == NotebookCellKind::Markdown)
            .map(|cell| (cell.id, cell.source.as_str()))
    }

    /// Return code cells and retained outputs for a notebook cell range.
    ///
    /// Markdown cells are intentionally ignored. The returned structure is meant
    /// to support AI analysis over a code-and-results region delimited by
    /// markdown boundaries determined elsewhere.
    ///
    /// The range is inclusive on both ends.
    pub fn get_for_area(&self, start_id: usize, end_id: usize) -> NotebookArea {
        let (start_id, end_id) = if start_id <= end_id {
            (start_id, end_id)
        } else {
            (end_id, start_id)
        };

        let mut area = NotebookArea::default();

        for cell in self
            .cells
            .iter()
            .filter(|cell| cell.id >= start_id && cell.id <= end_id)
        {
            if cell.kind != NotebookCellKind::Code {
                continue;
            }

            area.code_cells.push(NotebookCodeBlock {
                id: cell.id,
                execution_count: cell.execution_count,
                source: cell.source.clone(),
                output_ids: cell.output_ids.clone(),
            });

            for output_id in &cell.output_ids {
                if let Some(output) = self.retained_outputs.iter().find(|o| &o.id == output_id) {
                    area.outputs.push(output.clone());
                }
            }
        }

        area
    }

    /// Return the source path from which this notebook was parsed.
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// Return the original notebook file size in bytes.
    pub fn file_size(&self) -> usize {
        self.file_size
    }

    /// Return the total number of cells stored in the notebook.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Return whether the notebook contains no cells.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Return all retained outputs as a slice.
    pub fn retained_outputs(&self) -> &[RetainedOutput] {
        &self.retained_outputs
    }
}

/// Parse one notebook cell and append retained outputs into the notebook-wide store.
fn parse_cell(
    cell_id: usize,
    cell_json: &Value,
    config: &NotebookParserConfig,
    retained_outputs: &mut Vec<RetainedOutput>,
) -> Result<NotebookCell> {
    let kind = parse_cell_kind(cell_json.get("cell_type"));
    let source = render_ipynb_text(cell_json.get("source"));
    let execution_count = cell_json.get("execution_count").and_then(Value::as_i64);

    let output_ids = if kind == NotebookCellKind::Code {
        parse_outputs_for_cell(cell_id, cell_json.get("outputs"), config, retained_outputs)?
    } else {
        Vec::new()
    };

    Ok(NotebookCell {
        id: cell_id,
        kind,
        source,
        output_ids,
        execution_count,
    })
}

/// Convert notebook `cell_type` into the internal enum.
fn parse_cell_kind(value: Option<&Value>) -> NotebookCellKind {
    match value.and_then(Value::as_str) {
        Some("markdown") => NotebookCellKind::Markdown,
        Some("code") => NotebookCellKind::Code,
        Some("raw") => NotebookCellKind::Raw,
        Some(_) | None => NotebookCellKind::Unknown,
    }
}

/// Parse and reduce all outputs for a code cell, returning only retained output IDs.
fn parse_outputs_for_cell(
    cell_id: usize,
    outputs_value: Option<&Value>,
    config: &NotebookParserConfig,
    retained_outputs: &mut Vec<RetainedOutput>,
) -> Result<Vec<String>> {
    let Some(outputs_json) = outputs_value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };

    let mut ids = Vec::new();
    let mut next_output_index = 0usize;

    for output_json in outputs_json {
        let reduced_outputs = reduce_output(cell_id, output_json, config)
            .with_context(|| format!("failed to reduce outputs for cell {}", cell_id))?;

        for reduced in reduced_outputs {
            let id = format!("cell{}_out{}", cell_id, next_output_index);
            next_output_index += 1;

            ids.push(id.clone());
            retained_outputs.push(RetainedOutput {
                id,
                cell_id,
                kind: reduced.kind,
                summary: reduced.summary,
                content: reduced.content,
            });
        }
    }

    Ok(ids)
}

/// Private reduced output representation used while parsing.
#[derive(Debug)]
struct ReducedOutput {
    kind: RetainedOutputKind,
    summary: String,
    content: String,
}

/// Reduce one notebook output object into zero or more compact retained outputs.
///
/// This method already implements Archeo-oriented policy:
///
/// - image payloads are dropped,
/// - errors are ignored,
/// - text-like results are clipped,
/// - table-like outputs are subset,
/// - large opaque objects are compacted.
fn reduce_output(
    cell_id: usize,
    output_json: &Value,
    config: &NotebookParserConfig,
) -> Result<Vec<ReducedOutput>> {
    let output_type = output_json
        .get("output_type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    match output_type {
        "error" => Ok(Vec::new()),
        "stream" => reduce_stream_output(output_json.get("text"), config),
        "display_data" | "execute_result" => {
            reduce_display_bundle(cell_id, output_json.get("data"), config)
        }
        _ => Ok(vec![reduce_unknown_output(output_json, config)]),
    }
}

/// Reduce a stream output into compact text, list, or object-like content.
fn reduce_stream_output(
    text_value: Option<&Value>,
    config: &NotebookParserConfig,
) -> Result<Vec<ReducedOutput>> {
    let text = render_ipynb_text(text_value);
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    Ok(vec![reduce_textual_payload(&text, config, "stream output")])
}

/// Reduce a rich display bundle.
///
/// A single display bundle may yield multiple retained outputs if it contains
/// several useful representations such as a table preview and a JSON payload.
fn reduce_display_bundle(
    cell_id: usize,
    data_value: Option<&Value>,
    config: &NotebookParserConfig,
) -> Result<Vec<ReducedOutput>> {
    let Some(bundle) = data_value.and_then(Value::as_object) else {
        return Ok(Vec::new());
    };

    let mut retained = Vec::new();

    for (mime, value) in bundle {
        match mime.as_str() {
            mime if mime.starts_with("image/") => {
                // Explicitly drop heavyweight image payloads.
            }
            "text/plain" | "text/markdown" => {
                let text = render_ipynb_text(Some(value));
                if !text.trim().is_empty() {
                    retained.push(reduce_textual_payload(&text, config, mime));
                }
            }
            "text/html" => {
                let html = render_ipynb_text(Some(value));
                if !html.trim().is_empty() {
                    retained.push(reduce_html_payload(&html, config));
                }
            }
            "application/json" => {
                retained.push(reduce_json_payload(value, config, "application/json"));
            }
            mime if mime.starts_with("application/") && mime.ends_with("+json") => {
                retained.push(reduce_json_payload(value, config, mime));
            }
            "application/pdf" | "application/octet-stream" => {
                // Drop obviously heavyweight opaque payloads.
            }
            _ => {
                retained.push(reduce_unknown_mime_payload(cell_id, mime, value, config));
            }
        }
    }

    Ok(retained)
}

/// Reduce text-like payloads.
///
/// This first attempts table detection, then simple list detection, and finally
/// falls back to compact generic text.
fn reduce_textual_payload(text: &str, config: &NotebookParserConfig, label: &str) -> ReducedOutput {
    if let Some((summary, content)) =
        subset_table_like_text(text, config.max_table_rows, config.max_table_cols)
    {
        return ReducedOutput {
            kind: RetainedOutputKind::Table,
            summary: format!("subset table retained from {}", label),
            content: format!("{}\n{}", summary, content),
        };
    }

    if let Some((summary, content)) = reduce_simple_list_like_text(text, config.max_list_items) {
        return ReducedOutput {
            kind: RetainedOutputKind::List,
            summary: format!("subset list retained from {}", label),
            content: format!("{}\n{}", summary, content),
        };
    }

    let content = clip_text_lines_and_chars(text, config.max_text_lines, config.max_text_chars);
    ReducedOutput {
        kind: RetainedOutputKind::Text,
        summary: format!("clipped text retained from {}", label),
        content,
    }
}

/// Reduce HTML payloads.
///
/// HTML is intentionally treated conservatively. We do not attempt full HTML
/// parsing here; instead we strip it into a compact single string and store it
/// as an object-like payload.
fn reduce_html_payload(html: &str, config: &NotebookParserConfig) -> ReducedOutput {
    let compact = collapse_whitespace(html);
    let clipped = clip_chars(&compact, config.max_object_chars);

    ReducedOutput {
        kind: RetainedOutputKind::Object,
        summary: "clipped html/object output".to_string(),
        content: clipped,
    }
}

/// Reduce JSON payloads into compact retained content.
fn reduce_json_payload(value: &Value, config: &NotebookParserConfig, label: &str) -> ReducedOutput {
    if let Some((summary, content)) =
        subset_json_table_like_value(value, config.max_table_rows, config.max_table_cols)
    {
        return ReducedOutput {
            kind: RetainedOutputKind::Table,
            summary: format!("subset json table retained from {}", label),
            content: format!("{}\n{}", summary, content),
        };
    }

    let rendered = serde_json::to_string_pretty(value).unwrap_or_else(|_| "<invalid-json>".into());
    let clipped =
        clip_text_lines_and_chars(&rendered, config.max_text_lines, config.max_object_chars);

    ReducedOutput {
        kind: RetainedOutputKind::Json,
        summary: format!("clipped json retained from {}", label),
        content: clipped,
    }
}

/// Reduce an unknown MIME payload into a compact object-like preview.
fn reduce_unknown_mime_payload(
    cell_id: usize,
    mime: &str,
    value: &Value,
    config: &NotebookParserConfig,
) -> ReducedOutput {
    let preview = compact_json_preview(value);
    let clipped = clip_chars(&preview, config.max_object_chars);

    ReducedOutput {
        kind: RetainedOutputKind::Object,
        summary: format!(
            "unknown mime payload retained from cell {} ({})",
            cell_id, mime
        ),
        content: clipped,
    }
}

/// Reduce an unknown raw output object into a compact object-like preview.
fn reduce_unknown_output(output_json: &Value, config: &NotebookParserConfig) -> ReducedOutput {
    let preview = compact_json_preview(output_json);
    let clipped = clip_chars(&preview, config.max_object_chars);

    ReducedOutput {
        kind: RetainedOutputKind::Object,
        summary: "unknown notebook output retained as compact preview".to_string(),
        content: clipped,
    }
}

/// Render a Jupyter text field that may be stored as a string or array of strings.
fn render_ipynb_text(value: Option<&Value>) -> String {
    match value {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => {
            let mut out = String::new();
            for part in parts {
                match part {
                    Value::String(s) => out.push_str(s),
                    other => out.push_str(&compact_json_preview(other)),
                }
            }
            out
        }
        Some(other) => compact_json_preview(other),
    }
}

/// Produce a compact JSON preview suitable for unknown payloads.
fn compact_json_preview(value: &Value) -> String {
    let rendered = serde_json::to_string(value).unwrap_or_else(|_| "<unrenderable-json>".into());
    clip_chars(&rendered, 240)
}

/// Try to detect and subset a table-like plain-text payload.
///
/// Returns `(summary, content)` if the payload looks sufficiently tabular.
fn subset_table_like_text(
    text: &str,
    max_rows: usize,
    max_cols: usize,
) -> Option<(String, String)> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect();

    if lines.len() < 2 {
        return None;
    }

    let split_lines: Vec<Vec<String>> =
        lines.iter().map(|line| split_tableish_line(line)).collect();

    let tableish_count = split_lines.iter().filter(|cols| cols.len() >= 2).count();
    if tableish_count < 2 {
        return None;
    }

    let estimated_cols = split_lines.iter().map(Vec::len).max().unwrap_or(0);
    let retained_rows = split_lines.len().min(max_rows);

    let mut rendered = String::new();
    for cols in split_lines.iter().take(max_rows) {
        let row = cols
            .iter()
            .take(max_cols)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\t");
        rendered.push_str(&row);
        rendered.push('\n');
    }

    let summary = format!(
        "table preview: showing {} of {} rows and up to {} of {} columns",
        retained_rows,
        split_lines.len(),
        max_cols.min(estimated_cols),
        estimated_cols
    );

    Some((summary, rendered.trim_end().to_string()))
}

/// Split one line that looks table-like.
fn split_tableish_line(line: &str) -> Vec<String> {
    if line.contains('\t') {
        line.split('\t')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        line.split_whitespace()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// Try to reduce a simple list-like payload.
///
/// This is intentionally heuristic and conservative.
fn reduce_simple_list_like_text(text: &str, max_items: usize) -> Option<(String, String)> {
    let trimmed = text.trim();

    if !(trimmed.starts_with('[') && trimmed.ends_with(']')) {
        return None;
    }

    let inner = &trimmed[1..trimmed.len().saturating_sub(1)];
    let items: Vec<String> = inner
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if items.len() < 4 {
        return None;
    }

    let retained_items = items.iter().take(max_items).cloned().collect::<Vec<_>>();
    let summary = format!(
        "list preview: showing {} of {} items",
        retained_items.len(),
        items.len()
    );
    let content = format!("[{}]", retained_items.join(", "));

    Some((summary, content))
}

/// Try to reduce a JSON value that looks like a list of objects or a row-like table.
fn subset_json_table_like_value(
    value: &Value,
    max_rows: usize,
    max_cols: usize,
) -> Option<(String, String)> {
    let rows = value.as_array()?;
    if rows.is_empty() {
        return None;
    }

    let objects = rows
        .iter()
        .map(Value::as_object)
        .collect::<Option<Vec<_>>>()?;

    let mut all_keys = Vec::<String>::new();
    for obj in &objects {
        for key in obj.keys() {
            if !all_keys.contains(key) {
                all_keys.push(key.clone());
            }
        }
    }

    if all_keys.is_empty() {
        return None;
    }

    let retained_keys = &all_keys.iter().take(max_cols).cloned().collect::<Vec<_>>();
    let retained_rows = &objects.iter().take(max_rows).collect::<Vec<_>>();

    let mut out = String::new();
    out.push_str(&retained_keys.join("\t"));
    out.push('\n');

    for row in retained_rows {
        let values = retained_keys
            .iter()
            .map(|key| row.get(key).map(compact_cell_value).unwrap_or_default())
            .collect::<Vec<_>>();
        out.push_str(&values.join("\t"));
        out.push('\n');
    }

    let summary = format!(
        "json table preview: showing {} of {} rows and {} of {} columns",
        retained_rows.len(),
        rows.len(),
        retained_keys.len(),
        all_keys.len()
    );

    Some((summary, out.trim_end().to_string()))
}

/// Compact a single tabular cell value into one line.
fn compact_cell_value(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => clip_chars(&collapse_whitespace(s), 80),
        other => clip_chars(
            &collapse_whitespace(&serde_json::to_string(other).unwrap_or_default()),
            80,
        ),
    }
}

/// Clip text by both line count and total character count.
fn clip_text_lines_and_chars(text: &str, max_lines: usize, max_chars: usize) -> String {
    let mut out = String::new();

    for (idx, line) in text.lines().enumerate() {
        if idx >= max_lines {
            out.push_str("\n...");
            break;
        }

        if !out.is_empty() {
            out.push('\n');
        }

        out.push_str(line);

        if out.len() > max_chars {
            return clip_chars(&out, max_chars);
        }
    }

    clip_chars(&out, max_chars)
}

/// Clip a string to at most `max_chars`, appending `...` when truncated.
fn clip_chars(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    let mut clipped = text.chars().take(max_chars).collect::<String>();
    clipped.push_str("...");
    clipped
}

/// Collapse repeated whitespace into single spaces.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write_notebook(tmp: &tempfile::TempDir, name: &str, raw: &str) -> PathBuf {
        let path = tmp.path().join(name);
        fs::write(&path, raw).unwrap();
        path
    }

    #[test]
    fn from_file_parses_markdown_and_code_cells() {
        let dir = tempfile::tempdir().unwrap();
        let raw = json!({
            "cells": [
                {
                    "cell_type": "markdown",
                    "source": ["# Title\n", "Intro\n"]
                },
                {
                    "cell_type": "code",
                    "execution_count": 3,
                    "source": "x = 1\nprint(x)\n",
                    "outputs": [
                        {
                            "output_type": "stream",
                            "text": ["1", "\n"]
                        }
                    ]
                }
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "basic.ipynb", &raw);
        let notebook = Notebook::from_file(&path, NotebookParserConfig::default()).unwrap();

        assert_eq!(notebook.len(), 2);
        assert_eq!(notebook.file_size(), raw.len());
        assert_eq!(notebook.source_path(), path.as_path());

        let markdown = notebook.iter_over_markdown().collect::<Vec<_>>();
        assert_eq!(markdown, vec![(0, "# Title\nIntro\n")]);

        let area = notebook.get_for_area(0, 1);
        assert_eq!(area.code_cells.len(), 1);
        assert_eq!(area.code_cells[0].id, 1);
        assert_eq!(area.code_cells[0].execution_count, Some(3));
        assert_eq!(area.outputs.len(), 1);
        assert_eq!(area.outputs[0].kind, RetainedOutputKind::Text);
    }

    #[test]
    fn from_file_rejects_missing_cells_array() {
        let dir = tempfile::tempdir().unwrap();
        let raw = json!({"nbformat": 4}).to_string();
        let path = write_notebook(&dir, "broken.ipynb", &raw);

        let err = Notebook::from_file(&path, NotebookParserConfig::default()).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("notebook missing cells array"));
    }

    #[test]
    fn images_are_dropped_entirely() {
        let dir = tempfile::tempdir().unwrap();
        let raw = json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "plot()",
                    "outputs": [
                        {
                            "output_type": "display_data",
                            "data": {
                                "text/plain": "Figure(640x480)",
                                "image/png": "THIS_IS_A_HUGE_BASE64_IMAGE_PAYLOAD"
                            }
                        }
                    ]
                }
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "image.ipynb", &raw);
        let notebook = Notebook::from_file(&path, NotebookParserConfig::default()).unwrap();

        assert_eq!(notebook.retained_outputs().len(), 1);
        assert_eq!(
            notebook.retained_outputs()[0].kind,
            RetainedOutputKind::Text
        );
        assert!(
            !notebook.retained_outputs()[0]
                .content
                .contains("THIS_IS_A_HUGE_BASE64_IMAGE_PAYLOAD")
        );
    }

    #[test]
    fn errors_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let raw = json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "1/0",
                    "outputs": [
                        {
                            "output_type": "error",
                            "ename": "ZeroDivisionError",
                            "evalue": "division by zero",
                            "traceback": ["long traceback"]
                        }
                    ]
                }
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "errors.ipynb", &raw);
        let notebook = Notebook::from_file(&path, NotebookParserConfig::default()).unwrap();

        assert!(notebook.retained_outputs().is_empty());
        let area = notebook.get_for_area(0, 0);
        assert_eq!(area.code_cells.len(), 1);
        assert!(area.outputs.is_empty());
    }

    #[test]
    fn table_like_text_is_subset() {
        let dir = tempfile::tempdir().unwrap();
        let config = NotebookParserConfig {
            max_table_rows: 2,
            max_table_cols: 2,
            ..NotebookParserConfig::default()
        };

        let raw = json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "df",
                    "outputs": [
                        {
                            "output_type": "execute_result",
                            "data": {
                                "text/plain": "a b c\n1 2 3\n4 5 6\n7 8 9\n"
                            }
                        }
                    ]
                }
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "table.ipynb", &raw);
        let notebook = Notebook::from_file(&path, config).unwrap();

        assert_eq!(notebook.retained_outputs().len(), 1);
        let out = &notebook.retained_outputs()[0];
        assert_eq!(out.kind, RetainedOutputKind::Table);
        assert!(out.content.contains("showing 2 of 4 rows"));
        assert!(out.content.contains("a\tb"));
        assert!(out.content.contains("1\t2"));
        assert!(!out.content.contains("7\t8"));
    }

    #[test]
    fn json_row_table_is_subset() {
        let dir = tempfile::tempdir().unwrap();
        let config = NotebookParserConfig {
            max_table_rows: 2,
            max_table_cols: 2,
            ..NotebookParserConfig::default()
        };

        let raw = json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "rows",
                    "outputs": [
                        {
                            "output_type": "display_data",
                            "data": {
                                "application/json": [
                                    {"gene": "A", "score": 1.2, "extra": "x"},
                                    {"gene": "B", "score": 2.3, "extra": "y"},
                                    {"gene": "C", "score": 3.4, "extra": "z"}
                                ]
                            }
                        }
                    ]
                }
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "json_table.ipynb", &raw);
        let notebook = Notebook::from_file(&path, config).unwrap();

        let out = &notebook.retained_outputs()[0];
        assert_eq!(out.kind, RetainedOutputKind::Table);
        assert!(out.content.contains("showing 2 of 3 rows"));
    }

    #[test]
    fn list_like_text_is_reduced() {
        let dir = tempfile::tempdir().unwrap();
        let config = NotebookParserConfig {
            max_list_items: 3,
            ..NotebookParserConfig::default()
        };

        let raw = json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "vals",
                    "outputs": [
                        {
                            "output_type": "execute_result",
                            "data": {
                                "text/plain": "[1, 2, 3, 4, 5, 6]"
                            }
                        }
                    ]
                }
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "list.ipynb", &raw);
        let notebook = Notebook::from_file(&path, config).unwrap();

        let out = &notebook.retained_outputs()[0];
        assert_eq!(out.kind, RetainedOutputKind::List);
        assert!(out.content.contains("showing 3 of 6 items"));
        assert!(out.content.contains("[1, 2, 3]"));
    }

    #[test]
    fn markdown_iterator_only_returns_markdown_cells() {
        let dir = tempfile::tempdir().unwrap();
        let raw = json!({
            "cells": [
                {"cell_type": "markdown", "source": "A"},
                {"cell_type": "code", "source": "x = 1", "outputs": []},
                {"cell_type": "raw", "source": "raw"},
                {"cell_type": "markdown", "source": "B"}
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "markdowns.ipynb", &raw);
        let notebook = Notebook::from_file(&path, NotebookParserConfig::default()).unwrap();

        let markdown = notebook.iter_over_markdown().collect::<Vec<_>>();
        assert_eq!(markdown, vec![(0, "A"), (3, "B")]);
    }

    #[test]
    fn get_for_area_ignores_markdown_and_collects_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let raw = json!({
            "cells": [
                {"cell_type": "markdown", "source": "Section 1"},
                {
                    "cell_type": "code",
                    "source": "print('a')",
                    "outputs": [{"output_type": "stream", "text": "a\n"}]
                },
                {"cell_type": "markdown", "source": "Section 2"},
                {
                    "cell_type": "code",
                    "source": "print('b')",
                    "outputs": [{"output_type": "stream", "text": "b\n"}]
                }
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "area.ipynb", &raw);
        let notebook = Notebook::from_file(&path, NotebookParserConfig::default()).unwrap();

        let area = notebook.get_for_area(0, 2);
        assert_eq!(area.code_cells.len(), 1);
        assert_eq!(area.code_cells[0].id, 1);
        assert_eq!(area.outputs.len(), 1);
        assert_eq!(area.outputs[0].cell_id, 1);

        let area2 = notebook.get_for_area(0, 3);
        assert_eq!(area2.code_cells.len(), 2);
        assert_eq!(area2.outputs.len(), 2);
    }

    #[test]
    fn retained_output_ids_are_stable_per_cell() {
        let dir = tempfile::tempdir().unwrap();
        let raw = json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "x",
                    "outputs": [
                        {
                            "output_type": "display_data",
                            "data": {
                                "text/plain": "hello",
                                "application/json": {"x": 1}
                            }
                        }
                    ]
                }
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "ids.ipynb", &raw);
        let notebook = Notebook::from_file(&path, NotebookParserConfig::default()).unwrap();

        let ids = notebook
            .retained_outputs()
            .iter()
            .map(|o| o.id.clone())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["cell0_out0", "cell0_out1"]);
    }

    #[test]
    fn long_text_is_clipped() {
        let dir = tempfile::tempdir().unwrap();
        let config = NotebookParserConfig {
            max_text_lines: 2,
            max_text_chars: 12,
            ..NotebookParserConfig::default()
        };

        let raw = json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "text",
                    "outputs": [
                        {
                            "output_type": "stream",
                            "text": "line1\nline2\nline3\nline4\n"
                        }
                    ]
                }
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "clipped.ipynb", &raw);
        let notebook = Notebook::from_file(&path, config).unwrap();

        let out = &notebook.retained_outputs()[0];
        assert_eq!(out.kind, RetainedOutputKind::Text);
        assert!(out.content.len() <= 15);
    }

    #[test]
    fn html_is_retained_as_compact_object() {
        let dir = tempfile::tempdir().unwrap();
        let raw = json!({
            "cells": [
                {
                    "cell_type": "code",
                    "source": "html",
                    "outputs": [
                        {
                            "output_type": "display_data",
                            "data": {
                                "text/html": "<table>\n<tr><td>1</td></tr>\n</table>"
                            }
                        }
                    ]
                }
            ]
        })
        .to_string();

        let path = write_notebook(&dir, "html.ipynb", &raw);
        let notebook = Notebook::from_file(&path, NotebookParserConfig::default()).unwrap();

        let out = &notebook.retained_outputs()[0];
        assert_eq!(out.kind, RetainedOutputKind::Object);
        assert!(out.content.contains("<table>"));
    }
}
