export type AddQueueItemResponse = {
	queue: string[];
};

export type ReadQueueItemsResponse = {
	queue: string[];
};

export type AddSourceResponse = {
	sources: string[];
};

export type SendClientQueueInfoResponse = {
	info: { current_head_index: number };
};

export type SessionConnectedResponse = {
	id: number;
	sources: string[];
};
