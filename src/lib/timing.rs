use std::{sync, thread};

use resvg::tiny_skia;

use crate::handlers;

pub struct SchedulerEntry<'a> {
    pub ty: wgpu::BindingType,
    pub resource: wgpu::BindingResource<'a>,
}

pub trait Scheduler {
    fn init(
        queue: &wgpu::Queue, 
        device: &wgpu::Device, 
        stats: handlers::IntrsStats
    ) -> Self;

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
    fn init(
        _queue: &wgpu::Queue, 
        device: &wgpu::Device, 
        _stats: handlers::IntrsStats
    ) -> Self {

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
    #[allow(dead_code)]
    times_handle: thread::JoinHandle<anyhow::Result<()>>,
    times_sender: sync::mpsc::Sender<f32>,
}

impl BenchScheduler {
    // The total number of entries to benchmark
    // If None: Benchmarking won't stop
    const GRAPH_ENTRIES: Option<usize> = None;

    // Create a graph every N compute passes
    // If None: Graph is only generated when N passes have run such that
    // GRAPH_ENTRIES == Some(N)
    const GRAPH_ENTRY_INTERVAL: Option<usize> = Some(10);
}

impl Scheduler for BenchScheduler {
    fn init(
        queue: &wgpu::Queue, 
        device: &wgpu::Device, 
        stats: handlers::IntrsStats
    ) -> Self {
        let (times_sender, times_reciever) = sync::mpsc::channel();

        let times_handle = std::thread::spawn(move || {
            let mut data = Vec::new();

            // The running average of compute pass durations
            let mut avg = 0.;

            loop {
                match times_reciever.recv() {
                    Ok(value) if value == 0. => continue,
                    Ok(value) => {
                        // Begin computing running average
                        avg *= data.len() as f32;
                        avg += value;

                        // Add the new data value
                        data.push((data.len() as f64, value as f64));

                        // Complete the running average
                        avg /= data.len() as f32;

                        // Indicates that the final data point has been collected
                        let complete = matches!(Some(data.len()), Self::GRAPH_ENTRIES);

                        // If the last pass has completed or interval is reached
                        if Self::GRAPH_ENTRY_INTERVAL
                            .map(|i| data.len() % i)
                            .unwrap_or(1) == 0 || complete {
                            
                            // Generate the graph and save it
                            match graph(&data, Some(avg), stats) {
                                Ok(pixels) => {
                                    let _ = pixels.save_png("benchmark.png");
                                }
                                Err(e) => anyhow::bail!(e),
                            }
                        }

                        // Break if we've recorded all data points
                        if complete {
                            break Ok(());
                        }
                    },
                    Err(_) => break Ok(()),
                }
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

// Construct a graph from data points
fn graph(
    data: &[(f64, f64)], 
    avg: Option<f32>, 
    stats: handlers::IntrsStats
) -> anyhow::Result<tiny_skia::Pixmap> {
    use plotlib::{repr, view, style, page};

    use resvg::usvg;

    use once_cell::sync;

    let data_min = data
        .iter()
        .map(|(_, value)| *value)
        .fold(f64::INFINITY, |a, b| a.min(b));

    let data_max = data
        .iter()
        .map(|(_, value)| *value)
        .fold(f64::NEG_INFINITY, |a, b| a.max(b));

    let chart_view = { 
        // TODO: Do I really have to clone the data here?
        let chart = repr::Plot::new(data.to_vec())
            .legend(String::from("Compute Pass Duration (MS)"))
            .line_style(style::LineStyle::new().colour("#FF0000"));

        let chart_title = {
            let handlers::IntrsStats { name, .. } = stats;

            repr::Plot::new(Vec::with_capacity(0))
                .legend(format!("Handler: {name}"))
        };

        let chart_size = {
            let handlers::IntrsStats { size, .. } = stats;

            repr::Plot::new(Vec::with_capacity(0))
                .legend(format!("Size: {size} bytes"))
        };

        let chart_avg = {
            let chart_avg = avg
                .map(|avg| format!("Average: {avg}ms"))
                .unwrap_or(String::from(""));
            
            repr::Plot::new(Vec::with_capacity(0))
                .legend(chart_avg)
        };

        view::ContinuousView::new()
            .add(chart_title)
            .add(chart_size)
            .add(chart_avg)
            .add(chart)
            .x_range(0., data.len() as f64)
            .x_label("Frame")
            .y_range(data_min, data_max)
    };

    static FONT_DATABASE: sync::Lazy<usvg::fontdb::Database> = sync::Lazy::new(|| {
        let mut fonts = usvg::fontdb::Database::new();
        
        fonts.load_system_fonts();
        fonts
    });

    match page::Page::single(&chart_view).to_svg() {
        Ok(svg) => {
            let mut bytes: Vec<u8> = Vec::new();

            // Write SVG file contents into buffer
            svg::write(&mut bytes, &svg)?;

            // Reparse it from this buffer
            let tree = usvg::Tree::from_data(
                &bytes, 
                &usvg::Options::default(), 
                &FONT_DATABASE
            )?;

            // Grab the graph dimensions
            let temp = tree.size();

            let ( // Extract width and height from it
                width, 
                height
            ) = (temp.width() as u32, temp.height() as u32);

            // Construct a new Pixmap from these dimensions
            let mut pixels = tiny_skia::Pixmap::new(width, height)
                .unwrap();

            // Render into the Pixmap
            resvg::render(
                &tree, 
                tiny_skia::Transform::identity(), 
                &mut pixels.as_mut()
            );

            Ok(pixels)
        },
        Err(e) => anyhow::bail!(e),
    }
}