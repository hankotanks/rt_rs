use std::{sync, thread};
pub struct SchedulerEntry<'a> {
    pub ty: wgpu::BindingType,
    pub resource: wgpu::BindingResource<'a>,
}

pub trait Scheduler {
    fn init(queue: &wgpu::Queue, device: &wgpu::Device) -> Self;
    fn entry(&self) -> Option<SchedulerEntry>;
    fn desc(&self) -> wgpu::ComputePassDescriptor;
    fn pre(&self, encoder: &mut wgpu::CommandEncoder);
    fn post(&self, queue: &wgpu::Queue, device: &wgpu::Device);
    fn ready(&mut self) -> bool;
}

pub struct DefaultScheduler {
    completed: sync::Arc<sync::atomic::AtomicBool>,
    buffer: wgpu::Buffer,
    buffer_read: wgpu::Buffer,
}

impl Scheduler for DefaultScheduler {
    fn init(_queue: &wgpu::Queue, device: &wgpu::Device) -> Self {
        Self {
            completed: sync::Arc::new(sync::atomic::AtomicBool::new(true)),
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: wgpu::MAP_ALIGNMENT,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            buffer_read: device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: wgpu::MAP_ALIGNMENT,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: true,
            }),
        }
    }

    fn entry(&self) -> Option<SchedulerEntry<'_>> {
        let entry = SchedulerEntry { 
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            }, 
            resource: self.buffer.as_entire_binding(), 
        };

        Some(entry)
    }

    fn desc(&self) -> wgpu::ComputePassDescriptor {
        wgpu::ComputePassDescriptor::default()
    }

    fn pre(&self, encoder: &mut wgpu::CommandEncoder) {
        let Self { 
            buffer, 
            buffer_read, .. 
        } = self;

        // Queue the copy operation
        encoder.copy_buffer_to_buffer(
            buffer, 0, 
            buffer_read, 0, 
            wgpu::MAP_ALIGNMENT,
        );
    }

    fn post(&self, _queue: &wgpu::Queue, _device: &wgpu::Device) {
        let Self { 
            completed, 
            buffer_read, .. 
        } = self;

        let completed = completed.clone();
        buffer_read.slice(..).map_async(wgpu::MapMode::Read, move |_| {
            completed.store(true, sync::atomic::Ordering::Release);
        });
    }

    fn ready(&mut self) -> bool {
        let Self { 
            completed, 
            buffer_read, .. 
        } = self;

        let completed = completed
            .fetch_and(false, sync::atomic::Ordering::Acquire);

        if completed {
            buffer_read.unmap();
        }

        completed
    }
}

#[derive(Debug)]
pub struct BenchScheduler {
    period: f32,
    completed: sync::Arc<sync::atomic::AtomicBool>,
    set: wgpu::QuerySet,
    buffer: wgpu::Buffer,
    buffer_read: wgpu::Buffer,
    times_handle: thread::JoinHandle<()>,
    times_sender: sync::mpsc::Sender<f32>,
}

impl Scheduler for BenchScheduler {
    fn init(queue: &wgpu::Queue, device: &wgpu::Device) -> Self {
        let (times_sender, times_reciever) = sync::mpsc::channel();

        let times_handle = std::thread::spawn(move || {
            let mut data = Vec::new();

            loop {
                match times_reciever.recv() {
                    Ok(value) => //
                        data.push((data.len() as f64, value as f64)),
                    Err(_) => break,
                }

                // TODO: Do I really have to clone the data here?
                let chart = plotlib::repr::Plot::new(data.clone())
                    .line_style(plotlib::style::LineStyle::new().colour("#FF0000"));

                let chart_view = { 
                    let data_max = *data
                        .iter()
                        .map(|(_, value)| value)
                        .max_by(|a, b| a.total_cmp(b))
                        .unwrap_or(&1.);

                    let data_min = *data
                        .iter()
                        .map(|(_, value)| value)
                        .max_by(|a, b| b.total_cmp(a))
                        .unwrap_or(&0.);

                    plotlib::view::ContinuousView::new()
                        .add(chart)
                        .y_range(data_min, data_max)
                        .x_range(0., data.len() as f64)
                };

                let chart_page = plotlib::page::Page::single(&chart_view);

                let _ = chart_page.save("benchmark.svg");
            } 
        });

        Self {
            period: queue.get_timestamp_period(),
            completed: sync::Arc::new(sync::atomic::AtomicBool::new(true)),
            set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: None,
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
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
            times_handle,
            times_sender,
        }
    }

    fn entry(&self) -> Option<SchedulerEntry<'_>> { None }

    fn desc(&self) -> wgpu::ComputePassDescriptor {
        let Self { set: query_set, .. } = self;

        wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            }),
        }
    }

    fn pre(&self, encoder: &mut wgpu::CommandEncoder) {
        let Self {
            set: query_set, 
            buffer, .. 
        } = self;

        encoder.resolve_query_set(query_set, 0..2, buffer, 0);    
    }

    fn post(&self, queue: &wgpu::Queue, device: &wgpu::Device) {
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

    fn ready(&mut self) -> bool {
        let Self {
            period,
            completed,
            buffer_read, 
            times_sender, .. 
        } = self;

        let completed = completed
            .fetch_and(false, sync::atomic::Ordering::Acquire);

        if completed {
            let data = buffer_read.slice(..).get_mapped_range();

            let timestamps = data
                .chunks_exact(wgpu::QUERY_SIZE as usize)
                .take(2)
                .map(|time| u64::from_ne_bytes(time.try_into().unwrap()))
                .collect::<Vec<_>>();

            let [start, end, ..] = timestamps[..] else { unreachable!(); };

            if let Some(frame_time) = end.checked_sub(start) {
                let frame_time = 0.000001 * *period * frame_time as f32;

                let _ = times_sender.send(frame_time);
            }
        }  

        if completed {
            buffer_read.unmap();
        }

        completed
    }
}