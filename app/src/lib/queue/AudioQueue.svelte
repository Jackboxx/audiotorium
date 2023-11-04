<script lang="ts">
	import type { AudioMetaData } from '$api/AudioMetaData';
	import type { AudioNodeCommand } from '$api/AudioNodeCommand';
	import { API_PREFIX, sendCommandWithTimeout } from '$lib/utils';

	export let nodeName: string;
	export let currentHeadIndex: number | undefined;
	export let queue: AudioMetaData[];

	let deleteAllowed = true;

	const onElementClick = async (index: number) => {
		const cmd: AudioNodeCommand = {
			PLAY_SELECTED: {
				index
			}
		};

		await fetch(`${API_PREFIX}/commands/node/${nodeName}`, {
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
</script>

<div class="flex flex-grow flex-col gap-4">
	{#each queue as audioDataEntry, index}
		<div
			role="button"
			tabindex="0"
			on:keydown={undefined}
			on:dblclick={() => onElementClick(index)}
			class={`relative flex gap-2 rounded-sm ${
				currentHeadIndex === index ? 'bg-neutral-800' : ''
			}`}
		>
			<img
				class="h-[100px] min-h-[100px] w-[100px] min-w-[100px]"
				src={audioDataEntry.thumbnail_url}
				alt=""
			/>

			<div class="flex flex-1 flex-col gap-1 truncate">
				<span class="text-md sm:text-lg 2xl:text-xl">
					{audioDataEntry.name}
				</span>
				<span class="text-sm lg:text-lg">
					{audioDataEntry.author ?? '---'}
				</span>
			</div>

			<div
				role="button"
				tabindex="0"
				on:click={() => onRemove(index)}
				on:keydown={undefined}
				class="min-h-10 min-w-10 absolute right-0 top-2 z-10 h-10 w-10"
			>
				<img class="min-h-8 min-w-8 h-8 w-8" src="/cross.svg" alt="" />
			</div>
		</div>
	{/each}
</div>
