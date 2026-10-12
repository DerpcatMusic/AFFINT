//! Reproducible procedural CPU/document workloads; run only through the build coordinator.
//!
//! CSV timings cover rendering or one public transaction, never both together.
//! Fixture construction, admission probes, output inspection, and Frame drop are
//! outside measured samples. Deadline polling is inside measured render calls.
//! CSV schema 2 records `output_format`; `--output-format rgba8|linear-f32` selects
//! one owned representation without conversion. RGBA8 remains the default; edit
//! rows leave this field empty because they render no output.
//! All artwork/source bytes are original procedural fixtures. No quality reduction,
//! worker pool, cache, private production access, or new dependency is introduced.

use std::cell::Cell;
use std::error::Error;
use std::hint::black_box;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use affint_document::{
    Change, Color, Content, Edit, LayerSpec, Limits as DocumentLimits, LiveOperation, Mask,
    MaskMode, NodeId, PageSize, Parent, PixelSize, RasterAsset, Session, Transform, VectorContent,
};
use affint_render::{Frame, Limits, OutputFormat, RenderRequest, Renderer, View};
use kurbo::{Circle, Rect, Shape};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const FANOUT: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Workload {
    Photo24,
    Photo48,
    Vectors,
    Offscreen,
    Edits(usize),
}

impl Workload {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "photo24" => Ok(Self::Photo24),
            "photo48" => Ok(Self::Photo48),
            "vectors" => Ok(Self::Vectors),
            "offscreen" => Ok(Self::Offscreen),
            "edit100" => Ok(Self::Edits(100)),
            "edit1000" => Ok(Self::Edits(1000)),
            "edit10000" => Ok(Self::Edits(10_000)),
            _ => Err(problem("unknown workload; use --help for supported names")),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Photo24 => "photo24",
            Self::Photo48 => "photo48",
            Self::Vectors => "vectors",
            Self::Offscreen => "offscreen",
            Self::Edits(100) => "edit100",
            Self::Edits(1000) => "edit1000",
            Self::Edits(_) => "edit10000",
        }
    }
}

struct Options {
    samples: usize,
    warmups: usize,
    max_seconds: u64,
    vectors: usize,
    tile_size: u32,
    render_bytes: usize,
    render_work: u64,
    output_format: OutputFormat,
    workloads: Vec<Workload>,
}

impl Options {
    fn parse() -> Result<Option<Self>> {
        let mut options = Self {
            samples: 7,
            warmups: 1,
            max_seconds: 120,
            vectors: 64,
            tile_size: 256,
            render_bytes: 512 * 1024 * 1024,
            render_work: 20_000_000_000,
            output_format: OutputFormat::SrgbRgba8Straight,
            workloads: Vec::new(),
        };
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--help" || flag == "-h" {
                println!(
                    "workloads [--workload NAME]... [--samples 1..64] [--warmups 0..8] [--max-seconds 1..3600] [--vector-layers 1..256] [--tile-size 1..2048] [--render-mib 1..1024] [--render-work 1..1000000000000] [--output-format rgba8|linear-f32]\nNames: photo24 photo48 vectors offscreen edit100 edit1000 edit10000\nDefault: photo24 vectors offscreen edit100 edit1000 edit10000; seven samples, one warmup, 120 seconds per phase, RGBA8 output. linear-f32 retains HDR working color at 16 bytes per pixel; choose sufficient finite --render-mib admission (e.g.1024 for photo48). photo48 explicitly raises only the renderer output-pixel admission to 48MP. CSV is written to stdout; errors go to stderr."
                );
                return Ok(None);
            }
            let value = args
                .next()
                .ok_or_else(|| problem("an option needs a value"))?;
            match flag.as_str() {
                "--workload" => {
                    let workload = Workload::parse(&value)?;
                    if options.workloads.contains(&workload) {
                        return Err(problem("duplicate workload"));
                    }
                    options.workloads.push(workload);
                }
                "--samples" => options.samples = bounded(&value, 1, 64)? as usize,
                "--warmups" => options.warmups = bounded(&value, 0, 8)? as usize,
                "--max-seconds" => options.max_seconds = bounded(&value, 1, 3600)?,
                "--vector-layers" => options.vectors = bounded(&value, 1, 256)? as usize,
                "--tile-size" => options.tile_size = bounded(&value, 1, 2048)? as u32,
                "--render-mib" => {
                    options.render_bytes = (bounded(&value, 1, 1024)? as usize)
                        .checked_mul(1024 * 1024)
                        .ok_or_else(|| problem("render byte limit overflow"))?
                }
                "--render-work" => options.render_work = bounded(&value, 1, 1_000_000_000_000)?,
                "--output-format" => {
                    options.output_format = match value.as_str() {
                        "rgba8" => OutputFormat::SrgbRgba8Straight,
                        "linear-f32" => OutputFormat::LinearSrgbRgbaF32Premultiplied,
                        _ => return Err(problem("output format must be rgba8 or linear-f32")),
                    }
                }
                _ => return Err(problem("unknown option; use --help")),
            }
        }
        if options.workloads.is_empty() {
            options.workloads = vec![
                Workload::Photo24,
                Workload::Vectors,
                Workload::Offscreen,
                Workload::Edits(100),
                Workload::Edits(1000),
                Workload::Edits(10_000),
            ];
        }
        Ok(Some(options))
    }
}

