export type SetActiveSourceResponse = {
	sourceName: string;
};

export type AddQueueItemResponse = {
	queue: string[];
};

export type ReadQueueItemsResponse = {
	queue: string[];
};

export type MoveQueueItemResponse = {
	queue: string[];
};

export type AddSourceResponse = {
	sources: string[];
};

export type SendClientQueueInfoResponse = {
	isDownloadingAudio: boolean;
	playbackInfo: { currentHeadIndex: number };
	processorInfo: {
		playbackState: string;
		audioProgress: number;
	};
};

export type FinishedDownloadingResponse = {
	error?: string;
	queue?: string[];
};

export type SessionConnectedResponse = {
	id: number;
	sources: string[];
};
