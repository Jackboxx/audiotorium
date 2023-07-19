<script lang="ts">
	export let onDblClick: () => void;
	export let onRemoveClick: () => void;

	export let title: string;
	export let duration: number;
</script>

<div
	class="relative m-2 h-[80px] rounded border-[2px] border-transparent bg-gradient-to-r
        from-neutral-700 to-neutral-800 p-2 shadow-md lg:h-[130px]"
	on:dblclick={onDblClick}
	on:keydown={undefined}
	role="button"
	tabindex="0"
>
	<div
		class="absolute right-2 top-2 flex h-[24px] w-[24px] items-center justify-center"
		on:click={onRemoveClick}
		on:keydown={undefined}
		role="button"
		tabindex="0"
	>
		<img class="h-[20px] w-[20px] invert" src="/cross.svg" alt="x" />
	</div>

	<div class="w-11/12">
		<div class="truncate text-lg lg:text-2xl">{title}</div>
		<div class="truncate text-sm lg:text-lg">{duration}min</div>
	</div>
</div>