fn bounded(value: &str, minimum: u64, maximum: u64) -> Result<u64> {
    let parsed = value.parse::<u64>()?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(problem("option outside its finite range"));
    }
    Ok(parsed)
}
fn problem(message: &str) -> Box<dyn Error> {
    io::Error::new(io::ErrorKind::InvalidInput, message).into()
}
fn reserved<T>(count: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values.try_reserve_exact(count)?;
    Ok(values)
}
fn session(width: u32, height: u32) -> Result<Session> {
    Ok(Session::new(
        PageSize::new(f64::from(width), f64::from(height))?,
        DocumentLimits {
            max_history_entries: 0,
            max_history_metadata_bytes: 0,
            ..DocumentLimits::default()
        },
    )?)
}
fn owner(session: &Session) -> Parent {
    Parent::Page(session.snapshot().page_order()[0])
}
fn group(session: &mut Session, parent: Parent, index: usize, name: &str) -> Result<NodeId> {
    let created = session.transact(Edit::new(
        "Fixture group",
        [Change::InsertLayer {
            parent,
            index,
            layer: LayerSpec::new(name, Content::Group),
        }],
    ))?;
    created
        .nodes
        .first()
        .copied()
        .ok_or_else(|| problem("group identity was not returned"))
}
fn properties(
    session: &mut Session,
    node: NodeId,
    update: impl FnOnce(&mut LayerSpec),
) -> Result<()> {
    let snapshot = session.snapshot();
    let layer = snapshot
        .node(node)
        .ok_or_else(|| problem("fixture layer missing"))?;
    let mut spec = LayerSpec::new(layer.name(), layer.content().clone());
    spec.transform = layer.transform();
    spec.opacity = layer.opacity();
    spec.visible = layer.visible();
    spec.mask = layer.mask();
    spec.operations = layer.operations().to_vec();
    update(&mut spec);
    session.transact(Edit::new(
        "Fixture properties",
        [Change::SetLayer { node, layer: spec }],
    ))?;
    Ok(())
}
fn procedural_source(width: u32, height: u32, mask: bool) -> Result<Arc<RasterAsset>> {
    let size = PixelSize::new(width, height)?;
    let mut bytes = reserved(size.byte_len()?)?;
    for y in 0..height {
        for x in 0..width {
            let rgba = if mask {
                let r = ((u64::from(x) * 255) / u64::from(width)) as u8;
                let g = ((u64::from(y) * 255) / u64::from(height)) as u8;
                [
                    r,
                    g,
                    ((u16::from(r) + u16::from(g)) / 2) as u8,
                    128 + ((x ^ y) & 127) as u8,
                ]
            } else {
                [
                    x.wrapping_mul(17).wrapping_add(y.wrapping_mul(29)) as u8,
                    x.wrapping_mul(3).wrapping_add(y.wrapping_mul(7)) as u8,
                    x.wrapping_mul(11).wrapping_add(y.wrapping_mul(5)) as u8,
                    if (x + y) % 97 == 0 { 96 } else { 255 },
                ]
            };
            bytes.extend_from_slice(&rgba);
        }
    }
    Ok(Arc::new(RasterAsset::new(size, bytes)?))
}
fn import(
    session: &mut Session,
    parent: Parent,
    index: usize,
    width: u32,
    height: u32,
    mask: bool,
) -> Result<NodeId> {
    let asset = procedural_source(width, height, mask)?;
    let created = session.transact(Edit::new(
        "Fixture original source",
        [Change::InsertRaster {
            parent,
            index,
            name: if mask {
                "Original coverage"
            } else {
                "Original photo"
            }
            .into(),
            asset,
            transform: Transform::IDENTITY,
        }],
    ))?;
    created
        .nodes
        .first()
        .copied()
        .ok_or_else(|| problem("raster identity was not returned"))
}

