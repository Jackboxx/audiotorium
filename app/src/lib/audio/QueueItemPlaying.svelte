<script lang="ts">
	import type { SetAudioProgressMsg } from '../../schema/messages/queueMessages';

	export let sendWsMsg: <T>(msg: [string, T] | [string]) => void;
	export let onDblClick: (() => void) | undefined;
	export let onRemoveClick: (() => void) | undefined;

	export let title: string;
	export let duration: number;
	export let progress: number;

	let progressBarWidth = 0;

	function clampProgress(progress: number) {
		if (progress > 100) {
			return 100;
		} else if (progress < 0) {
			return 0;
		}

		return progress;
	}

	function onProgressBarClick(e: MouseEvent) {
		let progress = e.offsetX / progressBarWidth;
		let msg: SetAudioProgressMsg = ['SET_AUDIO_PROGRESS', { progress }];

		sendWsMsg(msg);
	}

	$: progress = clampProgress(progress);
</script>

<div
	class="m-2 h-[80px] rounded border-[2px] border-indigo-800 bg-gradient-to-r
        from-neutral-700 to-neutral-800 p-2 shadow-md lg:h-[130px]"
	on:dblclick={onDblClick}
	on:keydown={undefined}
	role="button"
	tabindex="0"
>
	<div class="mb-2 h-[60%]">
		<div class="truncate text-lg lg:text-2xl">{title}</div>
		<div class="truncate text-sm lg:text-lg">{duration}min</div>
	</div>

	<div class="relative cursor-pointer">
		<div
			bind:clientWidth={progressBarWidth}
			on:click={onProgressBarClick}
			role="none"
			class="absolute left-0 top-0 h-2 w-full rounded bg-indigo-950"
		/>
		<div
			on:click={onProgressBarClick}
			class=" absolute left-0 top-0 z-10 h-2 rounded bg-indigo-800"
			role="none"
			style={`width: ${progress ?? 0}%;`}
		/>
	</div>
</div>
