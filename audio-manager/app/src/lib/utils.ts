export const API_PREFIX = 'http://192.168.31.235:50051';
export const WS_PREFIX = 'ws://192.168.31.235:50051';

export function unixMillisToMinutesString(millis: number): string {
	const total_seconds = Math.trunc(millis / 1000);
	const seconds = total_seconds % 60;
	const minutes = Math.trunc(total_seconds / 60);

	const str_seconds = seconds < 10 ? `0${seconds}` : `${seconds}`;
	const str_minutes = minutes < 10 ? `0${minutes}` : `${minutes}`;

	return `${str_minutes}:${str_seconds}`;
}
