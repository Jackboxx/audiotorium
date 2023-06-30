<script lang="ts">
	import Banner from '$lib/banner.svelte';
	import YouTubeSearch from '$lib/search/YouTubeSearch.svelte';
	import { getVideoUrl } from '../../schema/video';

	const queue = [
		'hi',
		'rick',
		'foo',
		'bar',
		'baz',
		'a',
		'b',
		'c',
		'fuck',
		'more',
		'a',
		'b',
		'c'
	];
</script>

<div
	class="grid h-screen w-full grid-rows-5
    bg-gray-100 dark:bg-zinc-900 lg:grid-cols-2 lg:grid-rows-1"
>
	<YouTubeSearch
		onSearchResultClick={(video) => {
			console.log(getVideoUrl(video));
		}}
	/>

	<div
		class="row-span-2 flex flex-col items-center gap-2 lg:row-span-1 lg:border-l-[1px]
        lg:border-l-black lg:dark:border-l-neutral-300"
	>
		<div class="w-full">
			<Banner text="Playing" />
		</div>
		<div class="text-lg">{queue.at(0)}</div>

		<div class="w-full">
			<Banner text="Next" />
		</div>
		<div class="flex w-full flex-col gap-1 overflow-y-scroll">
			{#each queue.splice(1) as item}
				<div class="text-lg">{item}</div>
			{/each}
		</div>
	</div>
</div>
