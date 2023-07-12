<script lang="ts">
	export let onDblClick: () => void;
	export let title: string;
	export let duration: number;
	export let progress: number;

	function clampProgress(progress: number) {
		if (progress > 100) {
			return 100;
		} else if (progress < 0) {
			return 0;
		}

		return progress;
	}

	$: progress = clampProgress(progress);
</script>

<div
	class="m-2 h-[80px] rounded border-[2px] border-indigo-800 bg-gradient-to-r from-neutral-700
        to-neutral-800 p-2 shadow-md
        lg:h-[130px]"
	on:dblclick={onDblClick}
	on:keydown={undefined}
	role="button"
	tabindex="0"
>
	<div class="mb-2 h-[60%]">
		<div class="truncate text-lg lg:text-2xl">{title}</div>
		<div class="truncate text-sm lg:text-lg">{duration}min</div>
	</div>

	<div class="relative">
		<div class="absolute left-0 top-0 h-2 w-full rounded bg-indigo-950" />
		<div
			class=" absolute left-0 top-0 z-10 h-2 rounded bg-indigo-800"
			style={`width: ${progress ?? 0}%;`}
		/>
	</div>
</div>
