use std::path::PathBuf;

use actix::Addr;
use cpal::{
    traits::{DeviceTrait, StreamTrait},
    Device, Stream, StreamConfig,
};
use creek::{ReadDiskStream, SymphoniaDecoder};

use crate::{PlayNext, QueueServer};

#[derive(Default, Debug)]
pub enum AudioStreamState {
    Playing(PathBuf),
    Buffering,
    #[default]
    Finished,
    Error(anyhow::Error),
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum PlaybackState {
    #[default]
    Playing,
    Paused,
}

pub struct AudioPlayer {
    device: Device,
    config: StreamConfig,
    current_stream: Option<Stream>,
    queue: Vec<PathBuf>,
    server_addr: Addr<QueueServer>,
}

#[derive(Default)]
pub struct AudioProcessor {
    read_disk_stream: Option<ReadDiskStream<SymphoniaDecoder>>,
    playback_state: PlaybackState,
    stream_state: AudioStreamState,
    had_cache_miss_last_cycle: bool,
}

impl AudioPlayer {
    pub fn new(
        device: Device,
        config: StreamConfig,
        queue: Vec<PathBuf>,
        server_addr: Addr<QueueServer>,
    ) -> Self {
        Self {
            device,
            config,
            current_stream: None,
            queue,
            server_addr,
        }
    }

    pub fn play(&mut self, path: &PathBuf) -> anyhow::Result<()> {
        let read_disk_stream =
            ReadDiskStream::<SymphoniaDecoder>::new(path.clone(), 0, Default::default()).unwrap();

        let mut processor = AudioProcessor::default();
        processor.read_disk_stream = Some(read_disk_stream);

        let addr = self.server_addr.clone();
        let device_name = self.device.name().unwrap(); // this could not match the value passed
                                                       // through an external websocket update in
                                                       // the future

        let new_stream = self
            .device
            .build_output_stream(
                &self.config,
                move |data: &mut [f32], _| {
                    match processor.try_process(data) {
                        Err(err) => processor.stream_state = AudioStreamState::Error(err),
                        _ => {}
                    }

                    match processor.stream_state {
                        AudioStreamState::Finished => {
                            addr.do_send(PlayNext {
                                player_name: device_name.clone(),
                            });
                        }
                        // AudioStreamState::Error(err) => {
                        //     addr.send(todo!("err {err}"));
                        // }
                        _ => {}
                    }
                },
                move |err| todo!("log error: {err}"),
                None,
            )
            .unwrap();

        new_stream.play().unwrap();
        self.current_stream = Some(new_stream);
        Ok(())
    }

    pub fn push_to_queue(&mut self, path: PathBuf) {
        self.queue.push(path);
    }

    pub fn queue(&self) -> &[PathBuf] {
        &self.queue
    }

    /// removes the first element of the queue and returns it
    ///
    /// returns `None` if the queue is empty
    pub fn pop_first(&mut self) -> Option<PathBuf> {
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0))
        }
    }
}

impl AudioProcessor {
    pub fn try_process(&mut self, mut data: &mut [f32]) -> anyhow::Result<()> {
        let mut cache_missed_this_cycle = false;
        if let Some(read_disk_stream) = &mut self.read_disk_stream {
            if self.playback_state == PlaybackState::Paused {
                silence(data);
                return Ok(());
            }

            if !read_disk_stream.is_ready()? {
                self.stream_state = AudioStreamState::Buffering;
                cache_missed_this_cycle = true;
            }

            let num_frames = read_disk_stream.info().num_frames;
            let num_channels = usize::from(read_disk_stream.info().num_channels);

            while data.len() >= num_channels {
                let read_frames = data.len() / 2;
                let mut playhead = read_disk_stream.playhead();

                let read_data = read_disk_stream.read(read_frames)?;
                playhead += read_data.num_frames();

                if playhead >= num_frames {
                    let to_end_of_loop = read_data.num_frames() - (playhead - num_frames);

                    if read_data.num_channels() == 1 {
                        let ch = read_data.read_channel(0);

                        for i in 0..to_end_of_loop {
                            data[i * 2] = ch[i];
                            data[(i * 2) + 1] = ch[i];
                        }
                    } else if read_data.num_channels() == 2 {
                        let ch1 = read_data.read_channel(0);
                        let ch2 = read_data.read_channel(1);

                        for i in 0..to_end_of_loop {
                            data[i * 2] = ch1[i];
                            data[(i * 2) + 1] = ch2[i];
                        }
                    }

                    data = &mut data[to_end_of_loop * 2..];
                    self.stream_state = AudioStreamState::Finished;
                } else {
                    if read_data.num_channels() == 1 {
                        let ch = read_data.read_channel(0);

                        for i in 0..read_data.num_frames() {
                            data[i * 2] = ch[i];
                            data[(i * 2) + 1] = ch[i];
                        }
                    } else if read_data.num_channels() == 2 {
                        let ch1 = read_data.read_channel(0);
                        let ch2 = read_data.read_channel(1);

                        for i in 0..read_data.num_frames() {
                            data[i * 2] = ch1[i];
                            data[(i * 2) + 1] = ch2[i];
                        }
                    }

                    data = &mut data[read_data.num_frames() * 2..];
                }
            }
        } else {
            silence(data);
        }

        // When the cache misses, the buffer is filled with silence. So the next
        // buffer after the cache miss is starting from silence. To avoid an audible
        // pop, apply a ramping gain from 0 up to unity.
        if self.had_cache_miss_last_cycle {
            let buffer_size = data.len() as f32;
            for (i, sample) in data.iter_mut().enumerate() {
                *sample *= i as f32 / buffer_size;
            }
        }

        self.had_cache_miss_last_cycle = cache_missed_this_cycle;
        Ok(())
    }
}

fn silence(data: &mut [f32]) {
    for sample in data.iter_mut() {
        *sample = 0.0;
    }
}
