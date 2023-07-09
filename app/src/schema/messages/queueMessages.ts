export type SetActiveSourceMsg = ['SET_ACTIVE_SOURCE', SetActiveSourceParams];
export type SetActiveSourceParams = {
	sourceName: string;
};

export type AddQueueItemMsg = ['ADD_QUEUE_ITEM', AddQueueItemParams];
export type AddQueueItemParams = {
	url: string;
	title: string;
};

export type ReadQueueItemsMsg = ['READ_QUEUE_ITEMS'];

export type MoveQueueItemMsg = ['MOVE_QUEUE_ITEM', MoveQueueItemParams];
export type MoveQueueItemParams = {
	oldPos: number;
	newPos: number;
};

export type AddSourceMsg = ['ADD_SOURCE', AddSourceParams];
export type AddSourceParams = {
	sourceName: string;
};

export type PauseQueueMsg = ['PAUSE_QUEUE'];
export type UnPauseQueueMsg = ['UN_PAUSE_QUEUE'];
export type PlayNextMsg = ['PLAY_NEXT'];
export type PlayPreviousMsg = ['PLAY_PREVIOUS'];

export type PlaySelectedMsg = ['PLAY_SELECTED', PlaySelectedParams];
export type PlaySelectedParams = {
	index: number;
};

export type LoopQueueMsg = ['LOOP_QUEUE', LoopQueueParams];
export type LoopQueueParams = {
	bounds: {
		start: number;
		end: number;
	} | null;
};
