export type AddQueueItemMsg = ['ADD_QUEUE_ITEM', AddQueueItemParams];
export type AddQueueItemParams = {
	sourceName: string;
	url: string;
	title: string;
};

export type AddSourceMsg = ['ADD_SOURCE', AddSourceParams];
export type AddSourceParams = {
	sourceName: string;
};

export type PlayNextMsg = ['PLAY_NEXT', PlayNextParams];
export type PlayNextParams = {
	sourceName: string;
};

export type PlayPreviousMsg = ['PLAY_PREVIOUS', PlayPreviousParams];
export type PlayPreviousParams = {
	sourceName: string;
};

export type PlaySelectedMsg = ['PLAY_SELECTED', PlaySelectedParams];
export type PlaySelectedParams = {
	sourceName: string;
	index: number;
};

export type LoopQueueMsg = ['LOOP_QUEUE', LoopQueueParams];
export type LoopQueueParams = {
	sourceName: string;
	bounds: {
		start: number;
		end: number;
	} | null;
};
