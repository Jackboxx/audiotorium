<script lang="ts">
	import SearchBar from '../../lib/SearchBar.svelte';
	import {
		YouTubeSearchResultSchema,
		getVideoUrl,
		searchForVideos,
		type YouTubeVideo,
		getBestThumbNail
	} from '../../schema/video';

	let searchText = '';
	let searchResults: YouTubeVideo[] = [];

	const onConfirm = async () => {
		const res = await searchForVideos(searchText);

		try {
			searchResults = YouTubeSearchResultSchema.parse(await res.json()).items;
		} catch (err) {
			console.error(err);
		}
	};
</script>

<div
	class="flex min-h-screen w-full flex-col items-center bg-gray-100 px-2 dark:bg-zinc-900"
>
	<SearchBar bind:searchText {onConfirm} />
	{#each searchResults as video}
		<div class="my-4 grid aspect-video w-full max-w-[800px] grid-cols-2 gap-4">
			<div>
				<a href={getVideoUrl(video)} class="h-full w-full">
					<img
						class="h-full w-full rounded object-cover"
						src={getBestThumbNail(video.snippet.thumbnails).url}
						alt={video.snippet.title}
					/>
				</a>
			</div>
			<div class="flex flex-col gap-2">
				<span class="text-lg lg:text-2xl">
					{video.snippet.title}
				</span>

				<span class="text-md lg:text-lg">
					{video.snippet.description}
				</span>
			</div>
		</div>
	{/each}
</div>
