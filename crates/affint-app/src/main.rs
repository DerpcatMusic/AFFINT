//! AFFINT's first native MUI editor and explicit headless verification workflows.
#![forbid(unsafe_code)]

mod app;
mod editor;
mod gpu;
mod jobs;
mod upload;

use std::{fmt::Write, path::Path, sync::Arc};

use affint_document::{History, PixelSize, RasterAsset};
use affint_render::{OutputFormat, Renderer};
use affint_storage::{FileLimits, Store};
use editor::{AppResult, Editor, Tool, document_limits};
use kurbo::Point;
use sha2::{Digest, Sha256};

/// Workers send only a bounded mailbox wake; pixel buffers never enter event queues.
#[derive(Debug, Clone, Copy)]
pub struct Wake;

fn main() {
    if let Err(error) = entry() {
        eprintln!("AFFINT: {error}");
        std::process::exit(1);
    }
}
fn entry() -> AppResult<()> {
    let mut args = std::env::args_os().skip(1);
    match args.next().as_deref().and_then(std::ffi::OsStr::to_str) {
        None => app::run(),
        Some("--export-demo") => {
            let path = args
                .next()
                .ok_or("Usage: affint-app --export-demo OUTPUT.png")?;
            export_demo(Path::new(&path))
        }
        Some("--smoke-test") => {
            let path = args
                .next()
                .ok_or("Usage: affint-app --smoke-test ARTIFACT_DIRECTORY")?;
            smoke(Path::new(&path))
        }
        Some("--gpu-proof") => {
            let path = args
                .next()
                .ok_or("Usage: affint-app --gpu-proof ARTIFACT_DIRECTORY")?;
            gpu::proof(Path::new(&path))
        }
        Some("--interaction-profile") => {
            let path = args.next().ok_or(
                "Usage: AFFINT_DIAGNOSTICS=1 affint-app --interaction-profile ARTIFACT_DIRECTORY",
            )?;
            app::run_profile(Path::new(&path))
        }
        Some("--help") => {
            println!(
                "AFFINT native MUI editor\n  affint-app\n  affint-app --export-demo OUTPUT.png\n  affint-app --smoke-test ARTIFACT_DIRECTORY\n  affint-app --gpu-proof ARTIFACT_DIRECTORY\n  AFFINT_DIAGNOSTICS=1 affint-app --interaction-profile ARTIFACT_DIRECTORY\n\nGPU proof readback is diagnostic only; ordinary canvas uses CPU reference evaluation and same-device GPU display conversion. Interaction profiling records bounded CPU wall times and render/presentation identities, then writes diagnostics after the window closes."
            );
            Ok(())
        }
        Some(_) => Err("Unknown option. Run affint-app --help.".into()),
    }
}
fn renderer() -> AppResult<Renderer> {
    Ok(Renderer::new(affint_render::Limits {
        max_bytes: 256 * 1024 * 1024,
        ..Default::default()
    })?)
}
fn render_page(editor: &Editor, renderer: &Renderer) -> AppResult<affint_render::Frame> {
    let (snapshot, request) = editor.export_request()?;
    Ok(renderer.render(
        &snapshot,
        request.with_format(OutputFormat::SrgbRgba8Straight)?,
    )?)
}
fn export_demo(path: &Path) -> AppResult<()> {
    let editor = Editor::demo()?;
    let frame = render_page(&editor, &renderer()?)?;
    report_durability(affint_storage::save_png(
        path,
        PixelSize::new(frame.width(), frame.height())?,
        frame
            .rgba8()
            .ok_or("PNG export requires straight sRGB RGBA8 output.")?,
    )?);
    println!(
        "Exported original synthetic study at {} × {} pixels: {}",
        frame.width(),
        frame.height(),
        path.display()
    );
    Ok(())
}
fn smoke(path: &Path) -> AppResult<()> {
    std::fs::create_dir_all(path)?;
    let mut editor = Editor::demo()?;
    let original = editor.session.snapshot();
    let (asset, pixels) = original
        .assets()
        .next()
        .ok_or("The synthetic document has no raster source.")?;
    let mut checksum = String::with_capacity(64);
    for byte in Sha256::digest(pixels.pixels()) {
        write!(&mut checksum, "{byte:02x}")?;
    }
    editor.exposure(0.75)?;
    editor.toggle_mask()?;
    editor.tool = Tool::Crop;
    editor.press(Point::new(570., 170.))?;
    editor.drag(Point::new(760., 360.))?;
    editor.commit()?;
    editor.begin_opacity()?;
    editor.opacity(0.72)?;
    editor.commit()?;
    editor.tool = Tool::Move;
    editor.press(Point::new(660., 260.))?;
    editor.drag(Point::new(684., 280.))?;
    editor.commit()?;
    editor.history(History::Undo)?;
    editor.history(History::Redo)?;
    editor.reorder(false)?;
    let snapshot = editor.session.snapshot();
    if snapshot
        .asset(*asset)
        .ok_or("The original source is missing.")?
        .pixels()
        != pixels.pixels()
    {
        return Err("An editor operation changed the immutable source pixels.".into());
    }
    let store = Store::new(FileLimits::default(), document_limits())?;
    let native = path.join("study.affint");
    let receipt = store.save(&snapshot, &native)?;
    report_durability(receipt.durability);
    editor.saved(receipt.revision, native.clone());
    if editor.dirty() {
        return Err("The current save receipt did not clear the dirty state.".into());
    }
    let loaded = Editor::from_loaded(store.load(&native)?, native);
    let restored = loaded.session.snapshot();
    if restored
        .asset(*asset)
        .ok_or("The reopened original source is missing.")?
        .pixels()
        != pixels.pixels()
    {
        return Err("Native reopening changed original source pixels.".into());
    }
    let renderer = renderer()?;
    let before = render_page(&editor, &renderer)?;
    let after = render_page(&loaded, &renderer)?;
    if before.rgba8().ok_or("Smoke render has no RGBA8 output.")?
        != after
            .rgba8()
            .ok_or("Reopened smoke render has no RGBA8 output.")?
    {
        return Err("Native persistence changed rendered artwork.".into());
    }
    report_durability(affint_storage::save_png(
        path.join("study.png"),
        PixelSize::new(after.width(), after.height())?,
        after
            .rgba8()
            .ok_or("PNG export requires straight sRGB RGBA8 output.")?,
    )?);
    let text = format!(
        "Original synthetic AFFINT artwork, authored through real document commands.\nSource SHA-256: {checksum}\nSource pixels preserved after editable exposure, crop, mask, opacity, move, reorder, undo/redo and native save/reopen.\nNative reopened exact render equals saved revision.\nExact PNG dimensions: {} × {}\nUndo entries retained in editor session: {} (history is intentionally session-local).\nCPU reference evaluation; this is a correctness workflow, not a performance benchmark.\n",
        after.width(),
        after.height(),
        editor.session.history_status().undo
    );
    std::fs::write(path.join("smoke.txt"), text)?;
    page_smoke(path, &renderer)?;
    println!(
        "Editor command, source-preservation, persistence and exact export smoke passed: {}",
        path.display()
    );
    Ok(())
}

