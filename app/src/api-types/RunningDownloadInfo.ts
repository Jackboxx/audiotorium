// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { DownloadInfo } from "./DownloadInfo";
import type { ErrorResponse } from "./ErrorResponse";

export interface RunningDownloadInfo { active: Array<DownloadInfo>, failed: Array<[DownloadInfo, ErrorResponse]>, }