fn photo(width: u32, height: u32) -> Result<Session> {
    let mut session = session(width, height)?;
    let page = owner(&session);
    let outer = group(&mut session, page, 0, "Placed composite")?;
    let inner = group(
        &mut session,
        Parent::Layer(outer),
        0,
        "Editable light and mask",
    )?;
    let image = import(&mut session, Parent::Layer(inner), 0, width, height, false)?;
    let mask = import(&mut session, page, 1, width, height, true)?;
    let w = f64::from(width);
    let h = f64::from(height);
    properties(&mut session, image, |spec| {
        spec.operations = vec![
            LiveOperation::Exposure(0.75),
            LiveOperation::Crop(Rect::new(40.25, 32.5, w - 48.75, h - 40.25)),
        ]
    })?;
    properties(&mut session, mask, |spec| {
        spec.visible = false;
        spec.opacity = 0.85;
        spec.operations = vec![
            LiveOperation::Exposure(-0.25),
            LiveOperation::Crop(Rect::new(16.25, 24.75, w - 16.5, h - 24.25)),
        ];
    })?;
    properties(&mut session, inner, |spec| {
        spec.mask = Some(Mask {
            source: mask,
            mode: MaskMode::Luminance,
            inverted: false,
        });
        spec.operations = vec![LiveOperation::Exposure(-0.25)];
    })?;
    properties(&mut session, outer, |spec| spec.opacity = 0.93)?;
    session.transact(Edit::new(
        "Fixture affine placement",
        [Change::SetTransform {
            node: outer,
            transform: Transform::new([0.998, 0.003, -0.004, 1.001, 6.5, 4.25])?,
        }],
    ))?;
    Ok(session)
}

fn vectors(count: usize, offscreen: bool) -> Result<Session> {
    let mut session = session(512, 512)?;
    let page = owner(&session);
    let source = Content::Vector(VectorContent::new(
        Circle::new((0.0, 0.0), 14.0).to_path(0.001),
        Color::new(0.15, 0.39, 0.8, 0.8)?,
    )?);
    let mut columns = 1;
    while columns * columns < count {
        columns += 1;
    }
    let step = 512.0 / columns as f64;
    let mut changes = reserved(count)?;
    for index in 0..count {
        let mut layer = LayerSpec::new(format!("Procedural curve {index}"), source.clone());
        let x = (index % columns) as f64 * step + step / 2.0 + if offscreen { 1024.0 } else { 0.0 };
        let y = (index / columns) as f64 * step + step / 2.0;
        layer.transform = Transform::new([1.0, 0.0, 0.0, 1.0, x + 0.25, y + 0.125])?;
        changes.push(Change::InsertLayer {
            parent: page,
            index,
            layer,
        });
    }
    session.transact(Edit::new("Fixture curve batch", changes))?;
    Ok(session)
}

