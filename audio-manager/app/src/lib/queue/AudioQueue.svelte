<script lang="ts">
	import type { AudioMetaData } from '$api/AudioMetaData';
	import type { AudioNodeCommand } from '$api/AudioNodeCommand';
	import { API_PREFIX } from '$lib/utils';

	export let nodeName: string;
	export let currentHeadIndex: number | undefined;
	export let queue: AudioMetaData[];

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
</script>

<div class="flex flex-grow flex-col gap-4">
	{#each queue as audioDataEntry, index}
		<div
			role="button"
			tabindex="0"
			on:keydown={undefined}
			class={`flex gap-2 truncate ${
				currentHeadIndex === index ? 'bg-neutral-700' : ''
			}`}
			on:click={() => onElementClick(index)}
		>
			<img
				class="h-[100px] min-h-[100px] w-[100px] min-w-[100px]"
				src={audioDataEntry.thumbnail_url}
				alt=""
			/>
			<div class="flex flex-col gap-1">
				<span class="text-md sm:text-lg 2xl:text-xl">
					{audioDataEntry.name}
				</span>
				<span class="text-sm lg:text-lg">
					{audioDataEntry.author}
				</span>
			</div>
		</div>
	{/each}
</div>
