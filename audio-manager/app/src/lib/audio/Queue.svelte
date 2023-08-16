<script lang="ts">
	import Banner from '$lib/Banner.svelte';
	import { dndzone, type DndEvent } from 'svelte-dnd-action';
	import type { ResponseHandler } from '../../schema/messages/response';
	import { onMount } from 'svelte';
	import type {
		FinishedDownloadingResponse,
		SendClientQueueInfoResponse
	} from '../../schema/messages/queueResponses';
	import type {
		MoveQueueItemMsg,
		PauseQueueMsg,
		PlayNextMsg,
		PlayPreviousMsg,
		PlaySelectedMsg,
		RemoveQueueItemMsg,
		UnPauseQueueMsg
	} from '../../schema/messages/queueMessages';
	import QueueItem from './QueueItem.svelte';
	import QueueItemPlaying from './QueueItemPlaying.svelte';

	export let handlers: ResponseHandler[];
	export let sendWsMsg: <T>(msg: [string, T] | [string]) => void;

	export let queue: string[];

	let items: { name: string; id: number }[] = [];

	let currentIndex = 0;
	let progress = 0;

	let paused = false;

	let setOldPosLocked = false;
	let oldPos: number | undefined;
	let newPos: number | undefined;

	onMount(() => {
		handlers.push([
			'SEND_CLIENT_QUEUE_INFO_RESPONSE',
			(data: SendClientQueueInfoResponse) => {
				currentIndex = data.playbackInfo.currentHeadIndex;
				progress = data.processorInfo.audioProgress * 100;
				paused = data.processorInfo.playbackState === 'paused';
			}
		]);

		handlers.push([
			'FINISHED_DOWNLOADING_AUDIO',
			(data: FinishedDownloadingResponse) => {
				if (data.error) {
					console.error(data.error);
				} else if (data.queue) {
					queue = data.queue;
				}
			}
		]);

		handlers = handlers;
	});

	const next = () => {
		const msg: PlayNextMsg = ['PLAY_NEXT'];
		sendWsMsg(msg);
	};

	const previous = () => {
		const msg: PlayPreviousMsg = ['PLAY_PREVIOUS'];
		sendWsMsg(msg);
	};

	const select = (index: number) => {
		const msg: PlaySelectedMsg = ['PLAY_SELECTED', { index }];
		sendWsMsg(msg);
	};

    const remove = (index: number) => {
		const msg: RemoveQueueItemMsg = ['REMOVE_QUEUE_ITEM', { index }];
		sendWsMsg(msg);
    }

	const togglePause = () => {
		const msg: PauseQueueMsg | UnPauseQueueMsg = paused
			? ['UN_PAUSE_QUEUE']
			: ['PAUSE_QUEUE'];
		sendWsMsg(msg);
	};

	const transformDraggedElement = (
		_element: HTMLElement | undefined,
		_data: any,
		index: number | undefined
	) => {
		if (!setOldPosLocked) {
			setOldPosLocked = true;
			oldPos = index;
		}

		newPos = index;
	};

	const handleDndConsider = (
		e: CustomEvent<DndEvent<{ name: string; id: number }>>
	) => {
		items = e.detail.items;
	};
	const handleDndFinalize = (
		e: CustomEvent<DndEvent<{ name: string; id: number }>>
	) => {
		setOldPosLocked = false;
		items = e.detail.items;

		if (oldPos !== undefined && newPos !== undefined) {
			let msg: MoveQueueItemMsg = ['MOVE_QUEUE_ITEM', { oldPos, newPos }];

			sendWsMsg(msg);
		}
	};

	$: items = queue.map((item, index) => ({ name: item, id: index }));
</script>

<div class="lg:flex-grow lg:overflow-y-scroll">
	<div class="w-full">
		<Banner
			><div
				class="flex h-full items-center justify-between text-xl font-bold lg:text-3xl"
			>
				<div
					on:click={previous}
					on:keydown={undefined}
					tabindex="0"
					role="button"
				>
					<img
						src="/arrow-to-line-left.svg"
						class="h-6 invert lg:h-10"
						alt="previous"
					/>
				</div>
				<div
					on:click={togglePause}
					on:keydown={undefined}
					role="button"
					tabindex="0"
				>
					<img src="/media-play.svg" alt="pause" class="h-6 invert lg:h-10" />
				</div>
				<div on:click={next} on:keydown={undefined} role="button" tabindex="0">
					<img
						src="/arrow-to-line-right.svg"
						alt="next"
						class="h-6 invert lg:h-10"
					/>
				</div>
        </div>
        </Banner>
	</div>

	<section
		use:dndzone={{
			items,
			transformDraggedElement,
			dropTargetStyle: { background: '#57534e' }
		}}
		on:consider={handleDndConsider}
		on:finalize={handleDndFinalize}
	>
		{#each items as item (item.id)}
			{#if item.id === currentIndex}
				<QueueItemPlaying
					title={item.name}
					{progress}
					duration={5}
					onDblClick={undefined}
                    onRemoveClick={() => remove(item.id)}
					{sendWsMsg}
				/>
			{:else}
				<QueueItem
					title={item.name}
					duration={5}
                    onRemoveClick={() => remove(item.id)}
					onDblClick={() => select(item.id)}
				/>
			{/if}
		{/each}
	</section>
</div>
