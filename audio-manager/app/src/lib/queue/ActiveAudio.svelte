<script lang="ts">
	import type { AudioMetaData } from '$api/AudioMetaData';
	import type { AudioNodeCommand } from '$api/AudioNodeCommand';
	import type { ProcessorInfo } from '$api/ProcessorInfo';
	import { API_PREFIX, unixMillisToMinutesString } from '$lib/utils';

	export let activeAudio: AudioMetaData | undefined;
	export let playbackInfo: ProcessorInfo | undefined;
	export let nodeName: string;

	let barElement: HTMLDivElement;
	let setHoldingBarWithDelay: ReturnType<typeof setTimeout> | undefined = undefined;
	let holdingBar = false;
	let localProgress = 0;

	let displayedDuration = '';
	let displayedPlayedDuration = '';

	const onPlayPause = async () => {
		const action =
			playbackInfo?.playbackState === 'paused'
				? '"UN_PAUSE_QUEUE"'
				: '"PAUSE_QUEUE"';

		await fetch(`${API_PREFIX}/commands/node/${nodeName}`, {
			method: 'POST',
			body: action,
			headers: { 'Content-Type': 'application/json' }
		});
	};

	const onNext = async () => {
		await fetch(`${API_PREFIX}/commands/node/${nodeName}`, {
			method: 'POST',
			body: '"PLAY_NEXT"',
			headers: { 'Content-Type': 'application/json' }
		});
	};

	const onPrev = async () => {
		await fetch(`${API_PREFIX}/commands/node/${nodeName}`, {
			method: 'POST',
			body: '"PLAY_PREVIOUS"',
			headers: { 'Content-Type': 'application/json' }
		});
	};

	const onBarDragStart = (e: MouseEvent) => {
		const rect = barElement.getBoundingClientRect();
		const x = e.clientX - rect.left;
		const progress = Math.max(Math.min(x / rect.width, 1), 0);

		localProgress = progress;

		setHoldingBarWithDelay = setTimeout(() => {
			holdingBar = true;
		}, 100);
	};

	const onBarDragEnd = async (e: MouseEvent) => {
		const target = e.target as HTMLElement;
		if (!holdingBar && barElement !== target && !barElement.contains(target)) {
			return;
		}

		clearTimeout(setHoldingBarWithDelay); // account for short clicks
		setHoldingBarWithDelay = undefined;

		const cmd: AudioNodeCommand = {
			SET_AUDIO_PROGRESS: {
				progress: localProgress
			}
		};

		await new Promise(async (res, rej) => {
			setTimeout(rej, 500);
			res(
				await fetch(`${API_PREFIX}/commands/node/${nodeName}`, {
					method: 'POST',
					body: JSON.stringify(cmd),
					headers: { 'Content-Type': 'application/json' }
				})
			);
		});

		setTimeout(() => {
			holdingBar = false;
		}, 100);
	};

	const onBarDrag = (e: MouseEvent) => {
		if (!holdingBar) {
			return;
		}

		const rect = barElement.getBoundingClientRect();
		const x = e.clientX - rect.left;
		const progress = Math.max(Math.min(x / rect.width, 1), 0);

		localProgress = progress;
	};

	$: if (!holdingBar && !setHoldingBarWithDelay) {
		localProgress = playbackInfo?.audioProgress ?? 0;
	}

	$: if (activeAudio && activeAudio.duration) {
		displayedDuration = unixMillisToMinutesString(Number(activeAudio.duration));
		displayedPlayedDuration = unixMillisToMinutesString(
			Number(activeAudio.duration) * localProgress
		);
	}
</script>

<svelte:window on:mousemove={onBarDrag} on:mouseup={onBarDragEnd} />

<div class="flex h-[220px] min-h-[220px] w-full justify-center bg-neutral-800">
	<div class="flex h-full w-full max-w-[800px] flex-col gap-8 p-4">
		<div class="flex flex-col">
			<span class="text-lg font-bold sm:text-xl 2xl:text-2xl">
				{activeAudio?.name ?? '---'}</span
			>

			<span>{activeAudio?.author ?? '---'}</span>
		</div>

		<div
			bind:this={barElement}
			on:mousedown={onBarDragStart}
			class="bar relative cursor-pointer before:absolute before:top-3 after:absolute after:right-0 after:top-3"
			style={`--before-content: "${displayedPlayedDuration}"; --after-content: "${displayedDuration}";`}
		>
			<div
				class="absolute z-10 h-2 rounded bg-neutral-100"
				style={`width:
        ${holdingBar ? localProgress * 100 : (playbackInfo?.audioProgress ?? 0) * 100}%`}
			/>
			<div class="h-2 rounded bg-neutral-600" />
		</div>

		<div class="flex items-center justify-around gap-2">
			<div
				role="button"
				tabindex="0"
				on:click={onPrev}
				on:keydown={undefined}
				class="flex h-10 w-10 cursor-pointer items-center justify-center"
			>
				<img class="h-9 w-9 select-none" src="/prev.svg" alt="<" />
			</div>
			<div
				role="button"
				tabindex="0"
				on:click={onPlayPause}
				on:keydown={undefined}
				class="flex h-[52px] w-[52px] cursor-pointer items-center justify-center rounded-full
            bg-neutral-100"
			>
				<img
					class="h-8 w-8 select-none"
					src={`${
						playbackInfo?.playbackState === 'paused'
							? '/play.svg'
							: '/pause.svg'
					}`}
					alt={`${playbackInfo?.playbackState === 'paused' ? '⏵' : '⏸'}`}
				/>
			</div>
			<div
				role="button"
				tabindex="0"
				on:click={onNext}
				on:keydown={undefined}
				class="flex h-10 w-10 cursor-pointer items-center justify-center"
			>
				<img class="h-9 w-9 select-none" src="/next.svg" alt=">" />
			</div>
		</div>
	</div>
</div>

<style>
	.bar::before {
		content: var(--before-content);
	}

	.bar::after {
		content: var(--after-content);
	}
</style>
