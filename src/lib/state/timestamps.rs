use std::sync;

/*
pub trait Timer {
    fn init(device: &wgpu::Device) -> Self;
    fn desc(&self) -> wgpu::ComputePassDescriptor;
    fn pre(&self);
    fn post(&self, encoder: &mut wgpu::CommandEncoder);
    fn state(&self) -> bool;
} */

#[derive(Debug)]
pub struct Scheduler {
    #[allow(dead_code)]
    period: f32,
    completed: sync::Arc<sync::atomic::AtomicBool>,
    set: Option<wgpu::QuerySet>,
    buffer: wgpu::Buffer,
    buffer_read: wgpu::Buffer,
}

impl Scheduler {
    pub fn init(queue: &wgpu::Queue, device: &wgpu::Device) -> Self {
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let set = None;
            } else {
                let set = Some(device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: None,
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                }));
            }
        }

        Self {
            period: queue.get_timestamp_period(),
            completed: sync::Arc::new(sync::atomic::AtomicBool::new(true)),
            set,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 2 * wgpu::QUERY_SIZE as u64,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            buffer_read: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 2 * wgpu::QUERY_SIZE as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: true,
            }),
        }
    }

    pub fn desc(&self) -> wgpu::ComputePassDescriptor {
        let Self { set, .. } = self;

        wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: set.as_ref().map(|query_set| wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            }),
        }
    }

    pub fn pre(&self, encoder: &mut wgpu::CommandEncoder) {
        let Self {
            set, 
            buffer, .. 
        } = self;

        if let Some(query_set) = set {
            encoder.resolve_query_set(query_set, 0..2, buffer, 0);
        }         
    }

    pub fn post(&self, queue: &wgpu::Queue, device: &wgpu::Device) {
        let Self { 
            completed,
            buffer, 
            buffer_read, .. 
        } = self;

        // We will submit a second set of encoded commands which 
        // is responsible for copying timestamp information to `buffer_read`
        let mut encoder = device.create_command_encoder(&{
            wgpu::CommandEncoderDescriptor::default()
        });

        // Queue the copy operation
        encoder.copy_buffer_to_buffer(
            buffer, 0, 
            buffer_read, 0, 
            2 * wgpu::QUERY_SIZE as u64,
        );

        // Submit the command
        queue.submit(Some(encoder.finish()));

        // Update state when the copy has executed...
        // by extension, this tells us when the 
        let completed = completed.clone();
        buffer_read.slice(..).map_async(wgpu::MapMode::Read, move |_| {
            completed.store(true, sync::atomic::Ordering::Release);
        });
    }

    pub fn ready(&self) -> bool {
        let Self {
            completed,
            buffer_read, .. 
        } = self;

        let completed = completed
            .fetch_and(false, sync::atomic::Ordering::Acquire);

        #[cfg(not(target_arch = "wasm32"))] {
            let Self { period, .. } = self;

            if completed {
                let data = buffer_read.slice(..).get_mapped_range();

                let timestamps = data
                    .chunks_exact(wgpu::QUERY_SIZE as usize)
                    .take(2)
                    .map(|time| u64::from_ne_bytes(time.try_into().unwrap()))
                    .collect::<Vec<_>>();

                let [start, end, ..] = timestamps[..] else { unreachable!(); };

                if let Some(frame_time) = end.checked_sub(start) {
                    let frame_time = period * 0.000001 * frame_time as f32;

                    // TODO: Calculate a running average
                    log::info!("{:?}", frame_time);
                }
            }
        }    

        if completed {
            buffer_read.unmap();
        }

        completed
    }
}