struct EditFixture {
    session: Session,
    nodes: Vec<NodeId>,
    first_owner: NodeId,
    first_siblings: usize,
}
fn edit_fixture(leaves: usize) -> Result<EditFixture> {
    let mut session = session(1024, 1024)?;
    let page = owner(&session);
    let source = procedural_source(4, 4, false)?;
    let created = session.transact(Edit::new(
        "Fixture shared source",
        [Change::AddAsset { asset: source }],
    ))?;
    let asset = *created
        .assets
        .first()
        .ok_or_else(|| problem("asset identity was not returned"))?;
    let containers = leaves.div_ceil(FANOUT);
    let roots = containers.div_ceil(FANOUT);
    let root_changes = (0..roots).map(|index| Change::InsertLayer {
        parent: page,
        index,
        layer: LayerSpec::new(format!("Branch {index}"), Content::Group),
    });
    let root_ids = session
        .transact(Edit::new("Fixture branches", root_changes))?
        .nodes;
    let mut container_changes = reserved(containers)?;
    for index in 0..containers {
        let parent = *root_ids
            .get(index / FANOUT)
            .ok_or_else(|| problem("branch missing"))?;
        container_changes.push(Change::InsertLayer {
            parent: Parent::Layer(parent),
            index: index % FANOUT,
            layer: LayerSpec::new(format!("Container {index}"), Content::Group),
        });
    }
    let owners = session
        .transact(Edit::new("Fixture containers", container_changes))?
        .nodes;
    let mut leaf_changes = reserved(leaves)?;
    for index in 0..leaves {
        let parent = *owners
            .get(index / FANOUT)
            .ok_or_else(|| problem("container missing"))?;
        let mut layer = LayerSpec::new(format!("Shared raster {index}"), Content::Raster { asset });
        layer.transform = Transform::new([
            1.0,
            0.0,
            0.0,
            1.0,
            (index % 100) as f64 * 10.0,
            (index / 100) as f64 * 10.0,
        ])?;
        leaf_changes.push(Change::InsertLayer {
            parent: Parent::Layer(parent),
            index: index % FANOUT,
            layer,
        });
    }
    let nodes = session
        .transact(Edit::new("Fixture leaves in one batch", leaf_changes))?
        .nodes;
    let first_owner = *owners
        .first()
        .ok_or_else(|| problem("first container missing"))?;
    Ok(EditFixture {
        session,
        nodes,
        first_owner,
        first_siblings: leaves.min(FANOUT),
    })
}

struct Stats {
    values: Vec<u128>,
    warmups: usize,
    status: &'static str,
    failure: Option<String>,
}
fn measure<P, T>(
    options: &Options,
    mut prepare: impl FnMut(usize) -> Result<P>,
    mut execute: impl FnMut(P, Instant) -> Result<T>,
) -> Result<Stats> {
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(options.max_seconds))
        .ok_or_else(|| problem("deadline overflow"))?;
    let mut stats = Stats {
        values: reserved(options.samples)?,
        warmups: 0,
        status: "ok",
        failure: None,
    };
    for iteration in 0..options.warmups + options.samples {
        if Instant::now() >= deadline {
            stats.status = "deadline";
            break;
        }
        let prepared = prepare(iteration)?;
        let start = Instant::now();
        match execute(prepared, deadline) {
            Ok(value) => {
                let elapsed = start.elapsed().as_nanos();
                black_box(&value);
                if iteration < options.warmups {
                    stats.warmups += 1;
                } else {
                    stats.values.push(elapsed);
                }
                drop(value);
            }
            Err(error) => {
                stats.status = if error.downcast_ref::<affint_render::Error>()
                    == Some(&affint_render::Error::Canceled)
                {
                    "deadline"
                } else {
                    "error"
                };
                stats.failure = Some(error.to_string());
                break;
            }
        }
    }
    stats.values.sort_unstable();
    Ok(stats)
}
fn quantile(values: &[u128], percentile: usize) -> String {
    if values.is_empty() {
        return String::new();
    }
    // Nearest rank, intentionally without interpolation or a modeled distribution.
    let index = (values.len() * percentile).div_ceil(100) - 1;
    values
        .get(index)
        .map(ToString::to_string)
        .unwrap_or_default()
}

#[derive(Default)]
struct Admission {
    probe_ns: u128,
    output: usize,
    peak: usize,
    frame: usize,
    released: usize,
}
fn probe(
    renderer: &Renderer,
    snapshot: &affint_document::Snapshot,
    request: RenderRequest,
    options: &Options,
) -> Result<(Admission, Option<Box<dyn Error>>)> {
    let peak = Cell::new(0usize);
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(options.max_seconds))
        .ok_or_else(|| problem("probe deadline overflow"))?;
    let start = Instant::now();
    let result = renderer.render_with_cancel(snapshot, request, || {
        peak.set(peak.get().max(renderer.allocated_bytes()));
        Instant::now() >= deadline
    });
    let probe_ns = start.elapsed().as_nanos();
    let frame = match result {
        Ok(frame) => frame,
        Err(error) => {
            let admission = Admission {
                probe_ns,
                peak: peak.get(),
                released: renderer.allocated_bytes(),
                ..Admission::default()
            };
            return Ok((admission, Some(error.into())));
        }
    };
    black_box(frame.pixels());
    let output = frame.byte_len();
    let held = renderer.allocated_bytes();
    drop(frame);
    let released = renderer.allocated_bytes();
    if released != 0 {
        return Err(problem("renderer retained admission after frame drop"));
    }
    Ok((
        Admission {
            probe_ns,
            output,
            peak: peak.get(),
            frame: held,
            released,
        },
        None,
    ))
}

