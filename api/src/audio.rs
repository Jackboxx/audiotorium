use std::path::PathBuf;

use actix::Addr;
use cpal::{
    traits::{DeviceTrait, StreamTrait},
    Device, Stream, StreamConfig,
};
use creek::{ReadDiskStream, SymphoniaDecoder};
use log::error;

use crate::{PlayNextParams, QueueServer};

#[derive(Default, Debug)]
pub enum AudioStreamState {
    #[default]
    Playing,
    Buffering,
    Finished,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum PlaybackState {
    #[default]
    Playing,
    Paused,
}

pub struct AudioSource {
    device: Device,
    config: StreamConfig,
    current_stream: Option<Stream>,
    queue: Vec<PathBuf>,
    server_addr: Addr<QueueServer>,
    queue_head: usize,
    loop_start_end: Option<(usize, usize)>,
}

#[derive(Default)]
pub struct AudioProcessor {
    read_disk_stream: Option<ReadDiskStream<SymphoniaDecoder>>,
    playback_state: PlaybackState,
    had_cache_miss_last_cycle: bool,
}

impl AudioSource {
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
            queue_head: 0,
            loop_start_end: None,
        }
    }

    pub fn play_next(&mut self, source_name: String) -> anyhow::Result<()> {
        if let Some((start, end)) = self.loop_start_end {
            if self.queue_head > end {
                self.queue_head = start;
            }
        } else if self.queue_head >= self.queue.len() {
            self.queue_head = 0;
        }

        if let Some(audio) = self.current_track() {
            self.play(&audio, source_name)?;
        }

        Ok(())
    }

    fn play(&mut self, path: &PathBuf, source_name: String) -> anyhow::Result<()> {
        let read_disk_stream =
            ReadDiskStream::<SymphoniaDecoder>::new(path.clone(), 0, Default::default())?;

        let mut processor = AudioProcessor::default();
        processor.read_disk_stream = Some(read_disk_stream);

        let addr = self.server_addr.clone();
        let new_stream = self.device.build_output_stream(
            &self.config,
            move |data: &mut [f32], _| match processor.try_process(data) {
                Ok(state) => match state {
                    AudioStreamState::Finished => {
                        processor.read_disk_stream = None;

                        if let Err(err) = addr.try_send(PlayNextParams {
                            source_name: source_name.clone(),
                        }) {
                            error!("failed to play next audio in queue, ERROR: {err}");
                        }
                    }
                    _ => {}
                },
                Err(err) => error!("failed to process audio, ERROR: {err}"),
            },
            move |err| error!("failed to process audio, ERROR: {err}"),
            None,
        )?;

        new_stream.play()?;
        self.current_stream = Some(new_stream);
        Ok(())
    }

    /// sets the loop `start` and `end`.
    ///
    /// values are clamped between `0` and `queue.len()`.
    pub fn set_loop(&mut self, loop_start_end: Option<(usize, usize)>) {
        self.loop_start_end = loop_start_end
            .map(|(a, b)| (a.clamp(0, self.queue.len()), b.clamp(0, self.queue.len())));
    }

    /// gets the audio at the current `queue_head` and increments the `queue_head` by 1.
    pub fn current_track(&mut self) -> Option<PathBuf> {
        let audio = self
            .queue
            .get(self.queue_head)
            .map(|audio| audio.to_owned());

        self.queue_head += 1;
        audio
    }

    pub fn push_to_queue(&mut self, path: PathBuf) {
        self.queue.push(path);
    }

    pub fn queue(&self) -> &[PathBuf] {
        &self.queue
    }
}

impl AudioProcessor {
    pub fn try_process(&mut self, mut data: &mut [f32]) -> anyhow::Result<AudioStreamState> {
        let mut cache_missed_this_cycle = false;
        let mut stream_state = AudioStreamState::Playing;
        if let Some(read_disk_stream) = &mut self.read_disk_stream {
            if self.playback_state == PlaybackState::Paused {
                silence(data);
                return Ok(AudioStreamState::Playing);
            }

            if !read_disk_stream.is_ready()? {
                stream_state = AudioStreamState::Buffering;
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
                    stream_state = AudioStreamState::Finished;
                    break;
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
                    stream_state = AudioStreamState::Playing;
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
        Ok(stream_state)
    }
}

fn silence(data: &mut [f32]) {
    for sample in data.iter_mut() {
        *sample = 0.0;
    }
}
