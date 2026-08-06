use std::time::{Duration, Instant};

pub struct FrameProfiler {
    pub frame_history: [f32; 120], // Frame times in ms (last 120 frames)
    pub history_idx: usize,
    pub history_count: usize,
    pub last_telemetry_instant: Instant,

    // Subsystem timings for current frame (in ms)
    pub world_update_ms: f32,
    pub mesh_dispatch_ms: f32,
    pub gpu_upload_ms: f32,
    pub render_ms: f32,
}

impl Default for FrameProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameProfiler {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            frame_history: [16.6; 120],
            history_idx: 0,
            history_count: 0,
            last_telemetry_instant: now,
            world_update_ms: 0.0,
            mesh_dispatch_ms: 0.0,
            gpu_upload_ms: 0.0,
            render_ms: 0.0,
        }
    }

    /// Record a completed frame duration.
    pub fn record_frame(&mut self, frame_duration: Duration) {
        let frame_ms = (frame_duration.as_secs_f64() * 1000.0) as f32;
        self.frame_history[self.history_idx] = frame_ms;
        self.history_idx = (self.history_idx + 1) % 120;
        if self.history_count < 120 {
            self.history_count += 1;
        }
    }

    /// Calculate summary statistics (avg, min, p95, p99, 1% low fps) over recorded history.
    pub fn stats(&self) -> PerformanceStats {
        if self.history_count == 0 {
            return PerformanceStats::default();
        }

        let slice = &self.frame_history[..self.history_count];
        let mut sorted = slice.to_vec();
        sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let sum: f32 = sorted.iter().sum();
        let avg_ms = sum / sorted.len() as f32;
        let max_ms = sorted.last().copied().unwrap_or(16.6);

        let p95_idx = ((sorted.len() as f32 * 0.95) as usize).min(sorted.len() - 1);
        let p99_idx = ((sorted.len() as f32 * 0.99) as usize).min(sorted.len() - 1);

        let p95_ms = sorted[p95_idx];
        let p99_ms = sorted[p99_idx];

        let avg_fps = if avg_ms > 0.001 { 1000.0 / avg_ms } else { 0.0 };
        let min_fps = if max_ms > 0.001 { 1000.0 / max_ms } else { 0.0 };
        let low_1_fps = if p99_ms > 0.001 { 1000.0 / p99_ms } else { 0.0 };

        PerformanceStats {
            avg_ms,
            p95_ms,
            p99_ms,
            avg_fps,
            min_fps,
            low_1_fps,
        }
    }

    /// Emit periodic structured console log telemetry every 5 seconds.
    pub fn log_telemetry_if_needed(
        &mut self,
        loaded_chunks: usize,
        dirty_chunks: i32,
        in_flight_meshing: i32,
        mob_count: usize,
    ) {
        if self.last_telemetry_instant.elapsed() >= Duration::from_secs(5) {
            self.last_telemetry_instant = Instant::now();
            let stats = self.stats();
            println!(
                "[TELEMETRY] FPS: {:.0} avg ({:.0} min, {:.0} 1%low) | Frame Time: {:.2}ms avg (P95: {:.2}ms, P99: {:.2}ms) | Subsystems: Render {:.2}ms, World {:.2}ms, Mesh {:.2}ms, GPU Upload {:.2}ms | Chunks: {} loaded ({} dirty, {} in-flight) | Mobs: {}",
                stats.avg_fps,
                stats.min_fps,
                stats.low_1_fps,
                stats.avg_ms,
                stats.p95_ms,
                stats.p99_ms,
                self.render_ms,
                self.world_update_ms,
                self.mesh_dispatch_ms,
                self.gpu_upload_ms,
                loaded_chunks,
                dirty_chunks,
                in_flight_meshing,
                mob_count
            );
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PerformanceStats {
    pub avg_ms: f32,
    pub p95_ms: f32,
    pub p99_ms: f32,
    pub avg_fps: f32,
    pub min_fps: f32,
    pub low_1_fps: f32,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            avg_ms: 16.6,
            p95_ms: 16.6,
            p99_ms: 16.6,
            avg_fps: 60.0,
            min_fps: 60.0,
            low_1_fps: 60.0,
        }
    }
}