struct Row<'a> {
    phase: &'static str,
    workload: Workload,
    session: &'a Session,
    options: &'a Options,
    stats: &'a Stats,
    admission: &'a Admission,
    setup_ns: u128,
    width: u32,
    height: u32,
    leaves: usize,
    fanout: usize,
    max_pixels: u64,
}
fn write_row(output: &mut impl Write, row: Row<'_>) -> Result<bool> {
    let snapshot = row.session.snapshot();
    let usage = row.session.resource_usage()?;
    let values = &row.stats.values;
    let minimum = values.first().map(ToString::to_string).unwrap_or_default();
    let maximum = values.last().map(ToString::to_string).unwrap_or_default();
    writeln!(
        output,
        "2,{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        row.phase,
        row.workload.name(),
        row.width,
        row.height,
        if row.phase == "render" {
            match row.options.output_format {
                OutputFormat::SrgbRgba8Straight => "rgba8",
                OutputFormat::LinearSrgbRgbaF32Premultiplied => "linear-f32",
            }
        } else {
            ""
        },
        snapshot.layers().count(),
        snapshot.assets().count(),
        usage.pixel_bytes,
        usage.current_metadata_bytes,
        row.session.history_status().undo,
        row.leaves,
        row.fanout,
        row.options.warmups,
        row.stats.warmups,
        row.options.samples,
        values.len(),
        quantile(values, 50),
        quantile(values, 95),
        minimum,
        maximum,
        row.setup_ns,
        row.admission.probe_ns,
        row.admission.output,
        row.admission.peak,
        row.admission.frame,
        row.admission.released,
        row.options.max_seconds,
        row.options.tile_size,
        row.options.render_bytes,
        row.options.render_work,
        row.max_pixels,
        row.stats.status
    )?;
    output.flush()?;
    if let Some(failure) = &row.stats.failure {
        eprintln!("{} {}: {failure}", row.workload.name(), row.phase);
    }
    Ok(row.stats.status == "ok")
}

fn render_workload(output: &mut impl Write, workload: Workload, options: &Options) -> Result<bool> {
    let start = Instant::now();
    let (session, width, height) = match workload {
        Workload::Photo24 => (photo(6000, 4000)?, 6000, 4000),
        Workload::Photo48 => (photo(8000, 6000)?, 8000, 6000),
        Workload::Vectors => (vectors(options.vectors, false)?, 512, 512),
        Workload::Offscreen => (vectors(options.vectors, true)?, 512, 512),
        Workload::Edits(_) => return Err(problem("edit workload passed to renderer")),
    };
    let setup_ns = start.elapsed().as_nanos();
    let snapshot = session.snapshot();
    let max_pixels = if workload == Workload::Photo48 {
        48_000_000
    } else {
        32_000_000
    };
    let renderer = Renderer::new(Limits {
        tile_size: options.tile_size,
        max_bytes: options.render_bytes,
        max_work: options.render_work,
        max_pixels,
        ..Limits::default()
    })?;
    let request = RenderRequest::new(
        snapshot.page_order()[0],
        width,
        height,
        View::Region(Rect::new(0.0, 0.0, f64::from(width), f64::from(height))),
    )?
    .with_format(options.output_format)?;
    let (admission, failure) = probe(&renderer, &snapshot, request, options)?;
    if let Some(error) = failure {
        let stats = Stats {
            values: Vec::new(),
            warmups: 0,
            status: "probe_error",
            failure: Some(error.to_string()),
        };
        return write_row(
            output,
            Row {
                phase: "render",
                workload,
                session: &session,
                options,
                stats: &stats,
                admission: &admission,
                setup_ns,
                width,
                height,
                leaves: 0,
                fanout: 0,
                max_pixels,
            },
        );
    }
    let stats = measure(
        options,
        |_| Ok(request),
        |request, deadline| -> Result<Frame> {
            Ok(renderer.render_with_cancel(&snapshot, request, || Instant::now() >= deadline)?)
        },
    )?;
    write_row(
        output,
        Row {
            phase: "render",
            workload,
            session: &session,
            options,
            stats: &stats,
            admission: &admission,
            setup_ns,
            width,
            height,
            leaves: 0,
            fanout: 0,
            max_pixels,
        },
    )
}

