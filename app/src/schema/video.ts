import { PUBLIC_YOUTUBE_API_KEY } from '$env/static/public';
import { z } from 'zod';

export const ThumbNailSchema = z.object({
	url: z.string().url(),
	width: z.number(),
	height: z.number()
});

export const ThumbNailsSchema = z.object({
	default: ThumbNailSchema,
	medium: ThumbNailSchema.nullish(),
	high: ThumbNailSchema.nullish()
});

export const YouTubeVideoSchema = z.object({
	id: z.object({
		videoId: z.string().nonempty()
	}),
	snippet: z.object({
		title: z.string(),
		description: z.string(),
		thumbnails: ThumbNailsSchema
	})
});

export const YouTubeSearchResultSchema = z.object({ items: YouTubeVideoSchema.array() });

export type ThumbNail = z.infer<typeof ThumbNailSchema>;
export type ThumbNails = z.infer<typeof ThumbNailsSchema>;
export type YouTubeVideo = z.infer<typeof YouTubeVideoSchema>;
export type YouTubeSearchResult = z.infer<typeof YouTubeSearchResultSchema>;

export function getBestThumbNail(thumbNails: ThumbNails): ThumbNail {
	return thumbNails.high ?? thumbNails.medium ?? thumbNails.default;
}

export function getVideoUrl(video: YouTubeVideo): string {
	return `https://www.youtube.com/watch?v=${video.id.videoId}`;
}

export async function searchForVideos(search: string): Promise<Response> {
	return await fetch(
		`https://www.googleapis.com/youtube/v3/search?q=${search}&part=snippet&type=video&maxResults=10&key=${PUBLIC_YOUTUBE_API_KEY}`
	);
}
