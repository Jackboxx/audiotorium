// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { AudioMetaData } from "./AudioMetaData";
import type { AudioNodeHealth } from "./AudioNodeHealth";
import type { AudioStateInfo } from "./AudioStateInfo";
import type { DownloadInfo } from "./DownloadInfo";

export type NodeSessionWsResponse = { "SESSION_CONNECTED_RESPONSE": { QUEUE: Array<AudioMetaData> | null, HEALTH: AudioNodeHealth | null, DOWNLOADS: DownloadInfo | null, AUDIO_STATE_INFO: AudioStateInfo | null, } };