fn edit_workload(
    output: &mut impl Write,
    workload: Workload,
    leaves: usize,
    options: &Options,
) -> Result<bool> {
    let start = Instant::now();
    let mut fixture = edit_fixture(leaves)?;
    let setup_ns = start.elapsed().as_nanos();
    let selected = *fixture
        .nodes
        .get(leaves / 2)
        .ok_or_else(|| problem("transform target missing"))?;
    let snapshot = fixture.session.snapshot();
    let base = snapshot
        .node(selected)
        .ok_or_else(|| problem("transform target missing"))?
        .transform()
        .affine()
        .as_coeffs();
    drop(snapshot);
    let transforms = [
        Transform::new([base[0], base[1], base[2], base[3], base[4] + 0.25, base[5]])?,
        Transform::new([base[0], base[1], base[2], base[3], base[4] - 0.25, base[5]])?,
    ];
    let transform_stats = measure(
        options,
        |iteration| {
            Ok(Edit::new(
                "Place",
                [Change::SetTransform {
                    node: selected,
                    transform: transforms[iteration % 2],
                }],
            ))
        },
        |edit, _| Ok(fixture.session.transact(edit)?),
    )?;
    let admission = Admission::default();
    let transform_ok = write_row(
        output,
        Row {
            phase: "transact_transform",
            workload,
            session: &fixture.session,
            options,
            stats: &transform_stats,
            admission: &admission,
            setup_ns,
            width: 0,
            height: 0,
            leaves,
            fanout: FANOUT,
            max_pixels: 0,
        },
    )?;
    let selected = *fixture
        .nodes
        .first()
        .ok_or_else(|| problem("reorder target missing"))?;
    let parent = Parent::Layer(fixture.first_owner);
    let end = fixture.first_siblings - 1;
    let reorder_stats = measure(
        options,
        |iteration| {
            Ok(Edit::new(
                "Order",
                [Change::MoveLayer {
                    node: selected,
                    parent,
                    index: if iteration % 2 == 0 { end } else { 0 },
                }],
            ))
        },
        |edit, _| Ok(fixture.session.transact(edit)?),
    )?;
    let reorder_ok = write_row(
        output,
        Row {
            phase: "transact_reorder",
            workload,
            session: &fixture.session,
            options,
            stats: &reorder_stats,
            admission: &admission,
            setup_ns,
            width: 0,
            height: 0,
            leaves,
            fanout: FANOUT,
            max_pixels: 0,
        },
    )?;
    Ok(transform_ok && reorder_ok)
}

fn run() -> Result<bool> {
    let Some(options) = Options::parse()? else {
        return Ok(true);
    };
    eprintln!(
        "profile={} package={} history_limit=0 quantiles=nearest_rank deadline_polling=included_in_render timing_unit=nanoseconds",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        env!("CARGO_PKG_VERSION")
    );
    let mut output = io::BufWriter::new(io::stdout().lock());
    writeln!(
        output,
        "schema_version,phase,workload,output_width,output_height,output_format,layers,assets,source_admitted_bytes,current_metadata_admitted_bytes,undo_entries,fixture_leaves,fixture_fanout,warmups_requested,warmups_completed,samples_requested,samples_completed,p50_ns,p95_ns,min_ns,max_ns,setup_ns,first_admission_probe_ns,frame_output_bytes,render_peak_admitted_bytes,render_frame_admitted_bytes,render_released_admitted_bytes,phase_max_seconds,color_tile_size,render_limit_bytes,render_limit_work,render_limit_pixels,status"
    )?;
    output.flush()?;
    let mut success = true;
    for &workload in &options.workloads {
        eprintln!("starting {}", workload.name());
        let complete = match workload {
            Workload::Edits(leaves) => edit_workload(&mut output, workload, leaves, &options),
            _ => render_workload(&mut output, workload, &options),
        }?;
        success &= complete;
    }
    Ok(success)
}
fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(error) => {
            eprintln!("workloads: {error}");
            ExitCode::FAILURE
        }
    }
}
