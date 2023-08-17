use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use actix::Addr;
use anyhow::anyhow;
use cpal::{
    traits::{DeviceTrait, StreamTrait},
    Device, Stream, StreamConfig,
};
use creek::{ReadDiskStream, SymphoniaDecoder};
use log::error;
use rtrb::{Consumer, Producer, RingBuffer};
use serde::{Deserialize, Serialize};

use crate::server::{AudioBrain, LoopBounds, PlayNextServerParams, SendClientQueueInfoParams};

#[derive(Debug)]
pub enum AudioStreamState {
    Playing,
    Buffering,
    Finished,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackInfo {
    current_head_index: usize,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorInfo {
    playback_state: PlaybackState,
    audio_progress: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackState {
    #[default]
    Playing,
    Paused,
}

#[derive(Debug, Clone)]
pub enum AudioProcessorMessage {
    SetState(PlaybackState),
    SetProgress(f64),
}

pub struct AudioSource {
    device: Device,
    config: StreamConfig,
    current_stream: Option<Stream>,
    queue: Vec<PathBuf>,
    server_addr: Addr<AudioBrain>,
    processor_msg_buffer: Option<Producer<AudioProcessorMessage>>,
    queue_head: usize,
    loop_start_end: Option<(usize, usize)>,
    playback_info: PlaybackInfo,
}

pub struct AudioProcessor {
    msg_buffer: Consumer<AudioProcessorMessage>,
    read_disk_stream: Option<ReadDiskStream<SymphoniaDecoder>>,
    had_cache_miss_last_cycle: bool,
    last_msg_sent_at: Instant,
    hard_rate_limit: Duration,
    info: ProcessorInfo,
}

impl AudioProcessor {
    fn new(
        msg_buffer: Consumer<AudioProcessorMessage>,
        read_disk_stream: Option<ReadDiskStream<SymphoniaDecoder>>,
    ) -> Self {
        Self {
            msg_buffer,
            read_disk_stream,
            had_cache_miss_last_cycle: false,
            last_msg_sent_at: Instant::now(),
            hard_rate_limit: Duration::from_millis(33),
            info: ProcessorInfo::default(),
        }
    }
}

impl AudioSource {
    pub fn new(
        device: Device,
        config: StreamConfig,
        queue: Vec<PathBuf>,
        server_addr: Addr<AudioBrain>,
    ) -> Self {
        Self {
            device,
            config,
            current_stream: None,
            processor_msg_buffer: None,
            queue,
            server_addr,
            queue_head: 0,
            playback_info: PlaybackInfo {
                current_head_index: 0,
            },
            loop_start_end: None,
        }
    }

    pub fn play_next(&mut self, source_name: String) -> anyhow::Result<()> {
        if self.queue.is_empty() {
            self.current_stream = None;
            return Ok(());
        }

        self.update_queue_head(self.queue_head + 1);

        if let Some((start, end)) = self.loop_start_end {
            if self.queue_head > end {
                self.update_queue_head(start);
            }
        } else if self.queue_head >= self.queue.len() {
            self.update_queue_head(0);
        }

        if let Some(path) = self.get_path() {
            self.play(&path, source_name)?;
        }

        Ok(())
    }

    pub fn play_prev(&mut self, source_name: String) -> anyhow::Result<()> {
        if self.queue.is_empty() {
            self.current_stream = None;
            return Ok(());
        }

        // casting could technically be a problem if we have very large queues like 2^32
        // but in all realistic situations this should be fine
        //
        // !!! CHECK THIS IF YOU ARE HAVING STRANGE ERRORS IN EXTREME CASES !!!
        let mut fake_queue_head = self.queue_head as isize - 1;

        if let Some((start, end)) = self.loop_start_end {
            if fake_queue_head > end as isize {
                fake_queue_head = start as isize;
            } else if fake_queue_head < start as isize {
                fake_queue_head = end as isize;
            }
        } else if fake_queue_head < 0 {
            fake_queue_head = self.queue.len() as isize - 1;
        }

        self.update_queue_head(fake_queue_head as usize);

        if let Some(path) = self.get_path() {
            self.play(&path, source_name)?;
        }

        Ok(())
    }

    pub fn play_selected(&mut self, index: usize, source_name: String) -> anyhow::Result<()> {
        if self.queue.is_empty() {
            self.current_stream = None;
            return Ok(());
        }

        if index == self.queue_head {
            return Ok(());
        }

        let new_head_pos = if let Some((start, end)) = self.loop_start_end {
            index.clamp(start, end)
        } else {
            index.clamp(0, self.queue.len() - 1)
        };

        self.update_queue_head(new_head_pos);

        if let Some(path) = self.get_path() {
            self.play(&path, source_name)?;
        }

        Ok(())
    }

    pub fn set_stream_playback_state(&mut self, state: PlaybackState) {
        if let Some(buffer) = self.processor_msg_buffer.as_mut() {
            buffer
                .push(AudioProcessorMessage::SetState(state))
                .unwrap_or(());
        }
    }

    // progress is clamped between `0.0` and `1.0`
    pub fn set_stream_progress(&mut self, progress: f64) {
        let progress = progress.clamp(0.0, 1.0);
        if let Some(buffer) = self.processor_msg_buffer.as_mut() {
            buffer
                .push(AudioProcessorMessage::SetProgress(progress))
                .unwrap_or(());
        }
    }

    /// sets the loop `start` and `end`.
    ///
    /// values are clamped between `0` and `queue.len()`.
    pub fn set_loop(&mut self, loop_start_end: Option<LoopBounds>) {
        self.loop_start_end = loop_start_end.map(|LoopBounds { start, end }| {
            (
                start.clamp(0, self.queue.len()),
                end.clamp(0, self.queue.len()),
            )
        });
    }

    pub fn get_path(&self) -> Option<PathBuf> {
        self.queue
            .get(self.queue_head)
            .map(|audio| audio.to_owned())
    }

    /// if this is the first song to be added to the queue starts playing immediately
    pub fn push_to_queue(&mut self, path: PathBuf, source_name: String) -> anyhow::Result<()> {
        if self.queue.is_empty() {
            self.play(&path, source_name)?;
        }

        self.queue.push(path);
        Ok(())
    }

    pub fn remove_from_queue(&mut self, idx: usize, source_name: String) -> anyhow::Result<()> {
        if idx >= self.queue.len() {
            return Err(anyhow!("index out of bounds, can not remove item"));
        }

        self.queue.remove(idx);

        if self.queue.is_empty() {
            self.play_next(source_name) // play nothing
        } else if idx == self.queue_head {
            if self.queue_head > 0 {
                self.update_queue_head(self.queue_head - 1);
            } else {
                self.update_queue_head(self.queue.len() - 1);
            }

            self.play_next(source_name)
        } else if idx < self.queue_head {
            // keep playing current
            self.update_queue_head(self.queue_head - 1);
            Ok(())
        } else {
            Ok(()) // keep playing current
        }
    }

    pub fn move_queue_item(&mut self, old: usize, new: usize) {
        if old == new {
            return;
        }

        if old > new {
            for i in (new + 1..=old).rev() {
                if self.queue_head == i - 1 {
                    self.update_queue_head(i);
                } else if self.queue_head == i {
                    self.update_queue_head(i - 1);
                }

                self.queue.swap(i - 1, i);
            }
        } else {
            for i in old..new {
                if self.queue_head == i {
                    self.update_queue_head(i + 1);
                } else if self.queue_head == i + 1 {
                    self.update_queue_head(i);
                }

                self.queue.swap(i, i + 1);
            }
        }
    }

    /// updates `queue_head` and `playback_info`
    pub fn update_queue_head(&mut self, value: usize) {
        self.playback_info.current_head_index = value;
        self.queue_head = value;
    }

    pub fn queue(&self) -> &[PathBuf] {
        &self.queue
    }

    pub fn playback_info(&self) -> &PlaybackInfo {
        &self.playback_info
    }

    fn play(&mut self, path: &Path, source_name: String) -> anyhow::Result<()> {
        let read_disk_stream =
            ReadDiskStream::<SymphoniaDecoder>::new(path, 0, Default::default())?;

        let (producer, consumer) = RingBuffer::<AudioProcessorMessage>::new(1);
        self.processor_msg_buffer = Some(producer);

        let mut processor = AudioProcessor::new(consumer, Some(read_disk_stream));

        let addr = self.server_addr.clone();
        let new_stream = self.device.build_output_stream(
            &self.config,
            move |data: &mut [f32], _| match processor.try_process(data) {
                Ok(state) => match state {
                    AudioStreamState::Finished => {
                        processor.read_disk_stream = None;

                        if let Err(err) = addr.try_send(PlayNextServerParams {
                            source_name: source_name.clone(),
                        }) {
                            error!("failed to play next audio in queue, ERROR: {err}");
                        }
                    }
                    AudioStreamState::Buffering => {}
                    AudioStreamState::Playing => {
                        // prevent message spam from filling up mailbox of the server
                        if Instant::now().duration_since(processor.last_msg_sent_at)
                            > processor.hard_rate_limit
                        {
                            processor.last_msg_sent_at = Instant::now();
                            addr.do_send(SendClientQueueInfoParams {
                                source_name: source_name.clone(),
                                processor_info: processor.info.clone(),
                            });
                        }
                    }
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
}

impl AudioProcessor {
    pub fn try_process(&mut self, mut data: &mut [f32]) -> anyhow::Result<AudioStreamState> {
        let mut cache_missed_this_cycle = false;
        let mut stream_state = AudioStreamState::Playing;

        while let Ok(msg) = self.msg_buffer.pop() {
            match msg {
                AudioProcessorMessage::SetState(state) => self.info.playback_state = state,
                AudioProcessorMessage::SetProgress(percentage) => {
                    if let Some(read_disk_stream) = &mut self.read_disk_stream {
                        let num_frames = read_disk_stream.info().num_frames;
                        let seek_frame = (num_frames as f64 * percentage) as usize;
                        if let Ok(cache_found) =
                            read_disk_stream.seek(seek_frame, creek::SeekMode::Auto)
                        {
                            if !cache_found {
                                stream_state = AudioStreamState::Buffering;
                            }
                        }
                    }
                }
            }
        }

        if let Some(read_disk_stream) = &mut self.read_disk_stream {
            if self.info.playback_state == PlaybackState::Paused {
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

                self.info.audio_progress = playhead as f64 / num_frames as f64;
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
