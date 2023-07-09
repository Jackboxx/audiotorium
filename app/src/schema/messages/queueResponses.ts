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
	playbackInfo: { currentHeadIndex: number };
	playbackState: string;
};

export type SessionConnectedResponse = {
	id: number;
	sources: string[];
};
