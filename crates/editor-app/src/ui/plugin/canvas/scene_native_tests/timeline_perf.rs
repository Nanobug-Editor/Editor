//! Stream actual window events and normal renderer/runtime counters; samples never accumulate in RAM.
use super::*;
use gpui_test_foundation::profiler::{
    self, FrameDurationSnapshot, FrameEvent, FrameTimingCollector, InputLatencySnapshot,
};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};
use wasm::timeline::Snapshot;

/// The same bounded profiler and sampling overhead applies to full and patch publication runs.
pub(super) struct Recorder {
    path: PathBuf,
    file: BufWriter<File>,
    start: Instant,
    collector: FrameTimingCollector,
    previous_trace: bool,
    window_id: gpui_kit::WindowId,
    draw_base: FrameDurationSnapshot,
    input_base: InputLatencySnapshot,
    byte_base: u64,
    publication_base: u64,
    frames_base: u64,
    peaks: serde_json::Value,
    raw_draws: u64,
    raw_presents: u64,
}
impl Recorder {
    /// Begin after cold package activation and smoke; no warmup samples enter the measured histograms.
    pub fn start(
        mode: &str,
        seconds: u64,
        snapshot: &Snapshot,
        window: &Window,
    ) -> anyhow::Result<Self> {
        let path = std::env::var_os("NANOBUG_CANVAS_TIMELINE_RAW")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../target")
                    .join(format!(
                        "canvas-timeline-{mode}-{}.jsonl",
                        std::process::id()
                    ))
            });
        let previous_trace = profiler::trace_enabled();
        profiler::set_trace_enabled(true);
        let mut result = Self {
            file: BufWriter::new(File::create(&path)?),
            path,
            start: Instant::now(),
            collector: FrameTimingCollector::new(),
            previous_trace,
            window_id: window.window_handle().window_id(),
            draw_base: window.frame_duration_snapshot(),
            input_base: window.input_latency_snapshot(),
            byte_base: snapshot.bytes,
            publication_base: snapshot.publications,
            frames_base: snapshot.state["frames"].as_u64().unwrap_or(0),
            peaks: serde_json::json!({}),
            raw_draws: 0,
            raw_presents: 0,
        };
        let _ = result.collector.collect_unseen();
        result.write(serde_json::json!({"kind":"header","schema":1,"mode":mode,"requested_seconds":seconds,
            "profile":if cfg!(debug_assertions) {"debug"} else {"release"},
            "seed":snapshot.state["seed"],"initial_state":snapshot.state,"logical_items":100000,
            "cache_work_base":{"prepare":snapshot.images.statistics().prepared_items,
                "reuse":snapshot.images.statistics().reused_items,"shape":snapshot.images.statistics().paragraph_layouts,
                "raster":snapshot.images.statistics().rasterizations},
            "package":std::env::var_os("NANOBUG_CANVAS_TIMELINE_PACKAGE").map(|value|value.to_string_lossy().into_owned()),
            "exe":std::env::current_exe()?.to_string_lossy(),
            "machine":std::env::var("COMPUTERNAME").ok(),"cpu":std::env::var("PROCESSOR_IDENTIFIER").ok(),
            "logical_cpus":std::thread::available_parallelism()?.get(),"architecture":std::env::consts::ARCH,"os":std::env::consts::OS,
            "dpi_scale":window.scale_factor(),
            "observer":{"profiler":"gpui-pre 0.3.9 profiler; global frame ring 16 MiB; task rings 16 MiB per thread",
                "trace_rotation_seconds":1,"trace_rotation":"drain native frames, disable/clear trace buffers, reenable with a fresh cursor; HDR is unaffected",
                "raw_frame_poll_ms":100,"counter_sample_ms":1000,"runtime_capture_renew_seconds":45,
                "peaks_scope":"maximum of 1-second observations, not an OS-wide absolute peak",
                "input_latency_scope":"Window native dispatch to drawn frame; hardware and SendInput dispatch delay are excluded",
                "renderer_cache":"same retained production cache in both modes",
                "ingress_queue":"fixture adapter, capacity128; this is not the editor WorkSender queue",
                "memory":"OS working set and private commit; native GPU atlas bytes are not measured"}}))?;
        Ok(result)
    }
    /// Drain the bounded native trace frequently; only this real window's events are persisted.
    pub fn frames(&mut self) -> anyhow::Result<()> {
        for event in self.collector.collect_unseen() {
            match event {
                FrameEvent::Draw(frame) if frame.window_id == self.window_id => self.raw_draws += 1,
                FrameEvent::Present(frame) if frame.window_id == self.window_id => {
                    self.raw_presents += 1
                }
                _ => {}
            }
            let row = match event {
                FrameEvent::Draw(frame) if frame.window_id == self.window_id => serde_json::json!({
                    "kind":"draw","at_ns":frame.draw_start.saturating_duration_since(self.start).as_nanos(),
                    "duration_ns":frame.draw_end.duration_since(frame.draw_start).as_nanos(),
                    "dirty_age_ns":frame.dirty_at.map(|dirty|frame.draw_start.saturating_duration_since(dirty).as_nanos()),
                    "invalidations":frame.invalidations,"layout_ns":frame.phases.request_layout.as_nanos(),
                    "prepaint_ns":frame.phases.prepaint.as_nanos(),"paint_ns":frame.phases.paint.as_nanos(),
                    "refresh_ns":frame.refresh_interval.map(|interval|interval.as_nanos())}),
                FrameEvent::Present(frame) if frame.window_id == self.window_id => {
                    serde_json::json!({
                    "kind":"present","at_ns":frame.present_start.saturating_duration_since(self.start).as_nanos(),
                    "duration_ns":frame.present_duration().as_nanos(),
                    "animation_interval_ns":frame.animation_interval.map(|interval|interval.as_nanos())})
                }
                _ => continue,
            };
            self.write(row)?;
        }
        Ok(())
    }
    /// Current and live renderer residency stay distinct; work counters are cumulative SceneImages values.
    pub fn sample(
        &mut self,
        snapshot: &Snapshot,
        window: &Window,
        final_sample: bool,
    ) -> anyhow::Result<()> {
        self.frames()?;
        // The same UI turn cannot draw while rotating; background task buffers are bounded by one
        // sample interval rather than growing toward 16 MiB per executor thread over a ten-minute run.
        profiler::set_trace_enabled(false);
        profiler::set_trace_enabled(true);
        self.collector = FrameTimingCollector::new();
        let mut draw = window.frame_duration_snapshot();
        let mut input = window.input_latency_snapshot();
        draw.draw_duration_histogram
            .subtract(&self.draw_base.draw_duration_histogram)?;
        draw.dirty_to_present_histogram
            .subtract(&self.draw_base.dirty_to_present_histogram)?;
        draw.present_interval_histogram
            .subtract(&self.draw_base.present_interval_histogram)?;
        input
            .latency_histogram
            .subtract(&self.input_base.latency_histogram)?;
        input
            .events_per_frame_histogram
            .subtract(&self.input_base.events_per_frame_histogram)?;
        let statistics = snapshot.images.statistics();
        let (rss, private) = super::timeline_os::memory()?;
        for (name, value) in [
            ("rss_bytes", rss),
            ("private_bytes", private),
            ("live_derived_pixel_bytes", statistics.live_resident_bytes),
            ("live_textures", statistics.live_textures as u64),
            ("cached_items", statistics.cached_items as u64),
            ("native_vertices", statistics.vertices as u64),
            ("instance_resources", snapshot.resources as u64),
            ("fixture_ingress", snapshot.queue_peak as u64),
        ] {
            let previous = self.peaks[name].as_u64().unwrap_or(0);
            self.peaks[name] = serde_json::json!(previous.max(value));
        }
        // HDR histograms report nanoseconds. Empty observations remain null, never fabricated zero latency.
        macro_rules! distribution { ($histogram:expr) => {{ let histogram=&$histogram;
            if histogram.is_empty() {serde_json::Value::Null} else {serde_json::json!({
                "count":histogram.len(),"p50_ns":histogram.value_at_quantile(0.50),
                "p95_ns":histogram.value_at_quantile(0.95),"p99_ns":histogram.value_at_quantile(0.99),
                "max_ns":histogram.max()})}
        }}; }
        self.write(serde_json::json!({"kind":if final_sample {"summary"} else {"sample"},
            "elapsed_ms":self.start.elapsed().as_millis(),"frames":snapshot.state["frames"].as_u64().unwrap_or(0)-self.frames_base,
            "offset":snapshot.state["offset"],"business_ticks":snapshot.state["business_ticks"],
            "publication_bytes":snapshot.bytes-self.byte_base,"publications":snapshot.publications-self.publication_base,
            "publication_missing_byte_records":snapshot.missing_publication_bytes,
            "publication_records_complete":snapshot.publications==snapshot.expected_publications,
            "publication_records_expected":snapshot.expected_publications,
            "raw_draw_events":self.raw_draws,"raw_present_events":self.raw_presents,
            "draw_fps":self.raw_draws as f64/self.start.elapsed().as_secs_f64(),
            "present_fps":self.raw_presents as f64/self.start.elapsed().as_secs_f64(),
            "frame_trace_complete":self.raw_draws==draw.draw_duration_histogram.len(),
            "observation_ring_evictions":snapshot.observation_dropped,
            "rss_bytes":rss,"private_bytes":private,"peaks":self.peaks,
            "cache":{"prepare":statistics.prepared_items,"reuse":statistics.reused_items,
                "shape":statistics.paragraph_layouts,"raster":statistics.rasterizations,
                "current_derived_pixel_bytes":statistics.resident_bytes,
                "live_derived_pixel_bytes":statistics.live_resident_bytes,"live_textures":statistics.live_textures,
                "items":statistics.cached_items,"vertices":statistics.vertices},
            "fixture_queue_overflow":snapshot.queue_overflow,"resources":snapshot.resources,
            "draw":distribution!(draw.draw_duration_histogram),
            "dirty_to_present":distribution!(draw.dirty_to_present_histogram),
            "present_interval":distribution!(draw.present_interval_histogram),
            "input_to_frame":distribution!(input.latency_histogram),
            "input_mid_draw_excluded":input.mid_draw_events_dropped-self.input_base.mid_draw_events_dropped}))?;
        self.file.flush()?;
        if final_sample {
            eprintln!("native timeline raw report: {}", self.path.display());
        }
        Ok(())
    }
    /// Each JSON line is written immediately into a bounded file buffer; failure aborts the run.
    fn write(&mut self, row: serde_json::Value) -> anyhow::Result<()> {
        serde_json::to_writer(&mut self.file, &row)?;
        self.file.write_all(b"\n")?;
        Ok(())
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        profiler::set_trace_enabled(self.previous_trace);
        let _ = self.file.flush();
    }
}
