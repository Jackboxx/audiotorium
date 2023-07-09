<script lang="ts">
	import Banner from '$lib/banner.svelte';
	import { dndzone, type DndEvent } from 'svelte-dnd-action';
	import type { ResponseHandler } from '../../schema/messages/response';
	import { onMount } from 'svelte';
	import type { SendClientQueueInfoResponse } from '../../schema/messages/queueResponses';
	import type {
		MoveQueueItemMsg,
		PauseQueueMsg,
		PlayNextMsg,
		PlayPreviousMsg,
		PlaySelectedMsg,
		UnPauseQueueMsg
	} from '../../schema/messages/queueMessages';

	export let handlers: ResponseHandler[];
	export let sendWsMsg: <T>(msg: [string, T] | [string]) => void;

	export let queue: string[];

	let items: { name: string; id: number }[] = [];
	let currentIndex = 0;
	let paused = false;

	let setOldPosLocked = false;
	let oldPos: number | undefined;
	let newPos: number | undefined;

	onMount(() => {
		handlers.push([
			'SEND_CLIENT_QUEUE_INFO_RESPONSE',
			(data: SendClientQueueInfoResponse) => {
				console.log(data);
				currentIndex = data.playbackInfo.currentHeadIndex;
				paused = data.playbackState === 'paused';
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

<div class="flex-grow">
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
						class="h-6 dark:invert lg:h-10"
						alt="previous"
					/>
				</div>
				<div
					on:click={togglePause}
					on:keydown={undefined}
					role="button"
					tabindex="0"
				>
					<img
						src="/media-play.svg"
						alt="pause"
						class="h-6 dark:invert lg:h-10"
					/>
				</div>
				<div on:click={next} on:keydown={undefined} role="button" tabindex="0">
					<img
						src="/arrow-to-line-right.svg"
						alt="next"
						class="h-6 dark:invert lg:h-10"
					/>
				</div>
			</div></Banner
		>
	</div>

	<section
		class="w-full overflow-y-scroll"
		use:dndzone={{ items, transformDraggedElement }}
		on:consider={handleDndConsider}
		on:finalize={handleDndFinalize}
	>
		{#each items as item (item.id)}
			<div
				class={`m-2 h-1/6 truncate rounded-lg p-2 ${
					currentIndex == item.id
						? 'bg-sky-400 dark:bg-indigo-500'
						: 'bg-sky-200 dark:bg-indigo-300'
				} text-center align-middle text-lg
                    lg:text-2xl`}
				on:dblclick={() => select(item.id)}
				on:keydown={undefined}
				role="button"
				tabindex="0"
			>
				{item.name}
			</div>
		{/each}
	</section>
</div>
