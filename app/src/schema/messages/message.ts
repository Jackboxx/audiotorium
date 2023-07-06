export function msgString<T>(msg: [string, T]) {
	return `{ "${msg[0]}": ${JSON.stringify(msg[1])}}`;
}
