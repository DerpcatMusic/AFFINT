//! Bounded retained renderer workloads, executed only by the build coordinator.
//!
//! Timings cover the retained render call. Fixture construction, transactions,
//! independent fresh-uncached pixel oracles and owner drops are outside samples.
//! This is source instrumentation, not a measured speedup or native upload test.

use std::error::Error;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use affint_document::{
    Change, Color, Content, Edit, LayerSpec, Limits as DocumentLimits, NodeId, PageSize, Parent,
    PixelSize, RasterAsset, Session, Transform, VectorContent,
};
use affint_render::{Frame, FramePixels, Limits, OutputFormat, RenderRequest, Renderer, View};
use kurbo::{Circle, Shape};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const MIB: usize = 1024 * 1024;

struct Options {
    samples: usize,
    seconds: u64,
    format: OutputFormat,
    photo: bool,
}

impl Options {
    fn parse() -> Result<Option<Self>> {
        let mut options = Self {
            samples: 7,
            seconds: 120,
            format: OutputFormat::LinearSrgbRgbaF32Premultiplied,
            photo: false,
        };
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--help" {
                println!(
                    "retained_workloads [--workload curves64|photo24-pan] [--samples 1..31] [--max-seconds 1..600] [--output-format rgba8|linear-f32]\nShared renderer256MiB/cache128MiB. Exact curves512x512; original24MP6000x4000 and exact viewport1024x768. Setup, edits, fresh uncached oracle, and Frame drop excluded from render samples. One initial exact warmup. Photo camera visits13 bounded integer256-pixel positions, then repeats. No uploads/native timings. CSV stdout; errors stderr."
                );
                return Ok(None);
            }
            let value = args
                .next()
                .ok_or_else(|| problem("option requires a value"))?;
            match flag.as_str() {
                "--workload" => {
                    options.photo = match value.as_str() {
                        "curves64" => false,
                        "photo24-pan" => true,
                        _ => return Err(problem("unknown retained workload")),
                    }
                }
                "--samples" => options.samples = bounded(&value, 1, 31)? as usize,
                "--max-seconds" => options.seconds = bounded(&value, 1, 600)?,
                "--output-format" => {
                    options.format = match value.as_str() {
                        "rgba8" => OutputFormat::SrgbRgba8Straight,
                        "linear-f32" => OutputFormat::LinearSrgbRgbaF32Premultiplied,
                        _ => return Err(problem("unknown output format")),
                    }
                }
                _ => return Err(problem("unknown retained option")),
            }
        }
        Ok(Some(options))
    }
}

fn problem(message: &str) -> Box<dyn Error> {
    io::Error::new(io::ErrorKind::InvalidInput, message).into()
}

fn bounded(value: &str, min: u64, max: u64) -> Result<u64> {
    let value = value.parse()?;
    if !(min..=max).contains(&value) {
        return Err(problem("option out of range"));
    }
    Ok(value)
}

fn session(width: f64, height: f64) -> Result<Session> {
    Ok(Session::new(
        PageSize::new(width, height)?,
        DocumentLimits {
            max_history_entries: 0,
            max_history_metadata_bytes: 0,
            ..DocumentLimits::default()
        },
    )?)
}

fn curves() -> Result<(Session, Option<NodeId>)> {
    let mut session = session(512.0, 512.0)?;
    let parent = Parent::Page(session.snapshot().page_order()[0]);
    let mut target = None;
    for index in 0..64 {
        let x = 30.0 + (index % 8) as f64 * 62.0;
        let y = 30.0 + (index / 8) as f64 * 62.0;
        let vector = VectorContent::new(
            Circle::new((x, y), 24.25).to_path(0.001),
            Color::new(0.17, 0.57, 0.91, 0.71)?,
        )?;
        let node = session
            .transact(Edit::new(
                "Original curve",
                [Change::InsertLayer {
                    parent,
                    index,
                    layer: LayerSpec::new("Editable curve", Content::Vector(vector)),
                }],
            ))?
            .nodes[0];
        if index == 27 {
            target = Some(node);
        }
    }
    Ok((session, target))
}

fn photo() -> Result<(Session, Option<NodeId>)> {
    let mut session = session(6000.0, 4000.0)?;
    let parent = Parent::Page(session.snapshot().page_order()[0]);
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(6000 * 4000 * 4)?;
    for y in 0..4000 {
        for x in 0..6000 {
            pixels.extend_from_slice(&[
                (x % 251) as u8,
                (y % 241) as u8,
                ((x + y) % 239) as u8,
                if x % 13 == 0 { 91 } else { 231 },
            ]);
        }
    }
    let asset = Arc::new(RasterAsset::new(PixelSize::new(6000, 4000)?, pixels)?);
    session.transact(Edit::new(
        "Original24MP",
        [Change::InsertRaster {
            parent,
            index: 0,
            name: "Immutable24MP".to_owned(),
            asset,
            transform: Transform::IDENTITY,
        }],
    ))?;
    Ok((session, None))
}

