//! Interactive full-screen browsers. Each view is built from the same
//! [`crate::ui`] model that produces static reports, so a command describes its
//! data once and either prints it or browses it depending on the terminal.

mod app;
mod datasheet;
mod dds;
mod sheets;
mod table;
mod tree;

use std::io;

use std::sync::Arc;

use app::View;
use datasheet::DatasheetView;
pub use dds::{AlphaSurface, DdsCatalog, DdsItem, PakIndex, SharedIndex, TextureStore, shared_index};
use dds::DdsBrowser;
use nw_jobs::JobRunner;
use ratatui_image::picker::Picker;
pub use datasheet::{
    GridCell, GridColumn, GridType, IndexProgress, Loc, LocaleState, SheetData, SheetSource,
};
use sheets::SheetPicker;
pub use table::RowFeed;
use table::TableView;
pub use tree::TreeNode;
use tree::TreeView;

use crate::ui::Table;
use crate::ui::theme;

/// Launch the generic table browser over `table`, returning when the user
/// quits. `primary_col` is the column whose value is printed to stdout when the
/// user selects a row with Enter.
pub fn browse(
    title: impl Into<String>,
    stats: Vec<(String, String)>,
    table: Table,
    primary_col: usize,
) -> io::Result<()> {
    let mut view = TableView::new(title, stats, table, primary_col, theme::caps());
    app::run(&mut view)?;
    if let Some(line) = view.take_result() {
        println!("{line}");
    }
    Ok(())
}

/// Like [`browse`], but the rows stream in from `feed` (filled by a background
/// scan) so the browser opens instantly and never blocks on the full scan.
/// `table` is an empty template carrying the headers and column alignment.
pub fn browse_streaming(
    title: impl Into<String>,
    stats: Vec<(String, String)>,
    table: Table,
    primary_col: usize,
    feed: Arc<RowFeed>,
) -> io::Result<()> {
    let mut view = TableView::streaming(title, stats, table, primary_col, feed, theme::caps());
    app::run(&mut view)?;
    if let Some(line) = view.take_result() {
        println!("{line}");
    }
    Ok(())
}

/// Like [`browse`], but returns the selected primary-column value (if the user
/// pressed Enter) instead of printing it — used to drive file pickers.
pub fn pick(
    title: impl Into<String>,
    stats: Vec<(String, String)>,
    table: Table,
    primary_col: usize,
) -> io::Result<Option<String>> {
    let mut view = TableView::new(title, stats, table, primary_col, theme::caps());
    app::run(&mut view)?;
    Ok(view.take_result())
}

/// Browse a cross-sheet datasheet workspace. A streaming [`SheetPicker`] fills in
/// live while the source discovers datasheets in the background; selecting one
/// opens the grid. Picker and grid share one alternate-screen session, so moving
/// between them repaints in place with no flicker. When discovery has already
/// finished with exactly one sheet, the picker is skipped. `locale_mode` is
/// 0=key, 1=text, 2=both.
pub fn datasheet_browser(source: Arc<dyn SheetSource>, locale_mode: u8) -> io::Result<()> {
    if source.discovery().done && source.sheets().len() == 1 {
        let mut view = DatasheetView::new(source, 0, locale_mode, theme::caps());
        return app::run(&mut view);
    }

    let mut session = app::session()?;
    loop {
        let mut picker = SheetPicker::new(source.clone(), theme::caps());
        session.run(&mut picker)?;
        match picker.picked() {
            Some(id) => {
                let mut grid = DatasheetView::new(source.clone(), id, locale_mode, theme::caps());
                session.run(&mut grid)?;
            }
            None => return Ok(()),
        }
    }
}

/// Launch the DDS texture browser: a fuzzy-filterable list with a live image
/// preview. `runner` drives the background decode fan-out. Prints the selected
/// texture's path to stdout if the user presses Enter.
pub fn dds_browser(
    catalog: Arc<DdsCatalog>,
    store: Arc<TextureStore>,
    source: String,
    runner: JobRunner,
) -> io::Result<()> {
    let mut session = app::session()?;
    // The graphics-protocol query must run after the alternate screen is up but
    // before the event loop reads keys; fall back to half-blocks if unsupported.
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    let mut view = DdsBrowser::new(catalog, store, source, runner, picker, theme::caps());
    session.run(&mut view)?;
    drop(session);
    if let Some(line) = view.take_result() {
        println!("{line}");
    }
    Ok(())
}

/// Open the collapsible ObjectStream DOM tree viewer.
pub fn tree(title: impl Into<String>, nodes: Vec<TreeNode>) -> io::Result<()> {
    let mut view = TreeView::new(title, nodes, theme::caps());
    app::run(&mut view)
}

/// Whether interactive full-screen views should be used.
pub fn interactive() -> bool {
    theme::caps().interactive
}
