<script lang="ts">
	import Banner from '$lib/banner.svelte';
	import { flip } from 'svelte/animate';
	import { dndzone, type DndEvent } from 'svelte-dnd-action';
	import type { ResponseHandler } from '../../schema/messages/response';
	import { onMount } from 'svelte';
	import type { SendClientQueueInfoResponse } from '../../schema/messages/queueResponses';
	import type { MoveQueueItemMsg } from '../../schema/messages/queueMessages';

	export let handlers: ResponseHandler[];
	export let sendWsMsg: <T>(msg: [string, T]) => void;

	export let queue: string[];
	export let activeSource: string;

	let items: { name: string; id: number }[] = [];
	let currentIndex = 0;

	let setOldPosLocked = false;
	let oldPos: number | undefined;
	let newPos: number | undefined;

	onMount(() => {
		handlers.push([
			'SEND_CLIENT_QUEUE_INFO_RESPONSE',
			(data: SendClientQueueInfoResponse) => {
				currentIndex = data.info.current_head_index;
			}
		]);

		handlers = handlers;
	});

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
			let msg: MoveQueueItemMsg = [
				'MOVE_QUEUE_ITEM',
				{ sourceName: activeSource, oldPos, newPos }
			];

			sendWsMsg(msg);
		}
	};

	$: items = queue.map((item, index) => ({ name: item, id: index }));
</script>

<div class="flex-grow">
	<div class="w-full">
		<Banner text="Queue" />
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
				role="row"
				tabindex="0"
			>
				{item.name}
			</div>
		{/each}
	</section>
</div>