fn request(session: &Session, options: &Options, position: usize) -> Result<RenderRequest> {
    let (width, height, affine) = if options.photo {
        (
            1024,
            768,
            Transform::new([
                1.0,
                0.0,
                0.0,
                1.0,
                -1536.0 - (position % 13) as f64 * 256.0,
                -1024.0,
            ])?,
        )
    } else {
        (512, 512, Transform::IDENTITY)
    };
    Ok(RenderRequest::new(
        session.snapshot().page_order()[0],
        width,
        height,
        View::Affine(affine),
    )?
    .with_format(options.format)?)
}

fn agree(actual: &Frame, expected: &Frame) -> Result<()> {
    if actual.request() != expected.request() || actual.revision() != expected.revision() {
        return Err(problem("oracle result identity differs"));
    }
    let equal = match (actual.pixels(), expected.pixels()) {
        (FramePixels::SrgbRgba8Straight(a), FramePixels::SrgbRgba8Straight(b)) => a == b,
        (
            FramePixels::LinearSrgbRgbaF32Premultiplied(a),
            FramePixels::LinearSrgbRgbaF32Premultiplied(b),
        ) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(a, b)| a.map(f32::to_bits) == b.map(f32::to_bits))
        }
        _ => false,
    };
    if !equal {
        return Err(problem(
            "retained output differs from fresh uncached oracle",
        ));
    }
    Ok(())
}

fn run(options: Options) -> Result<()> {
    let (mut session, target) = if options.photo { photo()? } else { curves()? };
    let limits = Limits {
        max_bytes: 256 * MIB,
        ..Limits::default()
    };
    let renderer = Renderer::new_retained(limits.clone(), 128 * MIB)?;
    let fresh = Renderer::new(limits)?;
    let warm = request(&session, &options, 0)?;
    let deadline = Instant::now() + Duration::from_secs(options.seconds);
    let warm_frame =
        renderer.render_with_cancel(&session.snapshot(), warm, || Instant::now() >= deadline)?;
    let expected = fresh.render(&session.snapshot(), warm)?;
    agree(&warm_frame, &expected)?;
    drop(expected);
    drop(warm_frame);
    let mut output = io::BufWriter::new(io::stdout().lock());
    writeln!(
        output,
        "workload,sample,width,height,output_format,render_ns,tile_hits,tile_misses,evaluated_tiles,geometry_hits,geometry_misses,prepared_curves,evictions,refused,resident_bytes,source_capacity_bytes,retained_pixel_capacity_bytes,metadata_admission_bytes,render_charged_bytes,output_bytes,oracle"
    )?;
    for sample in 0..options.samples {
        if let Some(node) = target {
            session.transact(Edit::new(
                "Move one of64curves",
                [Change::SetTransform {
                    node,
                    transform: Transform::new([
                        1.0,
                        0.0,
                        0.0,
                        1.0,
                        0.125 + sample as f64 * 0.125,
                        -0.25,
                    ])?,
                }],
            ))?;
        }
        let request = request(&session, &options, sample + 1)?;
        let snapshot = session.snapshot();
        let before = renderer.retained_stats();
        let deadline = Instant::now() + Duration::from_secs(options.seconds);
        let started = Instant::now();
        let frame =
            renderer.render_with_cancel(&snapshot, request, || Instant::now() >= deadline)?;
        let elapsed = started.elapsed().as_nanos();
        let after = renderer.retained_stats();
        let charged = renderer.allocated_bytes();
        let expected = fresh.render(&snapshot, request)?;
        agree(&frame, &expected)?;
        drop(expected);
        if fresh.allocated_bytes() != 0 {
            return Err(problem("fresh oracle retains unexpected allocation"));
        }
        let metadata = after
            .resident_bytes
            .checked_sub(after.source_bytes)
            .and_then(|bytes| bytes.checked_sub(after.retained_pixel_bytes))
            .ok_or_else(|| problem("residency diagnostic arithmetic"))?;
        let workload = if options.photo {
            "photo24-pan"
        } else {
            "curves64-edit"
        };
        let width = request.width();
        let height = request.height();
        let format = if options.format == OutputFormat::SrgbRgba8Straight {
            "rgba8"
        } else {
            "linear-f32"
        };
        let output_bytes = frame.byte_len();
        write!(
            output,
            "{workload},{sample},{width},{height},{format},{elapsed},"
        )?;
        write!(
            output,
            "{},{},{},{},{},{},{},{},",
            after.tile_hits - before.tile_hits,
            after.tile_misses - before.tile_misses,
            after.evaluated_tiles - before.evaluated_tiles,
            after.geometry_hits - before.geometry_hits,
            after.geometry_misses - before.geometry_misses,
            after.prepared_curves - before.prepared_curves,
            after.evictions - before.evictions,
            after.refused - before.refused
        )?;
        writeln!(
            output,
            "{},{},{},{metadata},{charged},{output_bytes},bit-exact",
            after.resident_bytes, after.source_bytes, after.retained_pixel_bytes
        )?;
        drop(frame);
    }
    renderer.clear_retained();
    if renderer.allocated_bytes() != 0 || renderer.retained_stats().resident_bytes != 0 {
        return Err(problem("retained owners did not release after clear"));
    }
    output.flush()?;
    Ok(())
}

fn main() -> ExitCode {
    match Options::parse().and_then(|options| options.map_or(Ok(()), run)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("retained workload failed: {error}");
            ExitCode::FAILURE
        }
    }
}
