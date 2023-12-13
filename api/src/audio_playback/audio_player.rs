use std::sync::Arc;

use actix::Addr;
use anyhow::anyhow;
use cpal::{
    traits::{DeviceTrait, StreamTrait},
    Device, Stream, StreamConfig, StreamError,
};
use creek::{read::ReadError, ReadDiskStream, SymphoniaDecoder};
use rand::{seq::SliceRandom, thread_rng};
use rtrb::{Consumer, Producer, RingBuffer};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    commands::node_commands::AudioNodeCommand,
    message_send_handler::{ChangeDetector, MessageSendHandler, RateLimiter},
    node::{
        health::{AudioNodeHealth, AudioNodeHealthMild, AudioNodeHealthPoor},
        node_server::{AudioNode, SourceName},
        AudioProcessorToNodeMessage,
    },
    utils::setup_device,
};

use super::audio_item::{AudioDataLocator, AudioMetadata, AudioPlayerQueueItem};

type InternalQueue<ADL> = Vec<AudioPlayerQueueItem<ADL>>;

pub type SerializableQueue = Arc<[AudioMetadata]>;

pub struct AudioPlayer<ADL: AudioDataLocator> {
    source_name: SourceName,
    device: Device,
    config: StreamConfig,
    current_stream: Option<Stream>,
    queue: InternalQueue<ADL>,
    node_addr: Option<Addr<AudioNode>>,
    processor_msg_buffer: Option<Producer<AudioProcessorMessage>>,
    queue_head: usize,
    current_volume: f32,
}

