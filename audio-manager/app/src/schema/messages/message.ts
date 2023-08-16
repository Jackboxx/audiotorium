export function msgString<T>(msg: [string, T] | [string]) {
	return msg.length === 1 ? `"${msg}"` : `{ "${msg[0]}": ${JSON.stringify(msg[1])}}`;
}
