//! Chunk streaming keeps database waits outside the frame thread.
use super::*;

pub(super) struct LoadedChunk {
    pub chunk: Box<Chunk>,
    pub containers: Vec<((i32, i32, i32), crate::container::Container)>,
}
impl From<Box<Chunk>> for LoadedChunk {
    fn from(chunk: Box<Chunk>) -> Self {
        Self {
            chunk,
            containers: Vec::new(),
        }
    }
}
pub(super) type GenerationResult = Result<LoadedChunk, (i32, i32, String)>;

#[cfg(not(target_arch = "wasm32"))]
struct TerrainSnapshot {
    x: i32,
    z: i32,
    blocks: crate::chunk::BlockArray,
    liquid_levels: crate::chunk::LightArray,
}
#[cfg(not(target_arch = "wasm32"))]
impl TerrainSnapshot {
    fn capture(chunk: &Chunk) -> Self {
        Self {
            x: chunk.x,
            z: chunk.z,
            blocks: chunk.blocks.clone(),
            liquid_levels: chunk.liquid_levels.clone(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct PendingEviction {
    index: usize,
    snapshot: std::sync::Arc<TerrainSnapshot>,
    receiver: std::sync::mpsc::Receiver<Result<(), String>>,
    result: std::cell::OnceCell<Result<(), String>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl PendingEviction {
    fn poll(&self) -> Option<&Result<(), String>> {
        if self.result.get().is_none() {
            match self.receiver.try_recv() {
                Ok(result) => {
                    let _ = self.result.set(result);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    let _ = self
                        .result
                        .set(Err("Chunk save worker disconnected".into()));
                }
            }
        }
        self.result.get()
    }
    fn wait(&self) -> Result<(), String> {
        self.result
            .get_or_init(|| {
                self.receiver
                    .recv()
                    .unwrap_or_else(|_| Err("Chunk save worker disconnected".into()))
            })
            .clone()
    }
}

impl World {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn finish_chunk_save(&self) -> std::io::Result<()> {
        if let Some(pending) = &self.pending_eviction {
            pending.wait().map_err(std::io::Error::other)?;
        }
        Ok(())
    }

    fn displacement_ready(&mut self, index: usize) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(store) = self.bedrock.clone() else {
                return true;
            };
            if let Some(pending) = &self.pending_eviction {
                match pending.poll() {
                    None => return false,
                    Some(Err(error)) => {
                        self.storage_error =
                            Some(format!("Could not save displaced chunk: {error}"));
                        return false;
                    }
                    Some(Ok(())) => {}
                }
                if pending.index == index
                    && self.chunks[index].as_ref().is_some_and(|old| {
                        old.x == pending.snapshot.x
                            && old.z == pending.snapshot.z
                            && old.blocks == pending.snapshot.blocks
                            && old.liquid_levels == pending.snapshot.liquid_levels
                    })
                {
                    self.pending_eviction = None;
                    return true;
                }
                self.pending_eviction = None;
            }
            let Some(old) = &self.chunks[index] else {
                return true;
            };
            let seed = old.seed;
            let snapshot = std::sync::Arc::new(TerrainSnapshot::capture(old));
            let work = snapshot.clone();
            let dimension = self.dimension;
            let (sender, receiver) = std::sync::mpsc::channel();
            crate::platform::spawn(move || {
                let mut chunk = Chunk::new(work.x, work.z, seed);
                chunk.blocks = work.blocks.clone();
                chunk.liquid_levels = work.liquid_levels.clone();
                let result = store
                    .save_chunks(std::iter::once(&chunk), dimension, vec![])
                    .map_err(|error| error.to_string());
                let _ = sender.send(result);
            });
            self.pending_eviction = Some(PendingEviction {
                index,
                snapshot,
                receiver,
                result: std::cell::OnceCell::new(),
            });
            false
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = index;
            true
        }
    }

    pub(super) fn integrate_chunks(&mut self, pcx: i32, pcz: i32) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(pending) = &self.pending_eviction
            && let Some(Err(error)) = pending.poll()
        {
            self.storage_error = Some(format!("Could not save displaced chunk: {error}"));
        }
        let start = crate::platform::Instant::now();
        let mut integrated = 0;
        while self.storage_error.is_none() {
            if !self.is_loading && (integrated >= 2 || start.elapsed().as_secs_f32() >= 0.002) {
                break;
            }
            let result = match self.pending_chunk.take() {
                Some(chunk) => Ok(chunk),
                None => match self.gen_result_rx.try_recv() {
                    Ok(result) => result,
                    Err(_) => break,
                },
            };
            let loaded = match result {
                Ok(chunk) => chunk,
                Err((x, z, error)) => {
                    self.gen_in_flight.remove(&(x, z));
                    self.storage_error = Some(format!("Could not load chunk ({x}, {z}): {error}"));
                    break;
                }
            };
            let (x, z) = (loaded.chunk.x, loaded.chunk.z);
            if (x - pcx).abs() > self.view_distance || (z - pcz).abs() > self.view_distance {
                self.gen_in_flight.remove(&(x, z));
                continue;
            }
            let index = self.get_pool_index(x, z);
            if self.chunk_slot_holds(x, z) {
                self.gen_in_flight.remove(&(x, z));
                continue;
            }
            if !self.displacement_ready(index) {
                self.pending_chunk = Some(loaded);
                break;
            }
            self.gen_in_flight.remove(&(x, z));
            if self.chunks[index].as_ref().is_some_and(|old| old.dirty) {
                self.dirty_count -= 1;
            }
            self.chunks[index] = Some(loaded.chunk);
            self.apply_edits_to_chunk(x, z);
            // Validate against live edits, not the worker's earlier terrain snapshot.
            for (position, container) in loaded.containers {
                if self.get_block(position.0, position.1, position.2) == container.block() {
                    self.containers.entry(position).or_insert(container);
                }
            }
            if let Some(chunk) = &mut self.chunks[index] {
                chunk.dirty = true;
                self.dirty_count += 1;
            }
            for (nx, nz) in [(x - 1, z), (x + 1, z), (x, z - 1), (x, z + 1)] {
                if let Some(neighbor) = self.get_chunk_mut(nx, nz)
                    && !neighbor.dirty
                {
                    neighbor.dirty = true;
                    self.dirty_count += 1;
                }
            }
            self.try_spawn_village_mobs(x, z);
            self.try_spawn_natural_mobs(x, z);
            self.chunks_generated_count += 1;
            self.sync_gpu_chunk(x, z, pcx, pcz);
            integrated += 1;
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};

    fn world_with_displacement() -> (World, Arc<crate::bedrock::session::WorldStore>, PathBuf) {
        let path =
            std::env::temp_dir().join(format!("voxel-stream-save-{}", rand::random::<u64>()));
        let store = Arc::new(crate::bedrock::session::WorldStore::create(&path, 42).unwrap());
        let mut world = World::simulation(42);
        world.bedrock = Some(store.clone());
        world.is_loading = false;
        world.view_distance = 0;
        let mut old = Box::new(Chunk::new(0, 0, 42));
        old.dirty = false;
        old.blocks[1][1][1] = BlockType::Stone;
        let index = world.get_pool_index(0, 0);
        world.chunks[index] = Some(old);
        world.gen_in_flight.insert((POOL_WIDTH, 0));
        world
            .gen_result_tx
            .send(Ok(Box::new(Chunk::new(POOL_WIDTH, 0, 42)).into()))
            .unwrap();
        (world, store, path)
    }

    #[test]
    fn terrain_changes_during_save_are_durable_before_eviction() {
        let (mut world, store, path) = world_with_displacement();
        world.integrate_chunks(POOL_WIDTH, 0);
        assert!(world.get_chunk(0, 0).is_some());
        let old = world.get_chunk_mut(0, 0).unwrap();
        old.blocks[1][1][1] = BlockType::Dirt;
        old.blocks[2][1][1] = BlockType::Water;
        old.liquid_levels[2][1][1] = crate::chunk::WATER_SOURCE;
        world.finish_chunk_save().unwrap();
        world.integrate_chunks(POOL_WIDTH, 0);
        assert!(
            world.get_chunk(0, 0).is_some(),
            "changed terrain must be saved again"
        );
        world.finish_chunk_save().unwrap();
        world.integrate_chunks(POOL_WIDTH, 0);
        assert!(world.get_chunk(POOL_WIDTH, 0).is_some());
        let saved = store.read_chunk(0, 0, 0).unwrap().unwrap();
        assert_eq!(saved.blocks[1][1][1], BlockType::Dirt);
        assert_eq!(saved.blocks[2][1][1], BlockType::Water);
        assert_eq!(saved.liquid_levels[2][1][1], crate::chunk::WATER_SOURCE);
        drop(world);
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn failed_save_keeps_outgoing_terrain_and_rejects_final_save() {
        let (mut world, store, path) = world_with_displacement();
        let (sender, receiver) = mpsc::channel();
        sender.send(Err("injected disk error".into())).unwrap();
        world.pending_eviction = Some(PendingEviction {
            index: world.get_pool_index(0, 0),
            snapshot: Arc::new(TerrainSnapshot::capture(&Chunk::new(0, 0, 42))),
            receiver,
            result: std::cell::OnceCell::new(),
        });
        world.integrate_chunks(POOL_WIDTH, 0);
        assert!(
            world
                .storage_error
                .as_ref()
                .unwrap()
                .contains("injected disk error")
        );
        assert_eq!(
            world.get_chunk(0, 0).unwrap().blocks[1][1][1],
            BlockType::Stone
        );
        assert!(world.get_chunk(POOL_WIDTH, 0).is_none());
        assert!(world.finish_chunk_save().is_err());
        drop(world);
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn headless_mesh_completion_clears_the_in_flight_flag() {
        let mut world = World::simulation(42);
        world.is_loading = false;
        world.last_pcx = 0;
        world.last_pcz = 0;
        let mut chunk = Box::new(Chunk::new(0, 0, 42));
        chunk.dirty = false;
        chunk.meshing_in_progress = true;
        chunk.mesh_job_id = 7;
        let light = chunk.light.clone();
        let index = world.get_pool_index(0, 0);
        world.chunks[index] = Some(chunk);
        world.meshing_in_flight = 1;
        let mesh = || crate::chunk::MeshData {
            v: vec![],
            t: vec![],
            n: vec![],
            c: vec![],
        };
        world
            .mesh_result_tx
            .send(MeshResult {
                x: 0,
                z: 0,
                job_id: 7,
                light,
                opaque: mesh(),
                transparent: mesh(),
                water: mesh(),
            })
            .unwrap();
        world.update(Vec3::new(0., 180., 0.), 0., BlockType::Air);
        assert!(!world.get_chunk(0, 0).unwrap().meshing_in_progress);
        assert_eq!(world.meshing_in_flight, 0);
    }

    #[test]
    fn completed_chunks_are_integrated_in_bounded_batches() {
        let mut world = World::simulation(42);
        world.is_loading = false;
        for x in 0..8 {
            world.gen_in_flight.insert((x, 0));
            world
                .gen_result_tx
                .send(Ok(Box::new(Chunk::new(x, 0, 42)).into()))
                .unwrap();
        }
        world.integrate_chunks(0, 0);
        assert!((1..=2).contains(&world.chunks_generated_count));
        assert!(world.gen_in_flight.len() >= 6);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod streaming_profile {
    use super::*;
    #[test]
    fn streaming_does_not_wait_for_the_storage_lock() {
        use std::sync::{Arc, mpsc};
        let path =
            std::env::temp_dir().join(format!("voxel-stream-lock-{}", rand::random::<u64>()));
        let store = Arc::new(crate::bedrock::session::WorldStore::create(&path, 42).unwrap());
        let mut world = World::simulation(42);
        world.bedrock = Some(store.clone());
        world.is_loading = false;
        world.view_distance = 0;
        world.last_pcx = POOL_WIDTH;
        world.last_pcz = 0;
        let index = world.get_pool_index(0, 0);
        let mut old = Box::new(Chunk::new(0, 0, 42));
        old.dirty = false;
        old.blocks[1][1][1] = BlockType::Stone;
        world.chunks[index] = Some(old);
        world.gen_in_flight.insert((POOL_WIDTH, 0));
        world
            .gen_result_tx
            .send(Ok(Box::new(Chunk::new(POOL_WIDTH, 0, 42)).into()))
            .unwrap();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let locked = store.clone();
        let worker = std::thread::spawn(move || {
            let mut released = false;
            locked.with_locked_database(|| {
                ready_tx.send(()).unwrap();
                released = release_rx
                    .recv_timeout(std::time::Duration::from_secs(2))
                    .is_ok();
            });
            released
        });
        ready_rx.recv().unwrap();
        world.update(
            Vec3::new(POOL_WIDTH as f32 * 16., 180., 0.),
            0.,
            BlockType::Air,
        );
        let _ = release_tx.send(());
        let nonblocking = worker.join().unwrap();
        assert!(
            world.get_chunk(0, 0).is_some(),
            "keep the outgoing chunk until its save finishes"
        );
        world.finish_chunk_save().unwrap();
        drop(world);
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
        assert!(
            nonblocking,
            "chunk integration waited for the database lock"
        );
    }
    #[test]
    #[ignore = "release-mode movement profile"]
    fn profile_forward_streaming() {
        use std::time::{Duration, Instant};
        let path = std::env::temp_dir().join(format!("voxel-movement-{}", rand::random::<u64>()));
        let store = std::sync::Arc::new(
            crate::bedrock::session::WorldStore::create(&path, 1074691402050369410).unwrap(),
        );
        let mut world = World::simulation(store.seed);
        world.bedrock = Some(store.clone());
        world.view_distance = 4;
        let start = Instant::now();
        let mut progress = Instant::now();
        while world.is_loading {
            world.update(Vec3::new(0., 180., 0.), 0., BlockType::Air);
            assert!(world.storage_error.is_none(), "{:?}", world.storage_error);
            if progress.elapsed().as_secs() >= 5 {
                eprintln!(
                    "STREAM loading: {} loaded, {} generation, {} dirty, {} meshing",
                    world.chunks_generated_count,
                    world.gen_in_flight.len(),
                    world.dirty_count,
                    world.meshing_in_flight
                );
                progress = Instant::now();
            }
            assert!(start.elapsed().as_secs() < 120);
            std::thread::sleep(Duration::from_millis(2));
        }
        println!("STREAM warmup: {:?}", start.elapsed());
        let mut frames = Vec::new();
        for frame in 0..720 {
            let position = Vec3::new(0., 180., frame as f32 * 0.8);
            let start = Instant::now();
            world.update(position, 1. / 60., BlockType::Air);
            frames.push(start.elapsed().as_secs_f64() * 1000.);
            assert!(world.storage_error.is_none(), "{:?}", world.storage_error);
            std::thread::sleep(Duration::from_millis(4));
        }
        frames.sort_by(f64::total_cmp);
        println!(
            "STREAM updates: p50={:.3}ms p95={:.3}ms p99={:.3}ms max={:.3}ms >16ms={}",
            frames[360],
            frames[684],
            frames[712],
            frames[719],
            frames.iter().filter(|&&v| v > 16.).count()
        );
        let drain_start = Instant::now();
        while !world.gen_in_flight.is_empty() || world.meshing_in_flight > 0 {
            assert!(drain_start.elapsed().as_secs() < 120);
            world.update(Vec3::new(0., 180., 575.2), 0., BlockType::Air);
            std::thread::sleep(Duration::from_millis(2));
        }
        world.finish_chunk_save().unwrap();
        drop(world);
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }
}