struct AudioProcessor {
    msg_buffer: Consumer<AudioProcessorMessage>,
    read_disk_stream: Option<ReadDiskStream<SymphoniaDecoder>>,
    had_cache_miss_last_cycle: bool,
    info: ProcessorInfo,
    node_addr: Option<Addr<AudioNode>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../app/src/api-types/")]
pub struct AudioInfo {
    pub playback_state: PlaybackState,
    pub current_queue_index: usize,
    pub audio_progress: f64,
    pub audio_volume: f32,
}

impl Default for AudioInfo {
    fn default() -> Self {
        Self {
            audio_volume: 1.0,
            audio_progress: Default::default(),
            current_queue_index: Default::default(),
            playback_state: Default::default(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct PlaybackInfo {
    pub current_queue_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessorInfo {
    pub playback_state: PlaybackState,
    pub audio_progress: f64,
    pub audio_volume: f32,
}

#[derive(Debug)]
pub enum AudioStreamState {
    Playing,
    Buffering,
    Finished,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum PlaybackState {
    #[default]
    Playing,
    Paused,
}

#[derive(Debug, Clone)]
pub enum AudioProcessorMessage {
    SetVolume(f32),
    SetState(PlaybackState),
    SetProgress(f64),
    Addr(Option<Addr<AudioNode>>),
}

impl ProcessorInfo {
    pub fn new(volume: f32) -> Self {
        Self {
            audio_volume: volume,
            audio_progress: Default::default(),
            playback_state: Default::default(),
        }
    }
}

impl<ADL: AudioDataLocator + Clone> AudioPlayer<ADL> {
    pub fn try_new(
        source_name: SourceName,
        node_addr: Option<Addr<AudioNode>>,
        restored_state: AudioInfo,
        restored_queue: Vec<AudioPlayerQueueItem<ADL>>,
    ) -> anyhow::Result<Self> {
        let (device, config) = setup_device(&source_name)?;

        let mut player = Self {
            source_name,
            device,
            config,
            queue: restored_queue,
            current_stream: None,
            processor_msg_buffer: None,
            node_addr,
            current_volume: restored_state.audio_volume,
            queue_head: restored_state.current_queue_index,
        };

        player.restore_state(restored_state);

        Ok(player)
    }

    pub fn try_recover_device(&mut self, current_progress: f64) -> anyhow::Result<()> {
        let (device, config) = setup_device(&self.source_name)?;
        self.device = device;
        self.config = config;

        self.play_selected(self.queue_head, true)?;
        self.set_stream_progress(current_progress);

        Ok(())
    }

    pub fn play_next(&mut self) -> anyhow::Result<()> {
        if self.queue.is_empty() {
            self.current_stream = None;
            return Ok(());
        }

        self.update_queue_head(self.queue_head + 1);

        if self.queue_head >= self.queue.len() {
            self.update_queue_head(0);
        }

        if let Some(locator) = self.get_locator() {
            self.play(&locator)?;
        }

        Ok(())
    }

    pub fn play_prev(&mut self) -> anyhow::Result<()> {
        if self.queue.is_empty() {
            self.current_stream = None;
            return Ok(());
        }

        let prev_head = self
            .queue_head
            .checked_sub(1)
            .unwrap_or(self.queue.len() - 1);
        self.update_queue_head(prev_head);

        if let Some(locator) = self.get_locator() {
            self.play(&locator)?;
        }

        Ok(())
    }

    pub fn play_selected(&mut self, index: usize, allow_self_select: bool) -> anyhow::Result<()> {
        if self.queue.is_empty() {
            self.current_stream = None;
            return Ok(());
        }

        if index == self.queue_head && !allow_self_select {
            return Ok(());
        }

        let new_head_pos = index.clamp(0, self.queue.len() - 1);
        self.update_queue_head(new_head_pos);

        if let Some(locator) = self.get_locator() {
            self.play(&locator)?;
        }

        Ok(())
    }

    pub fn set_stream_playback_state(&mut self, state: PlaybackState) {
        if let Some(buffer) = self.processor_msg_buffer.as_mut() {
            let _ = buffer.push(AudioProcessorMessage::SetState(state));
        }
    }

    // progress is clamped between `0.0` and `1.0`
    pub fn set_stream_progress(&mut self, progress: f64) {
        let progress = progress.clamp(0.0, 1.0);
        if let Some(buffer) = self.processor_msg_buffer.as_mut() {
            let _ = buffer.push(AudioProcessorMessage::SetProgress(progress));
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.current_volume = volume;

        if let Some(buffer) = self.processor_msg_buffer.as_mut() {
            let _ = buffer.push(AudioProcessorMessage::SetVolume(volume));
        }
    }

    /// if this is the first song to be added to the queue starts playing immediately
    pub fn push_to_queue(&mut self, item: AudioPlayerQueueItem<ADL>) -> anyhow::Result<()> {
        if self.queue.is_empty() {
            self.play(&item.locator)?;
        }

        self.queue.push(item);
        Ok(())
    }

    pub fn remove_from_queue(&mut self, idx: usize) -> anyhow::Result<()> {
        if idx >= self.queue.len() {
            return Err(anyhow!("index out of bounds, can not remove item"));
        }

        self.queue.remove(idx);

        if self.queue.is_empty() {
            self.play_next() // play nothing
        } else if idx == self.queue_head {
            if self.queue_head > 0 {
                self.update_queue_head(self.queue_head - 1);
            } else {
                self.update_queue_head(self.queue.len() - 1);
            }

            self.play_next()
        } else if idx < self.queue_head {
            // keep playing current
            self.update_queue_head(self.queue_head - 1);
            Ok(())
        } else {
            Ok(()) // keep playing current
        }
    }

    pub fn shuffle_queue(&mut self) -> anyhow::Result<()> {
        self.queue.shuffle(&mut thread_rng());
        self.update_queue_head(0);
        self.play_selected(0, true)
    }

    // holy shit this should be unit tested
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

    pub fn queue(&self) -> &[AudioPlayerQueueItem<ADL>] {
        &self.queue
    }

    pub fn queue_head(&self) -> usize {
        self.queue_head
    }

    pub fn set_addr(&mut self, node_addr: Option<Addr<AudioNode>>) {
        self.node_addr = node_addr.clone();

        if let Some(buffer) = self.processor_msg_buffer.as_mut() {
            let _ = buffer.push(AudioProcessorMessage::Addr(node_addr));
        }
    }

    fn get_locator(&self) -> Option<ADL> {
        self.queue
            .get(self.queue_head)
            .map(|audio| audio.locator.clone())
    }

    fn update_queue_head(&mut self, value: usize) {
        self.queue_head = value;
    }

    fn restore_state(&mut self, info: AudioInfo) {
        self.queue_head = info.current_queue_index;

        if let Some(locator) = self.get_locator() {
            if let Err(err) = self.play(&locator) {
                log::error!("failed to play audio after restore\nERROR: {err}")
            }

            self.set_volume(info.audio_volume);
            self.set_stream_progress(info.audio_progress);
            self.set_stream_playback_state(info.playback_state);
        } else {
            self.queue_head = 0
        }
    }

    fn play(&mut self, locator: &ADL) -> anyhow::Result<()> {
        // prevent bluez-alsa from throwing error 'device busy' by removing the stream accessing
        // the bluetooth device before creating a new stream
        self.current_stream = None;

        let read_disk_stream = locator.load_audio_data()?;

        let (producer, consumer) = RingBuffer::<AudioProcessorMessage>::new(16);
        self.processor_msg_buffer = Some(producer);

        let mut processor = AudioProcessor::new(
            consumer,
            Some(read_disk_stream),
            self.node_addr.clone(),
            self.current_volume,
        );

        let mut msg_handler = MessageSendHandler::with_limiters(vec![
            Box::new(ChangeDetector::<AudioProcessorToNodeMessage>::new(Some(
                AudioProcessorToNodeMessage::Health(AudioNodeHealth::Good),
            ))),
            Box::<RateLimiter>::default(),
        ]);

        let mut msg_handler_for_err = MessageSendHandler::with_limiters(vec![
            Box::new(ChangeDetector::<AudioProcessorToNodeMessage>::new(Some(
                AudioProcessorToNodeMessage::Health(AudioNodeHealth::Good),
            ))),
            Box::<RateLimiter>::default(),
        ]);

        let addr_for_err = self.node_addr.clone();

        let new_stream = self.device.build_output_stream(
            &self.config,
            move |data: &mut [f32], _| match processor.try_process(data) {
                Ok(state) => match state {
                    AudioStreamState::Finished => {
                        processor.read_disk_stream = None;

                        if let Some(addr) = processor.node_addr.as_ref() {
                            if let Err(err) = addr.try_send(AudioNodeCommand::PlayNext) {
                                log::error!("failed to play next audio in queue, ERROR: {err}");
                            }
                        }
                    }
                    AudioStreamState::Buffering => {
                        let msg = AudioProcessorToNodeMessage::Health(AudioNodeHealth::Mild(
                            AudioNodeHealthMild::Buffering,
                        ));

                        if let Some(addr) = processor.node_addr.as_ref() {
                            msg_handler.send_msg(msg, addr);
                        }
                    }
                    AudioStreamState::Playing => {
                        let msg =
                            AudioProcessorToNodeMessage::AudioStateInfo(processor.info.clone());

                        if let Some(addr) = processor.node_addr.as_ref() {
                            msg_handler.send_msg(msg, addr);
                        }
                    }
                },
                Err(err) => {
                    log::error!("failed to process audio, ERROR: {err}");

                    let msg = AudioProcessorToNodeMessage::Health(AudioNodeHealth::Poor(
                        AudioNodeHealthPoor::AudioStreamReadFailed,
                    ));

                    if let Some(addr) = processor.node_addr.as_ref() {
                        msg_handler.send_msg(msg, addr);
                    }
                }
            },
            move |err| {
                log::error!("failed to process audio, ERROR: {err}");
                let msg = match err {
                    StreamError::DeviceNotAvailable => AudioProcessorToNodeMessage::Health(
                        AudioNodeHealth::Poor(AudioNodeHealthPoor::DeviceNotAvailable),
                    ),

                    StreamError::BackendSpecific { err } => {
                        AudioProcessorToNodeMessage::Health(AudioNodeHealth::Poor(
                            AudioNodeHealthPoor::AudioBackendError(err.description),
                        ))
                    }
                };

                if let Some(addr) = addr_for_err.as_ref() {
                    msg_handler_for_err.send_msg(msg, addr);
                }
            },
            None,
        )?;

        new_stream.play()?;
        self.current_stream = Some(new_stream);
        Ok(())
    }
}

impl AudioProcessor {
    fn new(
        msg_buffer: Consumer<AudioProcessorMessage>,
        read_disk_stream: Option<ReadDiskStream<SymphoniaDecoder>>,
        node_addr: Option<Addr<AudioNode>>,
        volume: f32,
    ) -> Self {
        Self {
            msg_buffer,
            read_disk_stream,
            node_addr,
            had_cache_miss_last_cycle: false,
            info: ProcessorInfo::new(volume),
        }
    }

    fn try_process(
        &mut self,
        mut data: &mut [f32],
    ) -> Result<AudioStreamState, ReadError<symphonia_core::errors::Error>> {
        let mut cache_missed_this_cycle = false;
        let mut stream_state = AudioStreamState::Playing;

        while let Ok(msg) = self.msg_buffer.pop() {
            match msg {
                AudioProcessorMessage::Addr(addr) => self.node_addr = addr,
                AudioProcessorMessage::SetVolume(volume) => self.info.audio_volume = volume,
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

            if !read_disk_stream.is_ready().unwrap_or(false) {
                stream_state = AudioStreamState::Buffering;
                cache_missed_this_cycle = true;
            }

            let num_frames = read_disk_stream.info().num_frames;
            let num_channels = usize::from(read_disk_stream.info().num_channels);

            let vol = self.info.audio_volume;

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
                            data[i * 2] = ch[i] * vol;
                            data[(i * 2) + 1] = ch[i] * vol;
                        }
                    } else if read_data.num_channels() == 2 {
                        let ch1 = read_data.read_channel(0);
                        let ch2 = read_data.read_channel(1);

                        for i in 0..to_end_of_loop {
                            data[i * 2] = ch1[i] * vol;
                            data[(i * 2) + 1] = ch2[i] * vol;
                        }
                    }

                    data = &mut data[to_end_of_loop * 2..];

                    stream_state = AudioStreamState::Finished;
                    break;
                } else {
                    if read_data.num_channels() == 1 {
                        let ch = read_data.read_channel(0);

                        for i in 0..read_data.num_frames() {
                            data[i * 2] = ch[i] * vol;
                            data[(i * 2) + 1] = ch[i] * vol;
                        }
                    } else if read_data.num_channels() == 2 {
                        let ch1 = read_data.read_channel(0);
                        let ch2 = read_data.read_channel(1);

                        for i in 0..read_data.num_frames() {
                            data[i * 2] = ch1[i] * vol;
                            data[(i * 2) + 1] = ch2[i] * vol;
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
