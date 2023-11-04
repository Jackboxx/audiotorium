<script lang="ts">
	import type { AudioMetaData } from '$api/AudioMetaData';
	import type { ProcessorInfo } from '$api/ProcessorInfo';

	export let activeAudio: AudioMetaData | undefined;
	export let playbackInfo: ProcessorInfo | undefined;
	export let nodeName: string;

	const onPlayPause = async () => {
		const action =
			playbackInfo?.playbackState === 'paused'
				? '"UN_PAUSE_QUEUE"'
				: '"PAUSE_QUEUE"';

		await fetch(`http://127.0.0.1:50051/commands/node/${nodeName}`, {
			method: 'POST',
			body: action,
			headers: { 'Content-Type': 'application/json' }
		});
	};

	const onNext = async () => {
		await fetch(`http://127.0.0.1:50051/commands/node/${nodeName}`, {
			method: 'POST',
			body: '"PLAY_NEXT"',
			headers: { 'Content-Type': 'application/json' }
		});
	};

	const onPrev = async () => {
		await fetch(`http://127.0.0.1:50051/commands/node/${nodeName}`, {
			method: 'POST',
			body: '"PLAY_PREVIOUS"',
			headers: { 'Content-Type': 'application/json' }
		});
	};
</script>

<div class="m-[-16px] flex h-[220px] min-h-[220px] flex-col gap-8 bg-neutral-800 p-4">
	<div class="flex flex-col">
		<span class="text-lg font-bold sm:text-xl 2xl:text-2xl">
			{activeAudio?.name ?? '---'}</span
		>

		<span>By {activeAudio?.author ?? '---'}</span>
	</div>

	<div class="h-1 rounded bg-neutral-100" />

	<div class="flex items-center justify-around gap-2">
		<div
			role="button"
			tabindex="0"
			on:click={onPrev}
			on:keydown={onPrev}
			class="flex h-10 w-10 cursor-pointer items-center justify-center"
		>
			<img class="h-9 w-9" src="/prev.svg" alt="<" />
		</div>
		<div
			role="button"
			tabindex="0"
			on:click={onPlayPause}
			on:keydown={onPlayPause}
			class="flex h-[52px] w-[52px] cursor-pointer items-center justify-center rounded-full
            bg-neutral-100"
		>
			<img
				class="h-8 w-8"
				src={`${
					playbackInfo?.playbackState === 'paused' ? '/play.svg' : '/pause.svg'
				}`}
				alt={`${playbackInfo?.playbackState === 'paused' ? '⏵' : '⏸'}`}
			/>
		</div>
		<div
			role="button"
			tabindex="0"
			on:click={onNext}
			on:keydown={onNext}
			class="flex h-10 w-10 cursor-pointer items-center justify-center"
		>
			<img class="h-9 w-8" src="/next.svg" alt=">" />
		</div>
	</div>
</div>