fn page_smoke(path: &Path, renderer: &Renderer) -> AppResult<()> {
    let mut editor = Editor::demo()?;
    let first = editor.page()?;
    let original = editor.session.snapshot();
    let second = editor.add_page()?;
    if !editor
        .session
        .snapshot()
        .page(second)
        .ok_or("The blank page is missing.")?
        .roots()
        .is_empty()
    {
        return Err("Blank page creation copied artwork.".into());
    }
    editor.history(History::Undo)?;
    if editor.page()? != first || editor.session.snapshot().page(second).is_some() {
        return Err("Undo page creation did not restore a valid active page.".into());
    }
    editor.history(History::Redo)?;
    if editor.page()? != second {
        return Err("Redo did not restore the created page identity.".into());
    }
    let blue = Arc::new(RasterAsset::new(
        PixelSize::new(16, 16)?,
        [35, 95, 225, 255].repeat(256),
    )?);
    editor.import(second, blue, "Second-page original blue".into())?;
    editor.tool = Tool::Rectangle;
    editor.set_fill([0.13, 0.28, 0.93, 1.])?;
    editor.press(Point::new(120., 100.))?;
    editor.drag(Point::new(760., 560.))?;
    editor.commit()?;
    let (queued_snapshot, queued_request) = editor.export_request()?;
    editor.switch_page(first)?;
    if queued_request.page() != second
        || editor.session.snapshot().revision() != queued_snapshot.revision()
    {
        return Err("Page navigation changed a captured export or document revision.".into());
    }
    let store = Store::new(FileLimits::default(), document_limits())?;
    let native = path.join("multipage.affint");
    report_durability(store.save(&queued_snapshot, &native)?.durability);
    let mut reopened = Editor::from_loaded(store.load(&native)?, native);
    if reopened.session.snapshot().page_order() != queued_snapshot.page_order()
        || reopened.page()? != first
    {
        return Err(
            "Native reopening changed page identity/order or initial active-page policy.".into(),
        );
    }
    for (id, asset) in queued_snapshot.assets() {
        if reopened
            .session
            .snapshot()
            .asset(*id)
            .ok_or("A reopened source is missing.")?
            .pixels()
            != asset.pixels()
        {
            return Err("Multipage native persistence changed an original source.".into());
        }
    }
    for (id, asset) in original.assets() {
        if queued_snapshot
            .asset(*id)
            .ok_or("An original source is missing.")?
            .pixels()
            != asset.pixels()
        {
            return Err("Adding/editing another page changed the original study source.".into());
        }
    }
    reopened.switch_page(second)?;
    let captured = renderer.render(
        &queued_snapshot,
        queued_request.with_format(OutputFormat::SrgbRgba8Straight)?,
    )?;
    let restored = render_page(&reopened, renderer)?;
    if captured
        .rgba8()
        .ok_or("Captured PNG render has no RGBA8 output.")?
        != restored
            .rgba8()
            .ok_or("Restored PNG render has no RGBA8 output.")?
    {
        return Err("Captured active-page export differs after all-page native reopening.".into());
    }
    let first_frame = render_page(&editor, renderer)?;
    if first_frame
        .rgba8()
        .ok_or("First page render has no RGBA8 output.")?
        == captured
            .rgba8()
            .ok_or("Captured PNG render has no RGBA8 output.")?
    {
        return Err("The two smoke pages unexpectedly have identical artwork.".into());
    }
    report_durability(affint_storage::save_png(
        path.join("multipage-active.png"),
        PixelSize::new(captured.width(), captured.height())?,
        captured
            .rgba8()
            .ok_or("PNG export requires straight sRGB RGBA8 output.")?,
    )?);
    std::fs::write(
        path.join("multipage.txt"),
        format!(
            "Two distinct editable pages preserved in native storage.\nBlank page creation is one atomic undo; redo restores its PageId.\nNavigation does not mutate revision/history or retarget captured PNG export.\nAll original sources preserved; reopened active-page render equals captured export.\nActive exported page: {}. Output: {} × {} pixels.\n",
            second.get(),
            captured.width(),
            captured.height()
        ),
    )?;
    Ok(())
}
pub fn report_durability(durability: affint_storage::Durability) {
    match durability {
        affint_storage::Durability::Synced => {}
        affint_storage::Durability::PublishedUnconfirmed { directory, source } => eprintln!(
            "Published output, but directory durability is unconfirmed for {}: {source}",
            directory.display()
        ),
        affint_storage::Durability::PlatformUnverified => {
            eprintln!("Published output; directory durability is not verified on this platform.")
        }
    }
}
