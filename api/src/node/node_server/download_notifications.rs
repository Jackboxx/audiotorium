use crate::{
    audio_playback::audio_item::AudioPlayerQueueItem,
    downloader::{actor::NotifyDownloadUpdate, info::DownloadInfo},
    streams::node_streams::{AudioNodeInfoStreamMessage, RunningDownloadInfo},
};

use actix::Handler;

use super::{extract_queue_metadata, AudioNode};

impl Handler<NotifyDownloadUpdate> for AudioNode {
    type Result = ();

    fn handle(&mut self, msg: NotifyDownloadUpdate, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            NotifyDownloadUpdate::Queued(info) => {
                self.active_downloads.insert(info);

                let msg = AudioNodeInfoStreamMessage::Download(RunningDownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                });

                self.multicast(msg);
            }
            NotifyDownloadUpdate::FailedToQueue((info, err_resp)) => {
                self.failed_downloads.insert(info, err_resp);

                let msg = AudioNodeInfoStreamMessage::Download(RunningDownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                });

                self.multicast(msg);
            }
            NotifyDownloadUpdate::SingleFinished(Ok((info, metadata, path))) => {
                self.active_downloads.remove(&info);
                self.failed_downloads.remove(&info);

                let item = AudioPlayerQueueItem {
                    metadata,
                    locator: path,
                };

                if let Err(err) = self.player.push_to_queue(item) {
                    log::error!("failed to auto play first song, ERROR: {err}");
                    return;
                };

                let download_fin_msg = AudioNodeInfoStreamMessage::Download(RunningDownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                });
                self.multicast(download_fin_msg);

                let updated_queue_msg =
                    AudioNodeInfoStreamMessage::Queue(extract_queue_metadata(self.player.queue()));
                self.multicast(updated_queue_msg);
            }
            NotifyDownloadUpdate::SingleFinished(Err((info, err_resp))) => {
                self.active_downloads.remove(&info);
                self.failed_downloads.insert(info, err_resp);

                let msg = AudioNodeInfoStreamMessage::Download(RunningDownloadInfo {
                    active: self.active_downloads.clone().into_iter().collect(),
                    failed: self.failed_downloads.clone().into_iter().collect(),
                });

                self.multicast(msg);
            }
            NotifyDownloadUpdate::BatchUpdated { batch } => match batch {
                DownloadInfo::YoutubePlaylist { ref video_urls, .. } => {
                    if video_urls.is_empty() {
                        self.active_downloads.remove(&batch);
                    } else {
                        self.active_downloads.replace(batch);
                    };

                    let msg = AudioNodeInfoStreamMessage::Download(RunningDownloadInfo {
                        active: self.active_downloads.clone().into_iter().collect(),
                        failed: self.failed_downloads.clone().into_iter().collect(),
                    });

                    self.multicast(msg);
                }
                _ => {
                    log::warn!("received a batch updated that wasn't a valid batch, valid batches are [youtube-playlist]");
                }
            },
        }
    }
}
