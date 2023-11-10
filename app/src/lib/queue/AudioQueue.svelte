<script lang="ts">
	import type { AudioMetaData } from '$api/AudioMetaData';
	import type { AudioNodeCommand } from '$api/AudioNodeCommand';
	import { sendCommandWithTimeout } from '$lib/utils';

	import { dndzone, type DndEvent } from 'svelte-dnd-action';

	export let nodeName: string;
	export let currentHeadIndex: number | undefined;
	export let queue: AudioMetaData[];

	type DndQueueItem = { data: AudioMetaData; id: number };

	let dndQueue: DndQueueItem[] = [];

	let deleteAllowed = true;

	let setOldPosLocked = false;
	let oldPos: number | undefined;
	let newPos: number | undefined;

	const onElementClick = async (index: number) => {
		const cmd: AudioNodeCommand = {
			PLAY_SELECTED: {
				index
			}
		};

		await fetch(`${import.meta.env.VITE_API_PREFIX}/commands/node/${nodeName}`, {
			method: 'POST',
			body: JSON.stringify(cmd),
			headers: { 'Content-Type': 'application/json' }
		});
	};

	const onRemove = async (index: number) => {
		if (!deleteAllowed) {
			return;
		}

		const cmd: AudioNodeCommand = {
			REMOVE_QUEUE_ITEM: {
				index
			}
		};

		deleteAllowed = false; // lock deletes since they are index based and could overlap
		await sendCommandWithTimeout(cmd, nodeName, 3000, () => {
			deleteAllowed = true;
		});

		deleteAllowed = true;
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

	const handleDndConsider = (e: CustomEvent<DndEvent<DndQueueItem>>) => {
		dndQueue = e.detail.items;
	};

	const handleDndFinalize = async (e: CustomEvent<DndEvent<DndQueueItem>>) => {
		setOldPosLocked = false;
		dndQueue = e.detail.items;

		if (oldPos !== undefined && newPos !== undefined) {
			const cmd: AudioNodeCommand = {
				MOVE_QUEUE_ITEM: {
					oldPos,
					newPos
				}
			};

			await sendCommandWithTimeout(cmd, nodeName, 500, () => undefined);
		}
	};

	$: dndQueue = queue.map((item, index) => ({ data: item, id: index }));
</script>

<section
	use:dndzone={{
		items: dndQueue,
		transformDraggedElement,
		dropTargetStyle: { background: '#0000000' }
	}}
	on:consider={handleDndConsider}
	on:finalize={handleDndFinalize}
	class="flex flex-grow flex-col gap-4"
>
	{#each dndQueue as item (item.id)}
		<div
			role="button"
			tabindex="0"
			on:keydown={undefined}
			on:dblclick={() => onElementClick(item.id)}
			class={`relative flex gap-2 rounded-sm ${
				currentHeadIndex === item.id ? 'bg-neutral-800' : ''
			}`}
		>
			<img
				class="h-[100px] min-h-[100px] w-[100px] min-w-[100px]"
				src={item.data.thumbnail_url}
				alt=""
			/>

			<div class="flex flex-1 flex-col gap-1 truncate">
				<span class="text-md sm:text-lg 2xl:text-xl">
					{item.data.name}
				</span>
				<span class="text-sm lg:text-lg">
					{item.data.author ?? '---'}
				</span>
			</div>

			<div
				role="button"
				tabindex="0"
				on:click={() => onRemove(item.id)}
				on:keydown={undefined}
				class="min-h-10 min-w-10 absolute right-0 top-2 z-10 h-10 w-10"
			>
				<img class="min-h-8 min-w-8 h-8 w-8" src="/cross.svg" alt="" />
			</div>
		</div>
	{/each}
</section>
